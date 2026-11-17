//! Конфигурация приложения: модель, валидация и атомарное хранилище.
//! Секретов в конфигурации нет: пароль входа живёт только в памяти сессии.

mod model;
mod store;
mod validate;

pub use model::{AppConfig, ConnectionTest, SaveConfigRequest};
pub use store::ConfigStore;
pub use validate::ensure_connection_config;
