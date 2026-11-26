//! Доменная логика LAPS: поиск компьютера, чтение пароля, сброс срока.
//!
//! Все функции работают поверх нативного LDAP-клиента
//! ([`crate::services::ldap`]): одно соединение на операцию, никаких
//! внешних процессов, PowerShell и RSAT.
//!
//! Поддерживаемые варианты хранения пароля:
//! * legacy LAPS - `ms-Mcs-AdmPwd` (открытый текст, ACL на атрибут);
//! * Windows LAPS - `msLAPS-Password` (JSON, открытое хранение);
//! * Windows LAPS - `msLAPS-EncryptedPassword` (шифрованное хранение):
//!   конверт разбирается по MS-ADA2 2.66, а DPAPI-NG-блок расшифровывается
//!   нативным API Windows LAPS (`lapsutil.dll!DecryptNormalMode`, см.
//!   [`decrypt`]); для SSO остаётся запасной `NCryptUnprotectSecret`. Если у
//!   учётной записи процесса нет прав KDS или нет маршрута к контроллеру
//!   домена, карточка всё равно возвращается: UI показывает диагностику
//!   и доступные действия (например, сброс срока - агент сгенерирует
//!   новый пароль).

mod decrypt;
mod expiration;
mod model;
mod parse;
mod query;
pub(crate) mod time;

pub use expiration::RotationMode;
pub use model::{LapsFlavor, LapsInfo};
pub use parse::COMPUTER_ATTRIBUTES;

use ldap3::Scope;
use tracing::{debug, info};

use crate::error::AppError;
use crate::services::config::ConnectionTest;
use crate::services::ldap::{
    open_session, read_root_dse, search, DirectoryCredentials, NormalizedEntry, OPERATION_TIMEOUT,
};

use decrypt::{decrypt_windows_laps_password, DecryptIdentity};
use expiration::build_rotation_mods;
use parse::{build_laps_info, parse_encrypted_laps_blob, ATTR_WIN_ENCRYPTED};
use query::{find_computers, parse_query_async};

/// Учётная запись для KDS-дешифровки: явный вход даёт свой токен,
/// SSO оставляет работу под текущим процессом.
fn decrypt_identity(credentials: &DirectoryCredentials<'_>) -> Option<DecryptIdentity> {
    if credentials.use_integrated_auth {
        return None;
    }
    let account = credentials.bind_dn?.trim();
    let password = credentials.password?;
    if account.is_empty() || password.is_empty() {
        return None;
    }
    let (split_domain, user) = split_bind_account(account);
    // UPN нельзя передавать вместе с доменом; короткому имени домен
    // подставляем из Base DN/LDAP URL (домен каталога), затем из текущей
    // Windows-сессии. Иначе LogonWithCredentials создаёт локальный токен,
    // и KDS отвечает 0x8009002C, даже когда права в AD есть.
    let domain = match split_domain {
        Some(domain) => Some(domain.to_string()),
        None if account.contains('@') => None,
        None => domain_from_base_dn(credentials.base_dn)
            .or_else(|| domain_from_ldap_url(credentials.ldap_url))
            .or_else(current_domain),
    };
    debug!(
        domain = ?domain,
        user,
        "LAPS: дешифровка под явной учётной записью"
    );
    Some(DecryptIdentity {
        domain,
        user: user.to_string(),
        password: zeroize::Zeroizing::new(password.to_string()),
    })
}

/// DNS-домен из Base DN: `DC=company,DC=ru` -> `company.ru`.
fn domain_from_base_dn(base_dn: &str) -> Option<String> {
    let parts: Vec<&str> = base_dn
        .split(',')
        .filter_map(|part| {
            let part = part.trim();
            part.strip_prefix("DC=")
                .or_else(|| part.strip_prefix("dc="))
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join("."))
}

/// DNS-домен из LDAP URL: `ldap://dc1.company.ru` -> `company.ru`.
fn domain_from_ldap_url(url: &str) -> Option<String> {
    let host = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(url)
        .split('/')
        .next()?
        .split(':')
        .next()?;
    if host.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }
    let labels: Vec<&str> = host.split('.').filter(|label| !label.is_empty()).collect();
    if labels.len() >= 3 {
        Some(labels[1..].join("."))
    } else if labels.len() == 2 {
        Some(labels.join("."))
    } else {
        None
    }
}

