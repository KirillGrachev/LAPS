//! Разбор LAPS-атрибутов объекта компьютера.
//!
//! Источники данных (см. `services/ldap` и MS LAPS schema reference):
//! * legacy LAPS: `ms-Mcs-AdmPwd` (строка-пароль),
//!   `ms-Mcs-AdmPwdExpirationTime` (десятичный FILETIME);
//! * Windows LAPS, открытое хранение: `msLAPS-Password` — JSON
//!   `{"n":"Administrator","t":"1d8161b41c41cde","p":"<пароль>"}`
//!   (`t` — **шестнадцатеричный** FILETIME момента обновления),
//!   `msLAPS-PasswordExpirationTime` (десятичный FILETIME);
//! * Windows LAPS, шифрованное хранение: `msLAPS-EncryptedPassword` —
//!   конверт MS-ADA2 2.66 с DPAPI-NG/KDS-блоком. Заголовок разбирается
//!   здесь (платформонезависимо, с тестами), дешифровка блока — в
//!   [`super::decrypt`] через нативный API Windows LAPS (`lapsutil.dll`).

use serde::Deserialize;

use crate::services::ldap::NormalizedEntry;

use super::model::{LapsFlavor, LapsInfo};
use super::time;

/// LDAP-имена атрибутов.
pub const ATTR_CN: &str = "cn";
pub const ATTR_SAM_ACCOUNT_NAME: &str = "samaccountname";
pub const ATTR_DNS_HOST_NAME: &str = "dnshostname";
pub const ATTR_OPERATING_SYSTEM: &str = "operatingsystem";
pub const ATTR_LEGACY_PASSWORD: &str = "ms-mcs-admpwd";
pub const ATTR_LEGACY_EXPIRATION: &str = "ms-mcs-admpwdexpirationtime";
pub const ATTR_WIN_PASSWORD: &str = "mslaps-password";
pub const ATTR_WIN_EXPIRATION: &str = "mslaps-passwordexpirationtime";
pub const ATTR_WIN_ENCRYPTED: &str = "mslaps-encryptedpassword";

/// Полный список атрибутов поискового запроса.
///
/// `msLAPS-EncryptedPassword` запрашивается только для детекта presence —
/// значение (бинарный блок) наружу не отдаётся и в логах не появляется.
pub const COMPUTER_ATTRIBUTES: &[&str] = &[
    "cn",
    "sAMAccountName",
    "dNSHostName",
    "operatingSystem",
    "ms-Mcs-AdmPwd",
    "ms-Mcs-AdmPwdExpirationTime",
    "msLAPS-Password",
    "msLAPS-PasswordExpirationTime",
    "msLAPS-EncryptedPassword",
];

/// JSON содержимого `msLAPS-Password` (документированные поля `n`/`t`/`p`).
///
/// Допускаются и «длинные» имена полей — некоторые сторонние агенты
/// и ранние сборки писали `{"Password":...,"TimeStamp":...}`; парсер
/// терпим к обоим форматам.
#[derive(Debug, Deserialize)]
struct WindowsLapsJson {
    #[serde(default, alias = "AccountName")]
    n: Option<String>,
    #[serde(default)]
    t: Option<String>,
    #[serde(default, alias = "Password")]
    p: Option<String>,
    #[serde(default, alias = "TimeStamp")]
    timestamp: Option<String>,
}

/// Данные открытого пароля Windows LAPS.
#[derive(Debug, PartialEq, Eq)]
pub struct WindowsPlainPassword {
    pub account_name: Option<String>,
    pub password: String,
    /// FILETIME момента обновления пароля (поле `t`).
    pub updated_at: Option<i64>,
}

/// Длина заголовка значения `msLAPS-EncryptedPassword` (MS-ADA2 2.66):
/// 8 байт FILETIME + 4 байта размера блоба + 4 зарезервированных.
pub const ENCRYPTED_ATTRIBUTE_HEADER_LEN: usize = 16;

