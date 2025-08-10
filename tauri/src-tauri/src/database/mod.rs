use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::time::Duration;
use std::sync::Arc;

pub mod models;
pub mod repository;
pub mod utils;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Arc<Self>, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .connect(database_url)
            .await?;
        
        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await?;
        
        Ok(Arc::new(Self { pool }))
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}