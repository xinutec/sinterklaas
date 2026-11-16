//! What every request handler shares.

use sqlx::mysql::MySqlPool;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub cfg: std::sync::Arc<Config>,
}

impl AppState {
    pub fn new(pool: MySqlPool, cfg: Config) -> Self {
        AppState {
            pool,
            cfg: std::sync::Arc::new(cfg),
        }
    }
}