/// NetBIOS-домен текущей Windows-сессии, если он не совпадает с именем
/// компьютера (то есть пользователь действительно доменный).
fn current_domain() -> Option<String> {
    let domain = std::env::var("USERDOMAIN").ok()?.trim().to_string();
    let computer = std::env::var("COMPUTERNAME").unwrap_or_default();
    if domain.is_empty() || domain.eq_ignore_ascii_case(computer.trim()) {
        None
    } else {
        Some(domain)
    }
}

/// `COMPANY\user` -> (Some("COMPANY"), "user"); UPN и короткое имя
/// передаются как есть (домен определяется Windows/UPN-суффиксом).
fn split_bind_account(account: &str) -> (Option<&str>, &str) {
    match account.split_once('\\') {
        Some((domain, user)) if !domain.trim().is_empty() && !user.trim().is_empty() => {
            (Some(domain.trim()), user.trim())
        }
        _ => (None, account),
    }
}

/// Карточка компьютера: для шифрованного хранения разбираем конверт
/// `msLAPS-EncryptedPassword` и расшифровываем DPAPI-NG-блок нативно
/// (`lapsutil.dll!DecryptNormalMode`, см. [`decrypt`]). Любая ошибка (нет прав KDS,
/// нет маршрута к контроллеру домена, повреждённый блок) не является
/// отказом операции: карточка возвращается без пароля с диагностикой.
async fn laps_info_from_entry(
    entry: &NormalizedEntry,
    credentials: &DirectoryCredentials<'_>,
) -> LapsInfo {
    let mut info = build_laps_info(entry);
    if info.flavor != LapsFlavor::WindowsEncrypted {
        return info;
    }

    let Some(raw) = entry.binary(ATTR_WIN_ENCRYPTED) else {
        info.decrypt_error = Some("значение msLAPS-EncryptedPassword пусто".to_string());
        return info;
    };
    let parsed = match parse_encrypted_laps_blob(&raw) {
        Ok(parsed) => parsed,
        Err(reason) => {
            tracing::warn!(
                computer = %info.computer_name,
                error = %reason,
                "LAPS: заголовок msLAPS-EncryptedPassword не разобран"
            );
            info.decrypt_error = Some(reason);
            return info;
        }
    };

    // Метка обновления из заголовка атрибута полезна и без дешифровки.
    info.updated_at = parsed.updated_at.and_then(time::filetime_to_rfc3339_local);

    // lapsutil/LogonWithCredentials - блокирующие Win32/RPC-вызовы.
    // Уводим их с async-воркера, иначе недоступный DC подвешивает команду.
    let candidates: Vec<Vec<u8>> = parsed.candidates.iter().map(|part| part.to_vec()).collect();
    let identity = decrypt_identity(credentials);
    let identity_label = identity
        .as_ref()
        .map(DecryptIdentity::label)
        .unwrap_or_else(|| "текущая Windows-учётка".to_string());
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let refs: Vec<&[u8]> = candidates.iter().map(Vec::as_slice).collect();
        decrypt_windows_laps_password(&refs, identity.as_ref())
    })
    .await
    .unwrap_or_else(|error| Err(format!("блокирующая задача дешифровки: {error}")));

    match outcome {
        Ok(plain) => apply_decrypted(&mut info, plain),
        Err(reason) => {
            let full_reason = format!(
                "identity={identity_label}; {reason}; заголовок: {}",
                parsed.diagnostics
            );
            tracing::warn!(
                computer = %info.computer_name,
                error = %full_reason,
                "LAPS: шифрованный пароль не расшифрован"
            );
            info.decrypt_error = Some(full_reason);
        }
    }
    info
}

fn apply_decrypted(info: &mut LapsInfo, plain: parse::WindowsPlainPassword) {
    info.password = Some(plain.password);
    info.account_name = plain.account_name;
    if let Some(updated_at) = plain.updated_at.and_then(time::filetime_to_rfc3339_local) {
        info.updated_at = Some(updated_at);
    }
    info.decrypted = true;
    info.decrypt_method = Some("dpapi-ng".to_string());
    info.decrypt_error = None;
}

