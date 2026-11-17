//! Нативная дешифровка `msLAPS-EncryptedPassword` (Windows LAPS).
//!
//! Windows LAPS хранит пароль как DPAPI-NG-блок (CNG group protection):
//! CMS `EnvelopedData`, контентный ключ которого обёрнут под KEK,
//! выведенный из **KDS root key** домена. Блок не содержит ключа — только
//! идентификатор ключа; разрешённый субъект получает нужный узел
//! L0/L1/L2-дерева у контроллера домена по MS-GKDI и выводит KEK вниз.
//!
//! Дешифровка идёт **тем же системным API, что и у штатного модуля Windows LAPS**:
//! `lapsutil.dll!DecryptNormalMode`. Это принципиально:
//! * `DecryptNormalMode` понимает формат конверта Windows LAPS и сам
//!   выполняет MS-GKDI-запрос, не требуя от нас ASN.1/RPC;
//! * для входа явной учётной записью `LogonWithCredentials` создаёт
//!   Windows-токен этой учётной записи, и KDS-дешифровка выполняется
//!   **от её имени**, а не от имени процесса. Старая PowerShell-версия
//!   работала именно так (запускала дочерний процесс под введённой
//!   учёткой), поэтому и показывала пароль.
//!
//! Если `lapsutil.dll` недоступен (очень старая Windows без Windows LAPS),
//! остаётся запасной путь `NCryptUnprotectSecret` — но только для
//! SSO-сессии, где процесс и так работает под нужной учётной записью.
//! Для явной учётной записи fallback намеренно запрещён: иначе пароль
//! мог бы быть расшифрован под другим субъектом, чем выбрал оператор.

/// Учётная запись, под которой нужно выполнить KDS-дешифровку.
///
/// Отличается от учётных данных LDAP-bind: bind может идти под сервисной
/// учёткой, а KDS проверяет именно Windows-идентичность. Поэтому токен
/// создаётся отдельно через `LogonWithCredentials`.
///
/// Владение строками нужно, чтобы вынести блокирующий вызов `lapsutil`
/// в `spawn_blocking`: пароль копируется в `Zeroizing` и затирается при
/// уничтожении структуры.
///
/// На не-Windows сборке структура только создаётся (для честной валидации
/// сессии), но не читается: дешифровка там недоступна.
#[cfg_attr(not(windows), allow(dead_code))]
pub struct DecryptIdentity {
    /// NetBIOS-домен (`COMPANY`). `None` — UPN/локальная учётная запись;
    /// для короткого имени домен подставляется из `USERDOMAIN`, если
    /// приложение запущено под доменной учёткой.
    pub domain: Option<String>,
    /// `user` или `user@company.ru`.
    pub user: String,
    /// Копия пароля сессии, затираемая при drop.
    pub password: zeroize::Zeroizing<String>,
}

/// Расшифровывает DPAPI-NG-блок и разбирает JSON-пароль Windows LAPS.
///
/// `identity = None` — SSO/текущая учётная запись процесса.
/// `identity = Some(...)` — явный вход: создаётся токен указанной учётной
/// записи, и дешифровка выполняется под ним.
///
/// Ошибка возвращается короткой технической строкой без секретов:
/// код Win32/HRESULT и понятная причина. UI показывает её как диагностику,
/// а не как отказ операции: карточка компьютера остаётся доступной.
#[cfg(windows)]
pub fn decrypt_windows_laps_password(
    candidates: &[&[u8]],
    identity: Option<&DecryptIdentity>,
) -> Result<super::parse::WindowsPlainPassword, String> {
    match LapsUtil::load() {
        Ok(laps) => {
            let token = match identity {
                Some(identity) => Some(laps.logon(identity)?),
                None => None,
            };

            let mut last_error = "нет кандидатов на расшифровку".to_string();
            for (index, blob) in candidates.iter().enumerate() {
                match laps.decrypt(token.as_ref(), blob) {
                    Ok(plain) => {
                        tracing::debug!(
                            candidate = index + 1,
                            bytes = blob.len(),
                            "LAPS: кандидат заголовка расшифрован"
                        );
                        return Ok(plain);
                    }
                    Err(error) => {
                        last_error =
                            format!("вариант {} ({} байт): {error}", index + 1, blob.len());
                    }
                }
            }

            // SSO: та же учётная запись процесса, можно попробовать
            // второй системный API. Явный вход — только его токен.
            if identity.is_none() {
                for (index, blob) in candidates.iter().enumerate() {
                    match decrypt_with_ncrypt(blob) {
                        Ok(plain) => return Ok(plain),
                        Err(error) => {
                            last_error = format!(
                                "{last_error}; NCrypt вариант {} ({} байт): {error}",
                                index + 1,
                                blob.len()
                            );
                        }
                    }
                }
            }
            Err(last_error)
        }
        Err(load_error) => {
            if identity.is_some() {
                Err(format!(
                    "{load_error}; для дешифровки под явной учётной записью \
                     требуется модуль Windows LAPS"
                ))
            } else {
                let mut last_error = format!("{load_error}; NCrypt:");
                for (index, blob) in candidates.iter().enumerate() {
                    match decrypt_with_ncrypt(blob) {
                        Ok(plain) => return Ok(plain),
                        Err(error) => {
                            last_error.push_str(&format!(
                                " вариант {} ({} байт): {error};",
                                index + 1,
                                blob.len()
                            ));
                        }
                    }
                }
                Err(last_error)
            }
        }
    }
}

