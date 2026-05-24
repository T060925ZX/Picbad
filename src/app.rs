use crate::{cache::TransformCache, config::Config};
use sqlx::SqlitePool;

pub struct AppState {
    pub config: Config,
    pub pool: SqlitePool,
    pub cache: TransformCache,
}

impl AppState {
    pub fn new(config: Config, pool: SqlitePool) -> Self {
        let cache = TransformCache::new(config.cache_dir(), config.cache_max_bytes);
        Self {
            config,
            pool,
            cache,
        }
    }
}