/// Максимум имён в диагностике неоднозначного поиска.
const AMBIGUOUS_LIST_LIMIT: usize = 8;
/// Поиск компьютера и чтение его LAPS-карточки.
///
/// Ошибки возвращаются для транспортных/протокольных проблем, «не найден»
/// и неоднозначности. Компьютер с шифрованным паролем - **не** ошибка:
/// возвращается карточка с `flavor = WindowsEncrypted` и `password = None`,
/// чтобы UI показал диагностику и доступные операции.
pub async fn search_computer(
    credentials: &DirectoryCredentials<'_>,
    raw_query: &str,
) -> Result<LapsInfo, AppError> {
    let query = parse_query_async(raw_query).await?;

    let (mut ldap, _bind_ms) = open_session(credentials).await?;
    let base = resolve_base_dn(&mut ldap, credentials).await?;
    let entries = find_computers(
        &mut ldap,
        credentials,
        &base,
        &query,
        COMPUTER_ATTRIBUTES.to_vec(),
    )
    .await;
    let entries = match entries {
        Ok(entries) => entries,
        Err(e) => {
            let _ = ldap.unbind().await;
            return Err(e);
        }
    };
    let _ = ldap.unbind().await;

    match entries.len() {
        0 => Err(AppError::ComputerNotFound(format!(
            "«{raw_query}» не найден в {base}. Проверьте имя; для машин вне \
             стандартных контейнеров укажите точный Base DN в настройках."
        ))),
        1 => {
            let info = laps_info_from_entry(&entries[0], credentials).await;
            ensure_password_readable(&info)?;
            info!(
                computer = %info.computer_name,
                flavor = info.flavor.audit_label(),
                "LAPS: карточка компьютера получена"
            );
            Ok(info)
        }
        _ => {
            let names: Vec<String> = entries
                .iter()
                .map(|entry| build_laps_info(entry).computer_name)
                .take(AMBIGUOUS_LIST_LIMIT)
                .collect();
            Err(AppError::ComputerAmbiguous(names.join(", ")))
        }
    }
}

/// Карточка компьютера обязана содержать читаемый пароль, кроме случаев
/// шифрованного хранения (там UI показывает диагностику и управление сроком).
/// Шифрованный вариант возвращаем карточкой с `password = None` -
/// это не ошибка: UI показывает диагностику и доступные операции.
fn ensure_password_readable(info: &LapsInfo) -> Result<(), AppError> {
    match info.flavor {
        model::LapsFlavor::Legacy
        | model::LapsFlavor::WindowsPlain
        | model::LapsFlavor::WindowsEncrypted => Ok(()),
        model::LapsFlavor::None => Err(AppError::NoLapsData(format!(
            "Компьютер {} найден, но пароля LAPS не видно: машина не управляется LAPS, \
             пароль ещё не сгенерирован, либо вашей учётной записи не делегировано \
             право чтения атрибутов LAPS (AD скрывает атрибуты без прав).",
            info.computer_name
        ))),
    }
}

/// Сброс/продление срока истечения пароля LAPS по DN объекта.
///
/// Возвращает свежую карточку компьютера (перечитанную после модификации),
/// чтобы UI показал актуальное состояние.
/// Текущее состояние объекта: какие атрибуты срока присутствуют.
/// После записи объект перечитывается: UI показывает фактическое состояние.
/// Объект исчез между операциями - крайне маловероятно, но контракт
/// требует карточку: возвращаем состояние «до».
pub async fn rotate_password(
    credentials: &DirectoryCredentials<'_>,
    dn: &str,
    mode: &RotationMode,
) -> Result<LapsInfo, AppError> {
    validate_dn(dn)?;

    let (mut ldap, _bind_ms) = open_session(credentials).await?;
    let url = credentials.ldap_url;

    let before = match search_base_computer(&mut ldap, url, dn).await {
        Ok(entries) => entries,
        Err(e) => {
            let _ = ldap.unbind().await;
            return Err(e);
        }
    };
    let entry = before.into_iter().next().ok_or_else(|| {
        AppError::ComputerNotFound(format!("объект {dn} не найден или перемещён"))
    })?;
    let info_before = laps_info_from_entry(&entry, credentials).await;

    let mods = match build_rotation_mods(&info_before, mode) {
        Ok(mods) => mods,
        Err(e) => {
            let _ = ldap.unbind().await;
            return Err(e);
        }
    };

    debug!(dn, mods = mods.len(), "LDAP: модификация срока истечения");
    let result = ldap.with_timeout(OPERATION_TIMEOUT).modify(dn, mods).await;
    let result = match result {
        Ok(result) => result,
        Err(e) => {
            let _ = ldap.unbind().await;
            return Err(crate::services::ldap::classify_transport_error(url, &e));
        }
    };
    if result.rc != 0 {
        let _ = ldap.unbind().await;
        return Err(crate::services::ldap::classify_result(url, &result));
    }

    let after = match search_base_computer(&mut ldap, url, dn).await {
        Ok(entries) => entries,
        Err(e) => {
            let _ = ldap.unbind().await;
            return Err(e);
        }
    };
    let _ = ldap.unbind().await;

    let info = match after.into_iter().next() {
        Some(entry) => laps_info_from_entry(&entry, credentials).await,
        None => info_before,
    };
    info!(dn, mode = ?mode, "LAPS: срок истечения обновлён");
    Ok(info)
}

