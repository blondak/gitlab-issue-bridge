use anyhow::Context;
use bridge_core::{
    config::AppConfig,
    secrets::{encrypt_secret, ensure_secret_encryption_key_ready},
};
use sqlx::{migrate::Migrator, PgPool, Row};
use tracing::info;
use uuid::Uuid;

pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

pub async fn migrate_and_backfill(pool: &PgPool, config: &AppConfig) -> anyhow::Result<()> {
    MIGRATOR.run(pool).await.context("failed to run SQL migrations")?;
    ensure_bootstrap_admin(pool).await?;
    ensure_secret_encryption_key_ready(pool, config).await?;
    backfill_encrypted_gitlab_secrets(pool, config).await?;
    Ok(())
}

async fn ensure_bootstrap_admin(pool: &PgPool) -> anyhow::Result<()> {
    let email = env_string("INIT_ADMIN_EMAIL", "admin@example.com");
    let password = env_string("INIT_ADMIN_PASSWORD", "admin1234");
    let full_name = env_string("INIT_ADMIN_FULL_NAME", "Default Admin");

    sqlx::query(
        r#"
        INSERT INTO users (email, full_name, password_hash, is_admin, active)
        VALUES ($1, $2, crypt($3, gen_salt('bf')), TRUE, TRUE)
        ON CONFLICT (email) DO UPDATE
        SET full_name = EXCLUDED.full_name,
            is_admin = TRUE,
            active = TRUE,
            updated_at = NOW()
        "#,
    )
    .bind(&email)
    .bind(&full_name)
    .bind(&password)
    .execute(pool)
    .await
    .context("failed to ensure bootstrap admin user")?;

    info!("bootstrap admin ensured for {}", email);
    Ok(())
}

fn env_string(key: &str, default_value: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_value.to_string())
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

    use super::{env_string, migrate_and_backfill, MIGRATOR};

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
        let project_integrations_table_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM information_schema.tables
                WHERE table_schema = 'public'
                  AND table_name = 'project_integrations'
            )
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let issue_external_refs_table_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM information_schema.tables
                WHERE table_schema = 'public'
                  AND table_name = 'issue_external_refs'
            )
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(user_table_exists);
        assert!(worker_heartbeats_table_exists);
        assert!(project_integrations_table_exists);
        assert!(issue_external_refs_table_exists);
    }

    #[sqlx::test(migrations = false)]
    async fn migrate_and_backfill_bootstraps_admin_user(pool: PgPool) {
        let expected_email = env_string("INIT_ADMIN_EMAIL", "admin@example.com");
        let expected_password = env_string("INIT_ADMIN_PASSWORD", "admin1234");

        migrate_and_backfill(&pool, &bridge_core::config::AppConfig::default())
            .await
            .unwrap();

        let row = sqlx::query_as::<_, (String, bool, bool, bool)>(
            r#"
            SELECT email,
                   is_admin,
                   active,
                   password_hash = crypt($2, password_hash) AS password_ok
            FROM users
            WHERE email = $1
            "#,
        )
        .bind(&expected_email)
        .bind(&expected_password)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row, (expected_email, true, true, true));
    }
}