/// Минимально правдоподобный размер DPAPI-NG-блока.
const MIN_ENCRYPTED_BLOB_LEN: usize = 64;

/// Разобранное значение `msLAPS-EncryptedPassword`.
///
/// AD хранит в атрибуте не «голый» DPAPI-NG-блок, а конверт:
/// `PasswordUpdateTimestamp (8) | EncryptedPasswordSize (4) | Reserved (4) |
/// EncryptedPassword (N)`. Документированное смещение блока — 16 байт,
/// но разные агенты трактуют `EncryptedPasswordSize` по-разному:
/// * размер только блока (Microsoft, LAPS4Linux);
/// * размер вместе с 16-байтовым заголовком (встречается у сторонних
///   агентов и в экспортированных значениях);
/// * размер лежит в поле `Reserved`/по offset 12;
/// * timestamp бывает нулевым.
///
/// Поэтому парсер возвращает **несколько кандидатов** на расшифровку,
/// а не падает на первом несовпадении. Это важно для совместимости:
/// `DecryptNormalMode` сам скажет, какой вариант валиден, а пользователь
/// не увидит ложное «некорректный заголовок».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedLapsBlob<'a> {
    /// FILETIME момента записи пароля (если он правдоподобен).
    pub updated_at: Option<i64>,
    /// Возможные DPAPI-NG-блоки в порядке приоритета.
    pub candidates: Vec<&'a [u8]>,
    /// Человекочитаемая расшифровка заголовка для диагностики.
    pub diagnostics: String,
}

/// Разбирает конверт `msLAPS-EncryptedPassword` и собирает кандидатов.
///
/// `Err` — только если значение физически короче заголовка. Любые
/// странности размера/timestamp превращаются в набор кандидатов:
/// попытка расшифровки даст точную ошибку `lapsutil`, а не «некорректный
/// заголовок».
pub fn parse_encrypted_laps_blob<'a>(raw: &'a [u8]) -> Result<EncryptedLapsBlob<'a>, String> {
    if raw.len() < ENCRYPTED_ATTRIBUTE_HEADER_LEN {
        return Err(format!(
            "некорректный заголовок msLAPS-EncryptedPassword: {} байт (нужно ≥16); первые байты: {}",
            raw.len(),
            hex_preview(raw)
        ));
    }

    let timestamp = u64::from_le_bytes(raw[0..8].try_into().expect("8 байт"));
    let size_le = u32::from_le_bytes(raw[8..12].try_into().expect("4 байта")) as usize;
    let size_alt = u32::from_le_bytes(raw[12..16].try_into().expect("4 байта")) as usize;
    let size_be = u32::from_be_bytes(raw[8..12].try_into().expect("4 байта")) as usize;
    let payload = &raw[ENCRYPTED_ATTRIBUTE_HEADER_LEN..];

    let updated_at = i64::try_from(timestamp).ok().filter(|value| *value > 0);

    let mut candidates: Vec<&[u8]> = Vec::new();
    let mut add = |candidate: &'a [u8]| {
        if candidate.len() >= MIN_ENCRYPTED_BLOB_LEN
            && !candidates
                .iter()
                .any(|existing| existing.len() == candidate.len() && *existing == candidate)
        {
            candidates.push(candidate);
        }
    };

    // Основной вариант: размер — только блок.
    if size_le > 0 && size_le <= payload.len() {
        add(&payload[..size_le]);
    }
    // Размер включает 16-байтовый заголовок.
    if size_le >= ENCRYPTED_ATTRIBUTE_HEADER_LEN
        && size_le - ENCRYPTED_ATTRIBUTE_HEADER_LEN <= payload.len()
    {
        add(&payload[..size_le - ENCRYPTED_ATTRIBUTE_HEADER_LEN]);
    }
    // Альтернативное поле размера (offset 12) и big-endian.
    if size_alt > 0 && size_alt <= payload.len() {
        add(&payload[..size_alt]);
    }
    if size_be > 0 && size_be <= payload.len() {
        add(&payload[..size_be]);
    }
    // Последний шанс: весь хвост после заголовка.
    add(payload);
    // Совсем старые/сторонние агенты могли писать 12/8/4-байтовый
    // заголовок или «голый» блок: пробуем и эти смещения.
    for offset in [12, 8, 4] {
        if let Some(rest) = raw.get(offset..) {
            add(rest);
        }
    }
    // Всё значение целиком — если конкретный API ждёт его в таком виде.
    add(raw);

    if candidates.is_empty() {
        return Err(format!(
            "некорректный заголовок msLAPS-EncryptedPassword: len={}, size8={size_le}, \
             size12={size_alt}, sizeBE={size_be}, payload={}; первые байты: {}",
            raw.len(),
            payload.len(),
            hex_preview(raw)
        ));
    }

    let diagnostics = format!(
        "len={}, timestamp={}, size8={size_le}, size12={size_alt}, sizeBE={size_be}, \
         payload={}, кандидатов={}, первые байты: {}",
        raw.len(),
        timestamp,
        payload.len(),
        candidates.len(),
        hex_preview(raw)
    );

    Ok(EncryptedLapsBlob {
        updated_at,
        candidates,
        diagnostics,
    })
}