/// Диапазоны валидации DN объекта, приходящего с фронтенда.
fn validate_dn(dn: &str) -> Result<(), AppError> {
    let dn = dn.trim();
    if dn.is_empty() || dn.len() > 1024 {
        return Err(AppError::Validation(
            "Некорректный DN объекта компьютера".into(),
        ));
    }
    if dn.chars().any(|c| c.is_control()) {
        return Err(AppError::Validation(
            "DN объекта содержит недопустимые символы".into(),
        ));
    }
    // Компьютер в AD всегда CN=...; OU=/DC= не принимаем: команда меняет
    // атрибуты объекта по DN, и лишние формы - ненужная поверхность атаки.
    if !dn.to_ascii_lowercase().starts_with("cn=") {
        return Err(AppError::Validation(
            "DN объекта компьютера должен начинаться с CN=".into(),
        ));
    }
    Ok(())
}

/// Base-поиск объекта компьютера по DN.
async fn search_base_computer(
    ldap: &mut ldap3::Ldap,
    url: &str,
    dn: &str,
) -> Result<Vec<NormalizedEntry>, AppError> {
    search(
        ldap,
        url,
        dn,
        Scope::Base,
        "(&(objectCategory=computer)(objectClass=computer))",
        COMPUTER_ATTRIBUTES.to_vec(),
    )
    .await
}

/// База поиска: Base DN из настроек или `defaultNamingContext` из rootDSE.
pub(crate) async fn resolve_base_dn(
    ldap: &mut ldap3::Ldap,
    credentials: &DirectoryCredentials<'_>,
) -> Result<String, AppError> {
    let configured = credentials.base_dn.trim();
    if !configured.is_empty() {
        return Ok(configured.to_string());
    }
    let root = read_root_dse(ldap, credentials.ldap_url).await;
    root.default_naming_context.ok_or_else(|| {
        AppError::Config(
            "Не удалось определить корень домена: Base DN в настройках пуст, \
             а rootDSE сервера не вернул defaultNamingContext. \
             Укажите Base DN вручную (например, DC=company,DC=ru)."
                .into(),
        )
    })
}

