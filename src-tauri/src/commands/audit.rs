//! Команды журнала аудита: чтение страницы и открытие папки журнала.

use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::AppError;
use crate::services::audit::AuditPage;
use crate::state::AppState;

use super::run_blocking;

/// Страница журнала (новые записи сверху).
#[tauri::command]
pub async fn list_audit(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> Result<AuditPage, AppError> {
    let logger = state.audit.clone();
    run_blocking(move || logger.list(limit, offset)).await
}

/// Открывает папку журнала в системном проводнике.
///
/// Если файла `audit.log` ещё нет, всё равно открываем каталог данных:
/// кнопка «Папка журнала» не должна молча ничего не делать.
#[tauri::command]
pub async fn open_audit_folder(app: AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let path = state.audit.log_path();
    let dir = path
        .parent()
        .ok_or_else(|| AppError::Internal("у журнала нет родительского каталога".into()))?;

    std::fs::create_dir_all(dir)
        .map_err(|e| AppError::Internal(format!("не удалось создать папку журнала: {e}")))?;

    let opener = app.opener();
    let dir_text = dir.to_string_lossy().into_owned();
    let result = if path.exists() {
        opener
            .reveal_item_in_dir(&path)
            .or_else(|_| opener.open_path(dir_text.clone(), None::<String>))
    } else {
        opener.open_path(dir_text, None::<String>)
    };

    result.map_err(|e| AppError::Internal(format!("не удалось открыть папку журнала: {e}")))
}
