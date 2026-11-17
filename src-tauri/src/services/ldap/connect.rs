//! Установка соединения и аутентификация (StartTLS/LDAPS, GSSAPI/SSPI).
//!
//! Режимы аутентификации (по убыванию приоритета):
//! 1. **Интегрированная** — SASL GSSAPI под текущей сессией Windows
//!    (Kerberos/NTLM через SSPI). Пароль не вводится и не хранится:
//!    инженер работает под своей доменной учёткой, права чтения
//!    LAPS-атрибутов делегируются группе поддержки на стороне AD.
//! 2. **Простой bind** — учётная запись, введённая на экране входа
//!    (DN/UPN/sAMAccountName + пароль из памяти сессии).
//! 3. **Анонимный** — только если учётка не задана (современные домены
//!    анонимный поиск запрещают; ошибка классифицируется в `errors.rs`).

use ldap3::{LdapConnAsync, LdapConnSettings};
use tracing::{debug, info};

use crate::error::AppError;

use super::errors::{auth_error_text, classify_error};
use super::model::{DirectoryCredentials, CONNECT_TIMEOUT, OPERATION_TIMEOUT};

pub(super) fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Открывает LDAP-соединение с заданными настройками.
///
/// Драйвер соединения должен жить всё время работы с LDAP.
pub(super) async fn connect(
    credentials: &DirectoryCredentials<'_>,
) -> Result<ldap3::Ldap, AppError> {
    let url = credentials.ldap_url.trim();
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("ldap://") || lower.starts_with("ldaps://")) {
        return Err(AppError::Validation(
            "Адрес сервера должен начинаться с ldap:// или ldaps://".into(),
        ));
    }
    let is_ldap = lower.starts_with("ldap://");

    // Пароли LAPS и учётные данные не должны идти по открытому LDAP.
    // Для ldap:// StartTLS включается автоматически; ldaps:// уже шифрован.
    let start_tls = credentials.use_start_tls || is_ldap;
    if !is_secure_transport(&lower, start_tls) {
        return Err(AppError::LdapInsecure(format!(
            "{url}: соединение не шифруется. Используйте ldaps://<dc>:636 \
             или StartTLS (Настройки → Active Directory)."
        )));
    }

    // Схема в URL нормализована валидацией конфигурации, но сессия может
    // прийти и из старого config.json — приводим её здесь ещё раз.
    let normalized = {
        let (_, rest) = url.split_once("://").unwrap_or(("", url));
        if lower.starts_with("ldaps://") {
            format!("ldaps://{rest}")
        } else {
            format!("ldap://{rest}")
        }
    };

    let mut settings = LdapConnSettings::new().set_conn_timeout(CONNECT_TIMEOUT);
    if start_tls && is_ldap {
        settings = settings.set_starttls(true);
    }
    if credentials.allow_invalid_tls {
        settings = settings.set_no_tls_verify(true);
    }

    let (conn, ldap) = LdapConnAsync::with_settings(settings, &normalized)
        .await
        .map_err(|e| classify_error(url, &e))?;

    ldap3::drive!(conn);

    if start_tls && is_ldap {
        info!(url, "StartTLS включён (автоматически для ldap://)");
    }
    Ok(ldap)
}

/// Шифруется ли транспорт: `ldaps://` или `ldap://` + StartTLS.
fn is_secure_transport(url: &str, use_start_tls: bool) -> bool {
    url.starts_with("ldaps://") || (url.starts_with("ldap://") && use_start_tls)
}

/// Извлекает FQDN хоста из LDAP-URL (нужен для SPN `ldap/<fqdn>` в GSSAPI).
#[cfg(any(windows, test))]
pub(super) fn extract_host_fqdn(url: &str) -> String {
    let without_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host_port = without_scheme.split('/').next().unwrap_or(without_scheme);
    host_port.split(':').next().unwrap_or(host_port).to_string()
}

/// Аутентификация соединения. Возвращает длительность bind, мс.
///
/// На Windows — Kerberos/NTLM-билет текущего вошедшего в систему пользователя (SSPI).
pub(super) async fn bind(
    ldap: &mut ldap3::Ldap,
    credentials: &DirectoryCredentials<'_>,
) -> Result<u64, AppError> {
    let started = std::time::Instant::now();

    if credentials.use_integrated_auth {
        #[cfg(windows)]
        {
            let fqdn = extract_host_fqdn(credentials.ldap_url);
            debug!(fqdn = %fqdn, "LDAP: интегрированная аутентификация (GSSAPI/SSPI)");
            let result = ldap
                .with_timeout(OPERATION_TIMEOUT)
                .sasl_gssapi_bind(&fqdn)
                .await
                .map_err(|e| classify_error(credentials.ldap_url, &e))?;
            if result.rc != 0 {
                return Err(AppError::LdapAuth(format!(
                    "домен отклонил интегрированную аутентификацию текущего пользователя Windows: \
                     код {} ({})",
                    result.rc,
                    result.text.trim()
                )));
            }
            return Ok(started.elapsed().as_millis() as u64);
        }
        #[cfg(not(windows))]
        {
            let _ = ldap;
            return Err(AppError::Validation(
                "Аутентификация под текущей сессией Windows поддерживается только в Windows-сборке. \
                 Отключите её в настройках и укажите учётную запись с паролем."
                    .into(),
            ));
        }
    }

    let (Some(bind_dn), Some(password)) = (
        credentials.bind_dn.and_then(non_empty),
        credentials.password.and_then(non_empty),
    ) else {
        debug!("LDAP: анонимный bind");
        return Ok(0);
    };

    let result = ldap
        .with_timeout(OPERATION_TIMEOUT)
        .simple_bind(bind_dn, password)
        .await
        .map_err(|e| classify_error(credentials.ldap_url, &e))?;
    if result.rc != 0 {
        return Err(AppError::LdapAuth(auth_error_text(result.rc, &result.text)));
    }
    let elapsed = started.elapsed().as_millis() as u64;
    debug!(elapsed_ms = elapsed, "LDAP: bind выполнен");
    Ok(elapsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_encrypted_transport_for_credentials() {
        assert!(is_secure_transport("ldaps://dc1.company.ru:636", false));
        assert!(is_secure_transport("ldaps://dc1.company.ru", true));
        assert!(is_secure_transport("ldap://dc1.company.ru", true));
        assert!(!is_secure_transport("ldap://dc1.company.ru", false));
        assert!(!is_secure_transport("ldap://dc1.company.ru:389", false));
    }

    #[test]
    fn extracts_host_fqdn_from_ldap_urls() {
        assert_eq!(extract_host_fqdn("ldap://dc1.company.ru"), "dc1.company.ru");
        assert_eq!(
            extract_host_fqdn("ldaps://dc1.company.ru:636"),
            "dc1.company.ru"
        );
        assert_eq!(extract_host_fqdn("ldap://10.0.0.5:389/"), "10.0.0.5");
    }
}