/// Первые 24 байта значения в hex — для диагностики (без секретов:
/// это шифртекст/заголовок, не пароль).
fn hex_preview(raw: &[u8]) -> String {
    raw.iter()
        .take(24)
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Разбирает расшифрованное содержимое `msLAPS-EncryptedPassword`.
///
/// Windows LAPS пишет UTF-16LE JSON (`{"n":...,"t":...,"p":...}`), но
/// встречаются агенты, сохраняющие UTF-8. Пробуем UTF-16LE, затем UTF-8;
/// BOM и завершающие NUL отбрасываются.
///
/// На не-Windows сборке функция используется только тестами: сам вызов
/// дешифровки живёт в Windows-модуле.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_windows_plain_payload(raw: &[u8]) -> Option<WindowsPlainPassword> {
    parse_windows_plain_utf16le(raw).or_else(|| {
        let text = std::str::from_utf8(raw).ok()?;
        parse_windows_plain_json(text.trim_matches(['\0', '\u{feff}']).trim())
    })
}

#[cfg_attr(not(windows), allow(dead_code))]
fn parse_windows_plain_utf16le(raw: &[u8]) -> Option<WindowsPlainPassword> {
    // Нечётная длина — почти наверняка не UTF-16LE: не рискуем.
    if raw.len() < 2 || !raw.len().is_multiple_of(2) {
        return None;
    }
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    let text = String::from_utf16_lossy(&units);
    parse_windows_plain_json(text.trim_matches(['\0', '\u{feff}']).trim())
}

/// Разбор JSON из `msLAPS-Password`.
///
/// Возвращает `None`, если JSON невалиден или в нём нет пароля —
/// такой атрибут трактуется как отсутствие данных (см. `pick_flavor`).
/// Совместимость с «TimeStamp» в десятичном виде.
pub fn parse_windows_plain_json(raw: &str) -> Option<WindowsPlainPassword> {
    let parsed: WindowsLapsJson = serde_json::from_str(raw.trim()).ok()?;
    let password = parsed.p.filter(|p| !p.is_empty())?;

    let updated_at = parsed
        .t
        .as_deref()
        .and_then(time::parse_filetime_hex)
        .or_else(|| {
            parsed.timestamp.as_deref().and_then(|value| {
                time::parse_filetime_decimal(value).or_else(|| time::parse_filetime_hex(value))
            })
        });

    Some(WindowsPlainPassword {
        account_name: parsed.n.filter(|n| !n.is_empty()),
        password,
        updated_at,
    })
}