/// Диагностика подключения: bind, rootDSE, пробная выборка компьютеров.
/// `auth_mode` - метка режима, которым выполнялась проверка
/// (`integrated` / `account` / `anonymous`): вызывающий знает сессию.
pub async fn test_connection(
    credentials: &DirectoryCredentials<'_>,
    auth_mode: &str,
) -> Result<ConnectionTest, AppError> {
    let (mut ldap, bind_ms) = open_session(credentials).await?;

    let root = read_root_dse(&mut ldap, credentials.ldap_url).await;

    let _ = ldap.unbind().await;
    Ok(ConnectionTest {
        bind_ms,
        server_dns_name: root.server_dns_name,
        default_naming_context: root.default_naming_context,
        auth_mode: auth_mode.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_domain_for_short_account_names() {
        assert_eq!(
            domain_from_base_dn("OU=Workstations,DC=company,DC=ru").as_deref(),
            Some("company.ru")
        );
        assert_eq!(domain_from_base_dn("").as_deref(), None);
        assert_eq!(
            domain_from_ldap_url("ldap://dc1.company.ru:389").as_deref(),
            Some("company.ru")
        );
        assert_eq!(
            domain_from_ldap_url("ldaps://dc1.corp.local").as_deref(),
            Some("corp.local")
        );
        assert_eq!(domain_from_ldap_url("ldap://10.0.0.5").as_deref(), None);
    }

    #[test]
    fn splits_bind_account_for_decryption_token() {
        assert_eq!(
            split_bind_account(r"COMPANY\ivanov"),
            (Some("COMPANY"), "ivanov")
        );
        assert_eq!(
            split_bind_account(r" COMPANY \ ivanov "),
            (Some("COMPANY"), "ivanov")
        );
        assert_eq!(
            split_bind_account("ivanov@company.ru"),
            (None, "ivanov@company.ru")
        );
        assert_eq!(split_bind_account("ivanov"), (None, "ivanov"));
        // Кривой backslash не должен давать пустой домен/пользователь.
        assert_eq!(split_bind_account(r"\ivanov"), (None, r"\ivanov"));
        assert_eq!(split_bind_account(r"COMPANY\"), (None, r"COMPANY\"));
    }

    #[test]
    fn builds_decryption_identity_only_for_explicit_login() {
        let explicit = DirectoryCredentials {
            ldap_url: "ldaps://dc1.company.ru",
            base_dn: "DC=company,DC=ru",
            bind_dn: Some(r"COMPANY\ivanov"),
            password: Some("secret"),
            use_start_tls: false,
            allow_invalid_tls: false,
            use_integrated_auth: false,
        };
        let identity = decrypt_identity(&explicit).expect("явная учётка");
        assert_eq!(identity.domain.as_deref(), Some("COMPANY"));
        assert_eq!(identity.user.as_str(), "ivanov");
        assert_eq!(identity.password.as_str(), "secret");

        let upn = DirectoryCredentials {
            bind_dn: Some("ivanov@company.ru"),
            ..explicit.clone()
        };
        let identity = decrypt_identity(&upn).expect("UPN");
        assert!(identity.domain.is_none());
        assert_eq!(identity.user.as_str(), "ivanov@company.ru");

        let sso = DirectoryCredentials {
            bind_dn: None,
            password: None,
            use_integrated_auth: true,
            ..explicit
        };
        assert!(decrypt_identity(&sso).is_none());
    }

    #[test]
    fn validates_computer_dn() {
        assert!(validate_dn("CN=WS01,OU=Workstations,DC=company,DC=ru").is_ok());
        assert!(matches!(validate_dn(""), Err(AppError::Validation(_))));
        assert!(matches!(validate_dn("WS01"), Err(AppError::Validation(_))));
        assert!(matches!(
            validate_dn("CN=WS01\n,DC=x"),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            validate_dn(&("CN=x".to_string() + &"y".repeat(2000))),
            Err(AppError::Validation(_))
        ));
        // Не CN= - не компьютер: OU=/DC= не принимаем.
        assert!(matches!(
            validate_dn("OU=Workstations,DC=company,DC=ru"),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            validate_dn("DC=company,DC=ru"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn readable_flavors_pass_and_none_is_error() {
        let base = LapsInfo {
            dn: "CN=WS01,DC=x".into(),
            computer_name: "WS01".into(),
            dns_host_name: None,
            operating_system: None,
            flavor: model::LapsFlavor::WindowsPlain,
            account_name: None,
            password: Some("p".into()),
            updated_at: None,
            expires_at: None,
            legacy_present: false,
            windows_plain_present: true,
            encrypted_present: false,
            rotation_possible: true,
            decrypted: false,
            decrypt_method: None,
            decrypt_error: None,
            win_expiration_present: true,
            legacy_expiration_present: false,
        };
        assert!(ensure_password_readable(&base).is_ok());

        let encrypted = LapsInfo {
            flavor: model::LapsFlavor::WindowsEncrypted,
            password: None,
            ..base.clone()
        };
        assert!(ensure_password_readable(&encrypted).is_ok());

        let none = LapsInfo {
            flavor: model::LapsFlavor::None,
            password: None,
            ..base
        };
        assert!(matches!(
            ensure_password_readable(&none),
            Err(AppError::NoLapsData(_))
        ));
    }
}