/// На не-Windows дешифровка Windows LAPS недоступна: приложение целевым
/// образом работает в доменной сети Windows. Карточка компьютера при
/// этом остаётся доступной (пароль не показывается).
#[cfg(not(windows))]
pub fn decrypt_windows_laps_password(
    _candidates: &[&[u8]],
    _identity: Option<&DecryptIdentity>,
) -> Result<super::parse::WindowsPlainPassword, String> {
    Err("дешифровка Windows LAPS доступна только в Windows-сборке".to_string())
}

// ── Windows LAPS native API ──────────────────────────────────────────────────

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::path::PathBuf;
    use std::ptr::null_mut;

    use windows::core::{s, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{
        CloseHandle, FreeLibrary, LocalFree, HANDLE, HLOCAL, HMODULE,
    };
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::Win32::System::SystemInformation::GetWindowsDirectoryW;

    use zeroize::Zeroize;

    use super::DecryptIdentity;

    /// `DecryptNormalMode` из lapsutil.dll: (token, blob, len, flags, out, out_len).
    type DecryptNormalModeFn =
        unsafe extern "system" fn(HANDLE, *const u8, u32, u32, *mut *mut u8, *mut u32) -> u32;

    /// `LogonWithCredentials` из lapsutil.dll: (domain, user, password, out_token).
    type LogonWithCredentialsFn =
        unsafe extern "system" fn(PCWSTR, PCWSTR, PCWSTR, *mut HANDLE) -> u32;

    /// Токен учётной записи; закрывается при выходе из области видимости.
    pub(super) struct Token(pub(super) HANDLE);

    impl Drop for Token {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
    }

    /// Загруженный `lapsutil.dll` с нужными экспортами.
    pub(super) struct LapsUtil {
        module: HMODULE,
        decrypt: DecryptNormalModeFn,
        logon: LogonWithCredentialsFn,
    }

    impl Drop for LapsUtil {
        fn drop(&mut self) {
            if !self.module.is_invalid() {
                unsafe {
                    let _ = FreeLibrary(self.module);
                }
            }
        }
    }

    impl LapsUtil {
        /// Загружает системный модуль Windows LAPS.
        ///
        /// Путь фиксирован: `%SystemRoot%\System32\WindowsPowerShell\v1.0\
        /// Modules\LAPS\lapsutil.dll`. DLL подписан Microsoft и лежит в
        /// защищённой системной папке; поиск по имени не используем, чтобы
        /// исключить подмену через рабочую папку.
        pub(super) fn load() -> Result<Self, String> {
            let path = lapsutil_path();
            if !path.is_file() {
                return Err(format!("модуль Windows LAPS не найден: {}", path.display()));
            }

            let module = unsafe { LoadLibraryW(PCWSTR(HSTRING::from(path.as_os_str()).as_ptr())) }
                .map_err(|e| format!("LoadLibrary({}): {}", path.display(), e))?;

            let decrypt = match symbol::<DecryptNormalModeFn>(module, s!("DecryptNormalMode")) {
                Ok(function) => function,
                Err(error) => {
                    unsafe {
                        let _ = FreeLibrary(module);
                    }
                    return Err(format!("{}: {}", path.display(), error));
                }
            };
            let logon = match symbol::<LogonWithCredentialsFn>(module, s!("LogonWithCredentials")) {
                Ok(function) => function,
                Err(error) => {
                    unsafe {
                        let _ = FreeLibrary(module);
                    }
                    return Err(format!("{}: {}", path.display(), error));
                }
            };

            Ok(Self {
                module,
                decrypt,
                logon,
            })
        }

        /// Создаёт Windows-токен явной учётной записи.
        ///
        /// Пароль передаётся в UTF-16-буфере и затирается сразу после
        /// вызова. Токен закрывается `Token::drop`.
        pub(super) fn logon(&self, identity: &DecryptIdentity) -> Result<Token, String> {
            let user = wide(&identity.user);
            let domain = identity.domain.as_deref().map(wide);
            let mut password = wide(identity.password.as_str());
            let mut token = HANDLE::default();

            let code = unsafe {
                (self.logon)(
                    domain
                        .as_ref()
                        .map(|value| PCWSTR(value.as_ptr()))
                        .unwrap_or(PCWSTR::null()),
                    PCWSTR(user.as_ptr()),
                    PCWSTR(password.as_ptr()),
                    &mut token,
                )
            };
            password.zeroize();

            if code != 0 {
                // Нативный вызов мог успеть вернуть handle даже с ошибкой.
                if !token.is_invalid() {
                    unsafe {
                        let _ = CloseHandle(token);
                    }
                }
                return Err(format!("LogonWithCredentials: {}", win32_error(code)));
            }
            if token.is_invalid() {
                return Err("LogonWithCredentials вернул пустой токен".to_string());
            }
            Ok(Token(token))
        }

        /// Расшифровывает внутренний DPAPI-NG-блок.
        ///
        /// `flags = 0`: обычный режим (не recovery, не local KDS). Вывод —
        /// UTF-16LE JSON; буфер принадлежит нам и освобождается `LocalFree`.
        pub(super) fn decrypt(
            &self,
            token: Option<&Token>,
            blob: &[u8],
        ) -> Result<super::super::parse::WindowsPlainPassword, String> {
            if blob.len() > u32::MAX as usize {
                return Err("блок LAPS слишком велик".to_string());
            }

            let mut out: *mut u8 = null_mut();
            let mut out_len: u32 = 0;
            let code = unsafe {
                (self.decrypt)(
                    token.map(|value| value.0).unwrap_or_default(),
                    blob.as_ptr(),
                    blob.len() as u32,
                    0,
                    &mut out,
                    &mut out_len,
                )
            };

            if code != 0 {
                free_output(out);
                return Err(format!("DecryptNormalMode: {}", laps_error(code)));
            }
            if out.is_null() || out_len == 0 {
                free_output(out);
                return Err("DecryptNormalMode вернул пустой результат".to_string());
            }

            // SAFETY: при успехе API выделил out ровно на out_len байт.
            let plaintext = unsafe { std::slice::from_raw_parts_mut(out, out_len as usize) };
            let parsed = super::super::parse::parse_windows_plain_payload(plaintext);
            plaintext.zeroize();
            free_output(out);

            parsed.ok_or_else(|| "расшифрованный блок не является JSON LAPS".to_string())
        }
    }

    fn symbol<T: Copy>(module: HMODULE, name: windows::core::PCSTR) -> Result<T, String> {
        let Some(proc) = (unsafe { GetProcAddress(module, name) }) else {
            return Err(format!("экспорт {:?} не найден", name));
        };
        // SAFETY: адрес получен из GetProcAddress и приводится к сигнатуре,
        // объявленной в lapsutil.dll (совпадает с P/Invoke из lapspsh.dll).
        Ok(unsafe { std::mem::transmute_copy(&proc) })
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn lapsutil_path() -> PathBuf {
        // Берём каталог Windows у самой ОС, а не из переменной окружения:
        // подменённый SystemRoot не должен влиять на путь загрузки DLL.
        let root = windows_directory();
        root.join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("Modules")
            .join("LAPS")
            .join("lapsutil.dll")
    }

    fn windows_directory() -> PathBuf {
        let mut buffer = [0u16; 260];
        // SAFETY: буфер фиксированного размера; API пишет NUL-terminated
        // строку и возвращает длину без NUL.
        let length = unsafe { GetWindowsDirectoryW(Some(&mut buffer)) } as usize;
        if length == 0 || length >= buffer.len() {
            return PathBuf::from(r"C:\Windows");
        }
        PathBuf::from(String::from_utf16_lossy(&buffer[..length]))
    }

    fn free_output(pointer: *mut u8) {
        if !pointer.is_null() {
            unsafe {
                let _ = LocalFree(Some(HLOCAL(pointer.cast::<c_void>())));
            }
        }
    }

    /// Win32-код `LogonWithCredentials` -> понятная причина.
    fn win32_error(code: u32) -> String {
        let text = std::io::Error::from_raw_os_error(code as i32).to_string();
        format!("0x{code:08X} ({text})")
    }

    /// Код `DecryptNormalMode` -> понятная причина.
    ///
    /// 0x8009002C — NTE_NOT_FOUND/«нет доступа к ключу»: именно этот код
    /// cmdlet трактует как Unauthorized (учётка не имеет права KDS).
    fn laps_error(code: u32) -> String {
        let hint = match code {
            0x8009_002C => "учётная запись не авторизована на дешифровку LAPS (KDS)",
            0x8007_0005 => "доступ запрещён (нет прав на ключ KDS или атрибут LAPS)",
            0x8007_052E => "неверный пароль учётной записи дешифровки",
            _ => "ошибка Windows LAPS",
        };
        format!("0x{code:08X} ({hint})")
    }
}

#[cfg(windows)]
use win::LapsUtil;

// ── Запасной путь: NCryptUnprotectSecret ─────────────────────────────────────

/// Прямой вызов DPAPI-NG через CNG. Оставлен для очень старых сборок,
/// где `lapsutil.dll` отсутствует, и для SSO. В отличие от LAPS API,
/// не умеет работать под отдельным токеном — только под процессом.
#[cfg(windows)]
fn decrypt_with_ncrypt(blob: &[u8]) -> Result<super::parse::WindowsPlainPassword, String> {
    use windows::Win32::Security::Cryptography::{
        NCryptFreeBuffer, NCryptUnprotectSecret, NCRYPT_SILENT_FLAG,
    };

    let mut plain_ptr: *mut u8 = std::ptr::null_mut();
    let mut plain_len: u32 = 0;

    let status = unsafe {
        NCryptUnprotectSecret(
            None,
            NCRYPT_SILENT_FLAG,
            blob,
            None,
            None,
            &mut plain_ptr,
            &mut plain_len,
        )
    };

    if let Err(error) = status {
        if !plain_ptr.is_null() {
            unsafe {
                let _ = NCryptFreeBuffer(plain_ptr.cast());
            }
        }
        return Err(format!(
            "NCryptUnprotectSecret: HRESULT 0x{:08X}",
            error.code().0
        ));
    }
    if plain_ptr.is_null() || plain_len == 0 {
        if !plain_ptr.is_null() {
            unsafe {
                let _ = NCryptFreeBuffer(plain_ptr.cast());
            }
        }
        return Err("NCryptUnprotectSecret: пустой результат".to_string());
    }

    let plaintext = unsafe { std::slice::from_raw_parts_mut(plain_ptr, plain_len as usize) };
    let parsed = super::parse::parse_windows_plain_payload(plaintext);
    {
        use zeroize::Zeroize;
        plaintext.zeroize();
    }
    unsafe {
        let _ = NCryptFreeBuffer(plain_ptr.cast());
    }

    parsed.ok_or_else(|| "расшифрованный блок не является JSON LAPS".to_string())
}

#[cfg(all(windows, test))]
mod tests {
    /// Проверяет, что на машине с Windows LAPS загружается lapsutil.dll
    /// и находятся оба экспорта. Если модуля нет (старая Windows), тест
    /// не падает: приложение тогда использует NCrypt-fallback для SSO.
    #[test]
    fn loads_lapsutil_exports_when_installed() {
        match super::win::LapsUtil::load() {
            Ok(_) => {}
            Err(error) if error.contains("не найден") => {}
            Err(error) => panic!("lapsutil.dll найден, но не загрузился: {error}"),
        }
    }
}