/// Определяет вариант хранения по набору присутствующих атрибутов.
///
/// Приоритет: открытый Windows LAPS > legacy > шифрованный Windows LAPS.
/// (Открытый новый формат предпочтительнее legacy-остатков миграции;
/// шифрованный — только если открытых паролей нет вовсе.)
pub fn pick_flavor(
    windows_plain_password: Option<&WindowsPlainPassword>,
    legacy_password: Option<&str>,
    encrypted_present: bool,
) -> LapsFlavor {
    if windows_plain_password.is_some() {
        LapsFlavor::WindowsPlain
    } else if legacy_password.is_some() {
        LapsFlavor::Legacy
    } else if encrypted_present {
        LapsFlavor::WindowsEncrypted
    } else {
        LapsFlavor::None
    }
}

/// Собирает [`LapsInfo`] из записи каталога.
/// Срок истечения: для Windows LAPS — новый атрибут, для legacy — старый.
/// В смешанном домене берём тот, что соответствует выбранному варианту,
/// с запасным падением на второй.
/// Писать срок истечения умеем в атрибут того семейства, которое
/// присутствует на объекте (операция сброса/продления).
/// Шифрованный пароль на этом этапе не отдаётся: его добавит
/// `decrypt_windows_laps_password` после успешной нативной дешифровки.
pub fn build_laps_info(entry: &NormalizedEntry) -> LapsInfo {
    let computer_name = entry
        .text(ATTR_SAM_ACCOUNT_NAME)
        .map(|sam| sam.trim_end_matches('$').to_string())
        .or_else(|| entry.text(ATTR_CN))
        .unwrap_or_else(|| cn_from_dn(&entry.dn));

    let windows_plain = entry
        .text(ATTR_WIN_PASSWORD)
        .as_deref()
        .and_then(parse_windows_plain_json);
    let legacy_password = entry.text(ATTR_LEGACY_PASSWORD);
    let encrypted_present = entry.has(ATTR_WIN_ENCRYPTED);

    let flavor = pick_flavor(
        windows_plain.as_ref(),
        legacy_password.as_deref(),
        encrypted_present,
    );

    let win_expiration = entry
        .text(ATTR_WIN_EXPIRATION)
        .as_deref()
        .and_then(time::parse_filetime_decimal);
    let legacy_expiration = entry
        .text(ATTR_LEGACY_EXPIRATION)
        .as_deref()
        .and_then(time::parse_filetime_decimal);
    let expiration = match flavor {
        LapsFlavor::Legacy => legacy_expiration.or(win_expiration),
        LapsFlavor::WindowsPlain | LapsFlavor::WindowsEncrypted => {
            win_expiration.or(legacy_expiration)
        }
        LapsFlavor::None => win_expiration.or(legacy_expiration),
    };

    let win_expiration_present = win_expiration.is_some();
    let legacy_expiration_present = legacy_expiration.is_some();
    let rotation_possible = win_expiration_present || legacy_expiration_present;

    // Без `expect`: если инвариант flavor↔windows_plain когда-нибудь
    // нарушится, карточка просто останется без пароля, а не уронит процесс.
    let (password, account_name, updated_at) = match (flavor, windows_plain.as_ref()) {
        (LapsFlavor::WindowsPlain, Some(plain)) => (
            Some(plain.password.clone()),
            plain.account_name.clone(),
            plain.updated_at,
        ),
        (LapsFlavor::Legacy, _) => (legacy_password.clone(), None, None),
        _ => (None, None, None),
    };

    LapsInfo {
        dn: entry.dn.clone(),
        computer_name,
        dns_host_name: entry.text(ATTR_DNS_HOST_NAME),
        operating_system: entry.text(ATTR_OPERATING_SYSTEM),
        flavor,
        account_name,
        password,
        updated_at: updated_at.and_then(time::filetime_to_rfc3339_local),
        expires_at: expiration.and_then(time::filetime_to_rfc3339_local),
        decrypted: false,
        decrypt_method: None,
        decrypt_error: None,
        legacy_present: legacy_password.is_some() || legacy_expiration.is_some(),
        windows_plain_present: windows_plain.is_some() || win_expiration.is_some(),
        encrypted_present,
        rotation_possible,
        win_expiration_present,
        legacy_expiration_present,
    }
}

