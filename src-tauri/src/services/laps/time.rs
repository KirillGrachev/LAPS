//! Преобразования времени Active Directory.
//!
//! AD хранит моменты времени в формате **FILETIME**: число 100-наносекундных
//! интервалов с 1 января 1601 г. UTC. В LAPS встречаются три его представления:
//! * `ms-Mcs-AdmPwdExpirationTime` (legacy) и `msLAPS-PasswordExpirationTime`
//!   (Windows LAPS) - **десятичная** строка, например `133600000000000000`;
//! * поле `t` JSON-объекта `msLAPS-Password` - **шестнадцатеричная** строка
//!   без префикса, например `1d8161b41c41cde` (см. MS LAPS schema reference).
//!
//! Фронтенд получает/отправляет время в RFC 3339 с локальным смещением
//! (`2026-10-07T12:00:00+03:00`) и наивные локальные строки из datetime-пикера
//! (`2026-10-07T12:00`).

use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset};

/// Разница между эпохой FILETIME (1601-01-01) и Unix-эпохой в 100-нс интервалах
/// (11 644 473 600 секунд × 10⁷).
const FILETIME_UNIX_EPOCH_DIFF: i64 = 116_444_736_000_000_000;
const TICKS_PER_SECOND: i64 = 10_000_000;

/// FILETIME -> UTC-момент. Отклоняет вырожденные значения (0, отрицательные,
/// за пределами представимого диапазона).
pub fn filetime_to_utc(filetime: i64) -> Option<OffsetDateTime> {
    if filetime <= 0 {
        return None;
    }
    let secs = filetime
        .checked_sub(FILETIME_UNIX_EPOCH_DIFF)?
        .div_euclid(TICKS_PER_SECOND);
    let nanos = (filetime.rem_euclid(TICKS_PER_SECOND) * 100) as u32;
    OffsetDateTime::from_unix_timestamp(secs)
        .ok()?
        .replace_nanosecond(nanos)
        .ok()
}

/// UTC-момент -> FILETIME.
pub fn utc_to_filetime(datetime: OffsetDateTime) -> i64 {
    datetime.unix_timestamp() * TICKS_PER_SECOND
        + FILETIME_UNIX_EPOCH_DIFF
        + (datetime.nanosecond() / 100) as i64
}

/// Разбор десятичного FILETIME из строковых атрибутов AD.
/// Пробелы и нецифровые хвосты не допускаются - только целое число.
pub fn parse_filetime_decimal(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    trimmed.parse::<i64>().ok()
}

/// Разбор шестнадцатеричного FILETIME (поле `t` в JSON `msLAPS-Password`).
pub fn parse_filetime_hex(raw: &str) -> Option<i64> {
    let trimmed = raw.trim().trim_start_matches("0x").trim_start_matches("0X");
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    i64::from_str_radix(trimmed, 16).ok()
}

/// Наивная локальная строка (`2026-10-07T12:00[:00]`) -> FILETIME.
/// Неоднозначные/несуществующие локальные моменты (перевод часов) отклоняются.
pub fn parse_local_naive_to_filetime(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    let naive = PrimitiveDateTime::parse(
        trimmed,
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]"),
    )
    .or_else(|_| {
        PrimitiveDateTime::parse(
            trimmed,
            format_description!("[year]-[month]-[day]T[hour]:[minute]"),
        )
    })
    .or_else(|_| {
        PrimitiveDateTime::parse(
            trimmed,
            format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"),
        )
    })
    .or_else(|_| {
        PrimitiveDateTime::parse(
            trimmed,
            format_description!("[year]-[month]-[day] [hour]:[minute]"),
        )
    })
    .ok()?;

    // Смещение подбирается вокруг предполагаемого момента: `local_offset_at`
    // принимает UTC-инстант, а не гражданское время. Несуществующее время
    // (переход на летнее время) и неоднозначное (обратный переход)
    // отклоняются, как раньше делал `chrono::Local::from_local_datetime`.
    let local = local_datetime_to_utc(naive)?;
    Some(utc_to_filetime(local))
}

/// Наивный локальный момент -> UTC-момент с проверкой смещения.
fn local_datetime_to_utc(naive: PrimitiveDateTime) -> Option<OffsetDateTime> {
    let base = naive.assume_utc().unix_timestamp();
    let mut resolved: Option<OffsetDateTime> = None;
    for delta in [0_i64, -3600, 3600, -7200, 7200] {
        let Ok(instant) = OffsetDateTime::from_unix_timestamp(base + delta) else {
            continue;
        };
        let Ok(offset) = UtcOffset::local_offset_at(instant) else {
            continue;
        };
        let candidate = naive.assume_offset(offset);
        let Ok(roundtrip) = UtcOffset::local_offset_at(candidate.to_offset(UtcOffset::UTC)) else {
            continue;
        };
        if roundtrip != offset {
            continue;
        }
        match resolved {
            None => resolved = Some(candidate),
            Some(existing) if existing == candidate => {}
            // Два разных смещения дают один гражданский момент: время
            // неоднозначно, записывать произвольное нельзя.
            Some(_) => return None,
        }
    }
    resolved
}

