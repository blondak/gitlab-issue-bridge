use anyhow::Context;
use bridge_core::{config::AppConfig, secrets::ensure_secret_encryption_key_ready};
use sqlx::{migrate::Migrator, PgPool};

pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

pub async fn migrate(pool: &PgPool, config: &AppConfig) -> anyhow::Result<()> {
    MIGRATOR.run(pool).await.context("failed to run SQL migrations")?;
    ensure_secret_encryption_key_ready(pool, config).await?;
    Ok(())
}