/// Аварийное извлечение имени из DN (`CN=WS01,OU=...` -> `WS01`).
fn cn_from_dn(dn: &str) -> String {
    dn.split(',')
        .next()
        .and_then(|first| first.split_once('='))
        .map(|(_, value)| value.trim().to_string())
        .unwrap_or_else(|| dn.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ldap3::SearchEntry;
    use std::collections::HashMap;

    fn entry_with(attrs: Vec<(&str, &str)>) -> NormalizedEntry {
        NormalizedEntry::from_search(SearchEntry {
            dn: "CN=WS01,OU=Workstations,DC=company,DC=ru".to_string(),
            attrs: attrs
                .into_iter()
                .map(|(k, v)| (k.to_string(), vec![v.to_string()]))
                .collect::<HashMap<_, _>>(),
            bin_attrs: HashMap::new(),
        })
    }

    #[test]
    /// Формат из MS LAPS schema reference.
    fn parses_documented_windows_laps_json() {
        let raw = r#"{"n":"Administrator","t":"1d8161b41c41cde","p":"A6a3#7%eb!57be4a4B95Z4"}"#;
        let parsed = parse_windows_plain_json(raw).expect("валидный JSON");
        assert_eq!(parsed.account_name.as_deref(), Some("Administrator"));
        assert_eq!(parsed.password, "A6a3#7%eb!57be4a4B95Z4");
        assert_eq!(parsed.updated_at, Some(0x1d8161b41c41cde));
    }

    #[test]
    fn parses_custom_admin_account_json() {
        let raw = r#"{"n":"LocalAdmin","t":"1d978c1a2b3c4d5","p":"p@ss"}"#;
        let parsed = parse_windows_plain_json(raw).expect("валидный JSON");
        assert_eq!(parsed.account_name.as_deref(), Some("LocalAdmin"));
    }

    #[test]
    fn tolerates_long_field_names() {
        let raw = r#"{"Password":"secret123","TimeStamp":"133256448000000000"}"#;
        let parsed = parse_windows_plain_json(raw).expect("валидный JSON");
        assert_eq!(parsed.password, "secret123");
        assert_eq!(parsed.updated_at, Some(133_256_448_000_000_000));
    }

    #[test]
    fn rejects_broken_or_empty_json() {
        assert!(parse_windows_plain_json("не json").is_none());
        assert!(parse_windows_plain_json(r#"{"n":"Administrator"}"#).is_none());
        assert!(parse_windows_plain_json(r#"{"p":""}"#).is_none());
    }

    #[test]
    fn flavor_priority_prefers_windows_plain_over_legacy() {
        let plain = WindowsPlainPassword {
            account_name: None,
            password: "new".into(),
            updated_at: None,
        };
        assert_eq!(
            pick_flavor(Some(&plain), Some("old"), true),
            LapsFlavor::WindowsPlain
        );
        assert_eq!(pick_flavor(None, Some("old"), true), LapsFlavor::Legacy);
        assert_eq!(pick_flavor(None, None, true), LapsFlavor::WindowsEncrypted);
        assert_eq!(pick_flavor(None, None, false), LapsFlavor::None);
    }

    #[test]
    fn builds_info_for_legacy_computer() {
        let entry = entry_with(vec![
            ("CN", "WS01"),
            ("sAMAccountName", "WS01$"),
            ("dNSHostName", "ws01.company.ru"),
            ("operatingSystem", "Windows 10 Pro"),
            ("ms-Mcs-AdmPwd", "OldP@ssw0rd"),
            ("ms-Mcs-AdmPwdExpirationTime", "133256448000000000"),
        ]);
        let info = build_laps_info(&entry);
        assert_eq!(info.computer_name, "WS01");
        assert_eq!(info.flavor, LapsFlavor::Legacy);
        assert_eq!(info.password.as_deref(), Some("OldP@ssw0rd"));
        assert!(info.expires_at.is_some());
        assert!(info.legacy_present);
        assert!(!info.windows_plain_present);
        assert!(info.rotation_possible);
    }

    #[test]
    fn builds_info_for_windows_laps_plain_computer() {
        let entry = entry_with(vec![
            ("CN", "WS02"),
            ("sAMAccountName", "WS02$"),
            (
                "msLAPS-Password",
                r#"{"n":"Administrator","t":"1d8161b41c41cde","p":"N3wP@ss!"}"#,
            ),
            ("msLAPS-PasswordExpirationTime", "133256448000000000"),
        ]);
        let info = build_laps_info(&entry);
        assert_eq!(info.flavor, LapsFlavor::WindowsPlain);
        assert_eq!(info.password.as_deref(), Some("N3wP@ss!"));
        assert_eq!(info.account_name.as_deref(), Some("Administrator"));
        assert!(info.updated_at.is_some());
        assert!(info.windows_plain_present);
        assert!(info.rotation_possible);
    }

    #[test]
    /// Имитируем бинарный зашифрованный пароль + срок истечения.
    /// Сброс срока возможен и для шифрованного варианта: агент сам
    /// сгенерирует новый пароль, чтение здесь не нужно.
    fn encrypted_only_computer_has_no_password() {
        let with_encrypted = NormalizedEntry::from_search(SearchEntry {
            dn: "CN=WS03,OU=Workstations,DC=company,DC=ru".to_string(),
            attrs: HashMap::from([
                ("cn".to_string(), vec!["WS03".to_string()]),
                ("sAMAccountName".to_string(), vec!["WS03$".to_string()]),
                (
                    "msLAPS-PasswordExpirationTime".to_string(),
                    vec!["133256448000000000".to_string()],
                ),
            ]),
            bin_attrs: HashMap::from([(
                "msLAPS-EncryptedPassword".to_string(),
                vec![vec![0x01, 0x02, 0x03]],
            )]),
        });
        let info = build_laps_info(&with_encrypted);
        assert_eq!(info.flavor, LapsFlavor::WindowsEncrypted);
        assert!(info.password.is_none());
        assert!(info.encrypted_present);
        assert!(info.expires_at.is_some());
        assert!(info.rotation_possible);
    }

    #[test]
    fn extracts_cn_from_dn_as_fallback() {
        assert_eq!(cn_from_dn("CN=WS01,OU=X,DC=y"), "WS01");
        assert_eq!(cn_from_dn("garbage"), "garbage");
    }

    fn encrypted_attribute(timestamp: i64, declared: u32, blob: &[u8]) -> Vec<u8> {
        let mut raw = Vec::with_capacity(16 + blob.len());
        raw.extend_from_slice(&(timestamp as u64).to_le_bytes());
        raw.extend_from_slice(&declared.to_le_bytes());
        raw.extend_from_slice(&0u32.to_le_bytes());
        raw.extend_from_slice(blob);
        raw
    }

    #[test]
    /// Конверт MS-ADA2: 8 байт времени, 4 размера, 4 резерва, затем блок.
    /// Смещение блока — 16, а не 0/4/8/12.
    fn parses_encrypted_attribute_envelope() {
        let blob = vec![0xAB; 96];
        let raw = encrypted_attribute(133_256_448_000_000_000, blob.len() as u32, &blob);
        let parsed = parse_encrypted_laps_blob(&raw).expect("конверт разобран");
        assert_eq!(parsed.updated_at, Some(133_256_448_000_000_000));
        assert_eq!(parsed.candidates[0], blob.as_slice());
        assert!(!parsed.diagnostics.is_empty());
    }

    #[test]
    /// Некоторые агенты пишут в `EncryptedPasswordSize` размер вместе
    /// с 16-байтовым заголовком. Это не ошибка: берём size-16.
    fn accepts_size_including_header() {
        let blob = vec![0xAB; 96];
        let raw = encrypted_attribute(133_256_448_000_000_000, blob.len() as u32 + 16, &blob);
        let parsed = parse_encrypted_laps_blob(&raw).expect("size с заголовком");
        assert_eq!(parsed.candidates[0], blob.as_slice());
    }

    #[test]
    /// Размер может лежать в поле offset 12 (reserved у других агентов).
    fn accepts_alternative_size_field() {
        let blob = vec![0xAB; 96];
        let mut raw = Vec::new();
        raw.extend_from_slice(&133_256_448_000_000_000u64.to_le_bytes());
        raw.extend_from_slice(&0u32.to_le_bytes()); // size8
        raw.extend_from_slice(&(blob.len() as u32).to_le_bytes()); // size12
        raw.extend_from_slice(&blob);
        let parsed = parse_encrypted_laps_blob(&raw).expect("size12");
        assert_eq!(parsed.candidates[0], blob.as_slice());
    }

    #[test]
    /// Нулевой timestamp не мешает дешифровке: время просто неизвестно.
    fn accepts_zero_timestamp() {
        let blob = vec![0xAB; 96];
        let raw = encrypted_attribute(0, blob.len() as u32, &blob);
        let parsed = parse_encrypted_laps_blob(&raw).expect("timestamp 0");
        assert!(parsed.updated_at.is_none());
        assert_eq!(parsed.candidates[0], blob.as_slice());
    }

    #[test]
    /// Странный/нулевой/слишком большой размер — не отказ: последний
    /// кандидат — весь хвост после заголовка. Точную ошибку даст lapsutil.
    fn falls_back_to_payload_on_inconsistent_size() {
        let blob = vec![0xAB; 96];
        for declared in [0_u32, 8, 4096] {
            let raw = encrypted_attribute(133_256_448_000_000_000, declared, &blob);
            let parsed =
                parse_encrypted_laps_blob(&raw).unwrap_or_else(|e| panic!("size={declared}: {e}"));
            assert_eq!(parsed.candidates[0], blob.as_slice(), "size={declared}");
        }
    }

    #[test]
    fn rejects_too_short_encrypted_attribute() {
        assert!(parse_encrypted_laps_blob(&[]).is_err());
        assert!(parse_encrypted_laps_blob(&[0xAB; 15]).is_err());
    }

    #[test]
    /// Хвост после объявленного размера (выравнивание) не попадает в блок.
    fn encrypted_attribute_uses_declared_length() {
        let blob = vec![0xCD; 80];
        let mut raw = encrypted_attribute(133_256_448_000_000_000, 64, &blob);
        raw.extend_from_slice(&[0u8; 16]);
        let parsed = parse_encrypted_laps_blob(&raw).expect("конверт разобран");
        assert_eq!(parsed.candidates[0].len(), 64);
    }

    #[test]
    /// Расшифрованный payload — UTF-16LE JSON; BOM и NUL отбрасываются.
    fn parses_utf16le_decrypted_payload() {
        let json = r#"{"n":"Administrator","t":"1d8161b41c41cde","p":"N3wP@ss!"}"#;
        let mut raw: Vec<u8> = vec![0xFF, 0xFE];
        raw.extend(json.encode_utf16().flat_map(u16::to_le_bytes));
        raw.extend_from_slice(&[0, 0]);
        let parsed = parse_windows_plain_payload(&raw).expect("UTF-16LE JSON");
        assert_eq!(parsed.password, "N3wP@ss!");
        assert_eq!(parsed.account_name.as_deref(), Some("Administrator"));
        assert_eq!(parsed.updated_at, Some(0x1d8161b41c41cde));
    }

    #[test]
    fn parses_utf8_decrypted_payload_as_fallback() {
        let raw = br#"{"n":"LocalAdmin","p":"utf8-secret"}"#;
        let parsed = parse_windows_plain_payload(raw).expect("UTF-8 JSON");
        assert_eq!(parsed.password, "utf8-secret");
    }

    #[test]
    fn rejects_garbage_decrypted_payload() {
        assert!(parse_windows_plain_payload(b"").is_none());
        assert!(parse_windows_plain_payload(&[0xFF; 7]).is_none());
        assert!(parse_windows_plain_payload(b"not json").is_none());
    }
}
