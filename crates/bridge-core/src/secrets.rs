use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::config::AppConfig;

pub async fn ensure_secret_encryption_key_ready(
    pool: &sqlx::PgPool,
    config: &AppConfig,
) -> Result<()> {
    let has_stored_gitlab_secrets = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM project_gitlab_integrations
            WHERE COALESCE(NULLIF(token, ''), NULLIF(webhook_secret, ''), NULLIF(token_encrypted, ''), NULLIF(webhook_secret_encrypted, '')) IS NOT NULL
        )
        "#,
    )
    .fetch_one(pool)
    .await
    .context("failed to check stored GitLab secrets")?;

    if config.require_secret_encryption_key || has_stored_gitlab_secrets {
        validate_secret_encryption_key(&config.secret_encryption_key)?;
    }

    Ok(())
}

pub fn validate_secret_encryption_key(secret_key: &str) -> Result<()> {
    build_cipher(secret_key).map(|_| ())
}

pub fn encrypt_secret(secret_key: &str, plaintext: &str) -> Result<String> {
    if secret_key.trim().is_empty() {
        return Err(anyhow!("SECRET_ENCRYPTION_KEY is not configured"));
    }

    let cipher = build_cipher(secret_key)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| anyhow!("failed to encrypt secret"))?;

    let mut payload = nonce.to_vec();
    payload.extend(ciphertext);
    Ok(STANDARD.encode(payload))
}

pub fn decrypt_secret(secret_key: &str, encrypted_value: &str) -> Result<String> {
    if secret_key.trim().is_empty() {
        return Err(anyhow!("SECRET_ENCRYPTION_KEY is not configured"));
    }

    let decoded = STANDARD
        .decode(encrypted_value)
        .context("failed to decode encrypted secret")?;

    if decoded.len() <= 12 {
        return Err(anyhow!("encrypted secret payload is too short"));
    }

    let (nonce_bytes, ciphertext) = decoded.split_at(12);
    let cipher = build_cipher(secret_key)?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow!("failed to decrypt secret"))?;

    String::from_utf8(plaintext).context("decrypted secret is not valid UTF-8")
}

fn build_cipher(secret_key: &str) -> Result<Aes256Gcm> {
    let key = STANDARD
        .decode(secret_key)
        .context("SECRET_ENCRYPTION_KEY must be base64 encoded")?;

    if key.len() != 32 {
        return Err(anyhow!(
            "SECRET_ENCRYPTION_KEY must decode to exactly 32 bytes"
        ));
    }

    Aes256Gcm::new_from_slice(&key)
        .map_err(|_| anyhow!("failed to initialize AES-256-GCM cipher"))
}
