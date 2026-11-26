//! Общее управляемое состояние приложения.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};
use zeroize::Zeroizing;

use crate::error::AppError;
use crate::services::audit::AuditLogger;

/// Сессия входа: живёт только в памяти процесса, не сохраняется на диск
/// и не переживает перезапуск приложения (требование безопасности:
/// аутентификация при каждом входе, учётные данные не запоминаются).
pub struct Session {
    /// Человекочитаемый субъект для журнала аудита.
    pub actor: String,
    /// `true` - интегрированная аутентификация (SSPI текущей сессии Windows);
    /// `false` - явная учётная запись, введённая при входе.
    pub integrated: bool,
    /// Учётная запись явного входа (bind identifier), если режим не SSO.
    pub account: Option<String>,
    /// Пароль явного входа. Хранится только в памяти, обёрнут в `Zeroizing`
    /// (память затирается при удалении), никогда не пишется в конфиг/журналы.
    pub password: Option<Zeroizing<String>>,
}

/// Общее состояние, доступное командам через `State<'_, AppState>`.
///
/// Приложение не держит пул LDAP-соединений: каждая операция открывает
/// собственное соединение с аутентификацией текущей сессии (SSPI не
/// поддерживает повторное использование чужих контекстов между
/// соединениями без кеширования кредов), поэтому в состоянии живут
/// только пути, журнал и сессия входа.
#[derive(Clone)]
pub struct AppState {
    /// Папка данных приложения: `config.json` и `audit.log`.
    pub config_dir: PathBuf,
    /// Журнал аудита операций с паролями LAPS.
    pub audit: AuditLogger,
    /// Сессия входа (`None` до успешной аутентификации).
    pub session: Arc<Mutex<Option<Session>>>,
}

impl AppState {
    /// Подготовка состояния: папка данных и журнал аудита.
    ///
    /// Журнал - «схема Minecraft»: прошлый запуск уходит в gzip-архив,
    /// новый запуск начинает чистый живой файл.
    pub fn init(app: &AppHandle) -> Result<Self, AppError> {
        let config_dir = resolve_config_dir(app)?;
        let audit = AuditLogger::new(config_dir.clone());
        audit.archive_previous_run();
        Ok(Self {
            audit,
            config_dir,
            session: Arc::new(Mutex::new(None)),
        })
    }
}

/// Имя папки данных - как у продукта, а не reverse-DNS identifier Tauri.
const DATA_DIR_NAME: &str = "KMAruda LAPS";

/// Каталог данных: `%APPDATA%\KMAruda LAPS` (на Windows).
///
/// Tauri по умолчанию строит `app_data_dir()` из `identifier`
/// (`%APPDATA%\com.kmaruda.laps`), поэтому путь задаём явно. Заодно
/// переносим туда данные предыдущих сборок, если новый каталог ещё не создан.
fn resolve_config_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let base = app.path().data_dir().map_err(|e| {
        AppError::Config(format!(
            "не удалось определить папку данных приложения: {e}"
        ))
    })?;
    let target = base.join(DATA_DIR_NAME);

    let legacy = app.path().app_data_dir().ok();
    if let Some(legacy) = legacy.filter(|path| *path != target && path.exists()) {
        if target.exists() {
            tracing::info!(
                legacy = %legacy.display(),
                target = %target.display(),
                "каталог данных уже существует - старую папку не трогаю"
            );
        } else {
            match fs::rename(&legacy, &target) {
                Ok(()) => tracing::info!(
                    from = %legacy.display(),
                    to = %target.display(),
                    "каталог данных перенесён"
                ),
                Err(rename_error) => {
                    tracing::warn!(
                        from = %legacy.display(),
                        to = %target.display(),
                        error = %rename_error,
                        "не удалось переименовать каталог данных - копирую"
                    );
                    // Копируем во временный каталог и переименовываем только
                    // после успеха: иначе оборванное копирование оставит
                    // неполный каталог, который следующий запуск примет за
                    // готовый и больше не мигрирует.
                    let staging = base.join(format!("{DATA_DIR_NAME}.migrating"));
                    if staging.exists() {
                        let _ = fs::remove_dir_all(&staging);
                    }
                    if let Err(copy_error) = copy_dir_all(&legacy, &staging) {
                        let _ = fs::remove_dir_all(&staging);
                        // Не блокируем запуск: продолжаем работать в старом
                        // каталоге и повторим миграцию при следующем старте.
                        tracing::warn!(
                            from = %legacy.display(),
                            to = %target.display(),
                            error = %copy_error,
                            "не удалось скопировать каталог данных - остаюсь в старом"
                        );
                        return Ok(legacy);
                    }
                    if let Err(move_error) = fs::rename(&staging, &target) {
                        let _ = fs::remove_dir_all(&staging);
                        if target.exists() {
                            tracing::warn!(
                                error = %move_error,
                                "каталог данных уже создан другим запуском - использую его"
                            );
                            return Ok(target);
                        }
                        tracing::warn!(
                            from = %legacy.display(),
                            to = %target.display(),
                            error = %move_error,
                            "не удалось завершить перенос данных - остаюсь в старом каталоге"
                        );
                        return Ok(legacy);
                    }
                    tracing::info!(
                        from = %legacy.display(),
                        to = %target.display(),
                        "каталог данных скопирован"
                    );
                }
            }
        }
    }

    fs::create_dir_all(&target).map_err(|e| {
        AppError::Config(format!(
            "не удалось создать папку данных {}: {e}",
            target.display()
        ))
    })?;
    Ok(target)
}

/// Рекурсивное копирование для случая, когда rename недоступен
/// (например, каталоги на разных томах).
fn copy_dir_all(source: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let destination = target.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &destination)?;
        } else {
            fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}
