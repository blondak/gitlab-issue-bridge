use anyhow::Context;
use bridge_core::{config::AppConfig, secrets::{encrypt_secret, ensure_secret_encryption_key_ready}};
use sqlx::{migrate::Migrator, PgPool, Row};
use uuid::Uuid;

pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

pub async fn migrate_and_backfill(pool: &PgPool, config: &AppConfig) -> anyhow::Result<()> {
    MIGRATOR.run(pool).await.context("failed to run SQL migrations")?;
    ensure_secret_encryption_key_ready(pool, config).await?;
    backfill_encrypted_gitlab_secrets(pool, config).await?;
    Ok(())
}

async fn backfill_encrypted_gitlab_secrets(pool: &PgPool, config: &AppConfig) -> anyhow::Result<()> {
    let rows = sqlx::query(
        r#"
        SELECT id, token, webhook_secret, token_encrypted, webhook_secret_encrypted
        FROM project_gitlab_integrations
        "#,
    )
    .fetch_all(pool)
    .await
    .context("failed to load GitLab integrations for backfill")?;

    for row in rows {
        let id: Uuid = row.try_get("id")?;
        let token: String = row.try_get("token")?;
        let webhook_secret: String = row.try_get("webhook_secret")?;
        let token_encrypted: Option<String> = row.try_get("token_encrypted")?;
        let webhook_secret_encrypted: Option<String> = row.try_get("webhook_secret_encrypted")?;

        let mut next_token_encrypted = token_encrypted.unwrap_or_default();
        let mut next_webhook_secret_encrypted = webhook_secret_encrypted.unwrap_or_default();
        let mut should_update = false;

        if next_token_encrypted.is_empty() && !token.is_empty() {
            next_token_encrypted = encrypt_secret(&config.secret_encryption_key, &token)
                .context("failed to encrypt GitLab token during backfill")?;
            should_update = true;
        }

        if next_webhook_secret_encrypted.is_empty() && !webhook_secret.is_empty() {
            next_webhook_secret_encrypted = encrypt_secret(&config.secret_encryption_key, &webhook_secret)
                .context("failed to encrypt GitLab webhook secret during backfill")?;
            should_update = true;
        }

        if should_update {
            sqlx::query(
                r#"
                UPDATE project_gitlab_integrations
                SET token_encrypted = $2,
                    webhook_secret_encrypted = $3,
                    token = '',
                    webhook_secret = '',
                    updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(id)
            .bind(next_token_encrypted)
            .bind(next_webhook_secret_encrypted)
            .execute(pool)
            .await
            .context("failed to persist encrypted GitLab secrets")?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::MIGRATOR;

    #[sqlx::test(migrations = false)]
    async fn migrator_creates_base_schema_on_clean_database(pool: PgPool) {
        MIGRATOR.run(&pool).await.unwrap();

        let user_table_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM information_schema.tables
                WHERE table_schema = 'public'
                  AND table_name = 'users'
            )
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let worker_heartbeats_table_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM information_schema.tables
                WHERE table_schema = 'public'
                  AND table_name = 'worker_heartbeats'
            )
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(user_table_exists);
        assert!(worker_heartbeats_table_exists);
    }
}