/// FILETIME -> RFC 3339 в локальном часовом поясе (контракт фронтенда).
pub fn filetime_to_rfc3339_local(filetime: i64) -> Option<String> {
    let utc = filetime_to_utc(filetime)?;
    let offset = UtcOffset::local_offset_at(utc).ok()?;
    utc.to_offset(offset).format(&Rfc3339).ok()
}

/// Текущий момент в FILETIME.
pub fn now_filetime() -> i64 {
    utc_to_filetime(OffsetDateTime::now_utc())
}

/// Текущий момент в RFC 3339 с локальным смещением (для аудита).
pub fn now_rfc3339_local() -> String {
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    now.format(&Rfc3339).unwrap_or_default()
}

/// Читаемый вид времени аудита: `2026-10-05 13:42:07.123 +03:00`.
pub fn pretty_timestamp(rfc: &str) -> String {
    let Ok(dt) = OffsetDateTime::parse(rfc, &Rfc3339) else {
        return rfc.to_string();
    };
    dt.format(format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3] [offset_hour sign:mandatory]:[offset_minute]"
    ))
    .unwrap_or_else(|_| rfc.to_string())
}

/// Читаемый вид -> RFC 3339 (для контракта UI).
pub fn machine_timestamp(pretty: &str) -> Option<String> {
    let dt = OffsetDateTime::parse(
        pretty,
        &format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3] [offset_hour sign:mandatory]:[offset_minute]"
        ),
    )
    .ok()?;
    dt.format(format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3][offset_hour sign:mandatory]:[offset_minute]"
    ))
    .ok()
}

/// Метка для имени gzip-архива аудита: `2026-10-05-134207`.
pub fn archive_stamp_local() -> String {
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    now.format(format_description!(
        "[year]-[month]-[day]-[hour][minute][second]"
    ))
    .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    /// 2023-04-11T00:00:00Z (день выхода Windows LAPS) - эталонное значение.
    fn converts_known_filetime_values() {
        let reference = datetime!(2023-04-11 00:00:00 UTC);
        let ft = utc_to_filetime(reference);
        assert_eq!(ft, 133_256_448_000_000_000);
        assert_eq!(filetime_to_utc(ft), Some(reference));
    }

    #[test]
    /// Точность FILETIME - 100 нс; наносекундный остаток усекается.
    fn roundtrips_subsecond_precision() {
        let moment = datetime!(2026-10-05 12:34:56.1234567 UTC);
        let ft = utc_to_filetime(moment);
        let back = filetime_to_utc(ft).expect("обратная конвертация");
        assert_eq!(back.unix_timestamp(), moment.unix_timestamp());
        assert!((back.nanosecond() as i64 - moment.nanosecond() as i64).abs() < 100);
    }

    #[test]
    fn rejects_degenerate_filetimes() {
        assert_eq!(filetime_to_utc(0), None);
        assert_eq!(filetime_to_utc(-1), None);
    }

    #[test]
    fn parses_decimal_filetime_strings() {
        assert_eq!(
            parse_filetime_decimal("133600000000000000"),
            Some(133_600_000_000_000_000)
        );
        assert_eq!(
            parse_filetime_decimal(" 133600000000000000 "),
            Some(133_600_000_000_000_000)
        );
        assert_eq!(parse_filetime_decimal(""), None);
        assert_eq!(parse_filetime_decimal("133abc"), None);
        assert_eq!(parse_filetime_decimal("-133"), None);
        assert_eq!(parse_filetime_decimal("99999999999999999999999"), None);
    }

    #[test]
    /// Значение из документации MS LAPS: 1d8161b41c41cde.
    fn parses_hex_filetime_strings() {
        let ft = parse_filetime_hex("1d8161b41c41cde").expect("hex parse");
        assert!(ft > 130_000_000_000_000_000 && ft < 140_000_000_000_000_000);
        assert!(filetime_to_utc(ft).is_some());
        assert_eq!(
            parse_filetime_hex("0x1d8161b41c41cde"),
            parse_filetime_hex("1d8161b41c41cde")
        );
        assert_eq!(parse_filetime_hex("zzz"), None);
        assert_eq!(parse_filetime_hex(""), None);
    }

    #[test]
    /// Форматы datetime-пикера и ручного ввода.
    fn parses_local_naive_datetime_strings() {
        assert!(parse_local_naive_to_filetime("2026-10-07T12:00").is_some());
        assert!(parse_local_naive_to_filetime("2026-10-07T12:00:30").is_some());
        assert!(parse_local_naive_to_filetime("2026-10-07 12:00").is_some());
        assert!(parse_local_naive_to_filetime("07.10.2026 12:00").is_none());
        assert!(parse_local_naive_to_filetime("2026-13-45T99:99").is_none());
    }

    #[test]
    /// RFC 3339 с локальным смещением - date-fns parseISO читает его как есть.
    fn formats_expiration_for_frontend() {
        let ft = 133_256_448_000_000_000;
        let formatted = filetime_to_rfc3339_local(ft).expect("format");
        assert!(formatted.starts_with("2023-04-1"), "{formatted}");
        assert!(formatted.contains('T'));
    }
}
