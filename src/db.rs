use anyhow::Context;
use sqlx::{sqlite::SqliteConnectOptions, ConnectOptions, SqlitePool};
use std::{str::FromStr, time::Duration};

pub async fn connect(url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)
        .with_context(|| format!("invalid database url: {url}"))?
        .create_if_missing(true)
        .log_statements(tracing::log::LevelFilter::Debug);
    SqlitePool::connect_with(options)
        .await
        .context("connect sqlite")
}

pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(pool)
        .await?;
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(pool)
        .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL CHECK(role IN ('admin','user')),
            token TEXT NOT NULL UNIQUE,
            api_key TEXT UNIQUE,
            created_at TEXT NOT NULL,
            last_login_at TEXT
        )
        "#,
    )
    .execute(pool)
    .await?;
    let user_columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('users')")
            .fetch_all(pool)
            .await?;
    if !user_columns.iter().any(|name| name == "api_key") {
        sqlx::query("ALTER TABLE users ADD COLUMN api_key TEXT")
            .execute(pool)
            .await?;
    }
    let users_without_key: Vec<String> =
        sqlx::query_scalar("SELECT id FROM users WHERE api_key IS NULL OR api_key = ''")
            .fetch_all(pool)
            .await?;
    for id in users_without_key {
        sqlx::query("UPDATE users SET api_key = ? WHERE id = ?")
            .bind(format!("pk_{}", uuid::Uuid::new_v4().simple()))
            .bind(id)
            .execute(pool)
            .await?;
    }
    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_users_api_key ON users(api_key)")
        .execute(pool)
        .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS images (
            id TEXT PRIMARY KEY,
            owner_id TEXT NOT NULL,
            sha256 TEXT NOT NULL,
            file_name TEXT NOT NULL,
            mime TEXT NOT NULL,
            ext TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            width INTEGER,
            height INTEGER,
            created_at TEXT NOT NULL,
            hits INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY(owner_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(pool)
    .await?;
    let images_sql: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'images'",
    )
    .fetch_optional(pool)
    .await?;
    if images_sql
        .as_deref()
        .is_some_and(|sql| sql.contains("sha256 TEXT NOT NULL UNIQUE"))
    {
        sqlx::query(
            r#"
            CREATE TABLE images_new (
                id TEXT PRIMARY KEY,
                owner_id TEXT NOT NULL,
                sha256 TEXT NOT NULL,
                file_name TEXT NOT NULL,
                mime TEXT NOT NULL,
                ext TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                width INTEGER,
                height INTEGER,
                created_at TEXT NOT NULL,
                hits INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY(owner_id) REFERENCES users(id)
            )
            "#,
        )
        .execute(pool)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO images_new (id, owner_id, sha256, file_name, mime, ext, size_bytes, width, height, created_at, hits)
            SELECT id, owner_id, sha256, file_name, mime, ext, size_bytes, width, height, created_at, hits FROM images
            "#,
        )
        .execute(pool)
        .await?;
        sqlx::query("DROP TABLE images").execute(pool).await?;
        sqlx::query("ALTER TABLE images_new RENAME TO images")
            .execute(pool)
            .await?;
    }
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_images_owner ON images(owner_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_images_sha ON images(sha256)")
        .execute(pool)
        .await?;
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_images_owner_sha ON images(owner_id, sha256)",
    )
    .execute(pool)
    .await?;
    tokio::time::sleep(Duration::from_millis(1)).await;
    Ok(())
}
