//! Небольшое сохранение размера/позиции главного окна.
//!
//! Раньше это делал `tauri-plugin-window-state`, но он писал в
//! `app_config_dir()` от `identifier` (`%APPDATA%\com.kmaruda.laps`) и
//! возвращал старую папку. Здесь состояние лежит рядом с `config.json`
//! в `%APPDATA%\KMAruda LAPS`.

use std::fs;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Window};

use crate::state::AppState;

const STATE_FILE: &str = "window.json";

#[derive(Debug, Serialize, Deserialize)]
struct SavedWindowState {
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    maximized: bool,
}

/// Восстанавливает размер/позицию до показа окна.
pub fn restore(app: &AppHandle) {
    let Some(window) = app.get_webview_window(crate::MAIN_WINDOW) else {
        return;
    };
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let path = state.config_dir.join(STATE_FILE);
    let Ok(raw) = fs::read(&path) else {
        return;
    };
    let Ok(saved) = serde_json::from_slice::<SavedWindowState>(&raw) else {
        return;
    };
    if saved.width < 400 || saved.height < 300 {
        return;
    }

    let _ = window.set_size(tauri::Size::Physical(PhysicalSize::new(
        saved.width,
        saved.height,
    )));

    // Позиция могла быть сохранена на мониторе, который уже отключён.
    // Восстанавливаем её только если окно пересекается с доступным экраном,
    // иначе центрируем: иначе окно открывается за пределами рабочего стола.
    let intersects = window
        .available_monitors()
        .map(|monitors| {
            monitors.iter().any(|monitor| {
                let origin = monitor.position();
                let size = monitor.size();
                let left = i64::from(origin.x);
                let top = i64::from(origin.y);
                let right = left + i64::from(size.width);
                let bottom = top + i64::from(size.height);
                let window_right = i64::from(saved.x) + i64::from(saved.width);
                let window_bottom = i64::from(saved.y) + i64::from(saved.height);
                i64::from(saved.x) < right
                    && window_right > left
                    && i64::from(saved.y) < bottom
                    && window_bottom > top
            })
        })
        .unwrap_or(true);

    if intersects {
        let _ = window.set_position(tauri::Position::Physical(PhysicalPosition::new(
            saved.x, saved.y,
        )));
    } else {
        tracing::warn!(
            x = saved.x,
            y = saved.y,
            "сохранённая позиция окна вне доступных мониторов - центрирую"
        );
        let _ = window.center();
    }

    if saved.maximized {
        let _ = window.maximize();
    }
}

/// Сохраняет состояние при штатном закрытии окна.
pub fn save(window: &Window) {
    let Some(state) = window.app_handle().try_state::<AppState>() else {
        return;
    };
    let Ok(size) = window.inner_size() else {
        return;
    };
    let Ok(position) = window.outer_position() else {
        return;
    };
    let saved = SavedWindowState {
        width: size.width,
        height: size.height,
        x: position.x,
        y: position.y,
        maximized: window.is_maximized().unwrap_or(false),
    };
    let Ok(raw) = serde_json::to_vec_pretty(&saved) else {
        return;
    };
    let path = state.config_dir.join(STATE_FILE);
    if let Err(error) = fs::write(&path, raw) {
        tracing::warn!(path = %path.display(), %error, "не удалось сохранить состояние окна");
    }
}
