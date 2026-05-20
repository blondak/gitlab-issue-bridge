use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct AppConfig {
    pub debug: bool,
    pub frontend_origin: String,
    pub public_frontend_url: String,
    pub secret_encryption_key: String,
    pub attachments_dir: String,
    pub attachment_cache_dir: String,
    pub temp_upload_retention_hours: i64,
    pub session_cookie_secure: bool,
    pub require_secret_encryption_key: bool,
    pub worker_heartbeat_interval_seconds: u64,
    pub worker_stale_after_seconds: u64,
    pub queue_stale_after_seconds: u64,
    pub rate_limits: RateLimitConfig,
    pub uploads: UploadConfig,
    pub smtp: SmtpConfig,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub window_seconds: u64,
    pub login_per_email: u32,
    pub login_per_ip: u32,
    pub password_recovery_per_email: u32,
    pub password_recovery_per_ip: u32,
    pub invitation_resend_per_admin: u32,
    pub uploads_per_user: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct UploadConfig {
    pub max_bytes: usize,
    pub allowed_content_types: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
    pub starttls: bool,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let frontend_origin =
            std::env::var("FRONTEND_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".to_string());
        let public_frontend_url =
            std::env::var("PUBLIC_FRONTEND_URL").unwrap_or_else(|_| frontend_origin.clone());

        let session_cookie_secure = env_bool("SESSION_COOKIE_SECURE")
            .unwrap_or_else(|| public_frontend_url.starts_with("https://"));

        let attachments_dir = std::env::var("ATTACHMENTS_DATA_DIR")
            .unwrap_or_else(|_| "./.data/attachments".to_string());
        let attachment_cache_dir = std::env::var("ATTACHMENT_CACHE_DIR")
            .unwrap_or_else(|_| default_attachment_cache_dir(&attachments_dir));

        Self {
            debug: env_bool("DEBUG").unwrap_or(false),
            frontend_origin,
            public_frontend_url,
            secret_encryption_key: std::env::var("SECRET_ENCRYPTION_KEY").unwrap_or_default(),
            attachments_dir,
            attachment_cache_dir,
            temp_upload_retention_hours: std::env::var("TEMP_UPLOAD_RETENTION_HOURS")
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(24),
            session_cookie_secure,
            require_secret_encryption_key: env_bool("REQUIRE_SECRET_ENCRYPTION_KEY").unwrap_or(false),
            worker_heartbeat_interval_seconds: env_u64("WORKER_HEARTBEAT_INTERVAL_SECONDS", 15).max(1),
            worker_stale_after_seconds: env_u64("WORKER_STALE_AFTER_SECONDS", 60).max(1),
            queue_stale_after_seconds: env_u64("QUEUE_STALE_AFTER_SECONDS", 300).max(1),
            rate_limits: RateLimitConfig::from_env(),
            uploads: UploadConfig::from_env(),
            smtp: SmtpConfig {
                host: std::env::var("SMTP_HOST").unwrap_or_default(),
                port: std::env::var("SMTP_PORT")
                    .ok()
                    .and_then(|value| value.parse::<u16>().ok())
                    .unwrap_or(587),
                username: std::env::var("SMTP_USERNAME").unwrap_or_default(),
                password: std::env::var("SMTP_PASSWORD").unwrap_or_default(),
                from_email: std::env::var("SMTP_FROM_EMAIL")
                    .unwrap_or_else(|_| "no-reply@localhost".to_string()),
                from_name: std::env::var("SMTP_FROM_NAME")
                    .unwrap_or_else(|_| "IssueHub".to_string()),
                starttls: env_bool("SMTP_STARTTLS").unwrap_or(true),
            },
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            debug: false,
            frontend_origin: "http://localhost:3000".to_string(),
            public_frontend_url: "http://localhost:3000".to_string(),
            secret_encryption_key: String::new(),
            attachments_dir: "./.data/attachments".to_string(),
            attachment_cache_dir: "./.data/attachments/gitlab-cache".to_string(),
            temp_upload_retention_hours: 24,
            session_cookie_secure: false,
            require_secret_encryption_key: false,
            worker_heartbeat_interval_seconds: 15,
            worker_stale_after_seconds: 60,
            queue_stale_after_seconds: 300,
            rate_limits: RateLimitConfig::default(),
            uploads: UploadConfig::default(),
            smtp: SmtpConfig {
                host: String::new(),
                port: 587,
                username: String::new(),
                password: String::new(),
                from_email: "no-reply@localhost".to_string(),
                from_name: "IssueHub".to_string(),
                starttls: true,
            },
        }
    }
}

impl RateLimitConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("RATE_LIMIT_ENABLED").unwrap_or(true),
            window_seconds: env_u64("RATE_LIMIT_WINDOW_SECONDS", 900).max(1),
            login_per_email: env_u32("RATE_LIMIT_LOGIN_PER_EMAIL", 5),
            login_per_ip: env_u32("RATE_LIMIT_LOGIN_PER_IP", 50),
            password_recovery_per_email: env_u32("RATE_LIMIT_PASSWORD_RECOVERY_PER_EMAIL", 3),
            password_recovery_per_ip: env_u32("RATE_LIMIT_PASSWORD_RECOVERY_PER_IP", 20),
            invitation_resend_per_admin: env_u32("RATE_LIMIT_INVITATION_RESEND_PER_ADMIN", 10),
            uploads_per_user: env_u32("RATE_LIMIT_UPLOADS_PER_USER", 60),
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            window_seconds: 900,
            login_per_email: 5,
            login_per_ip: 50,
            password_recovery_per_email: 3,
            password_recovery_per_ip: 20,
            invitation_resend_per_admin: 10,
            uploads_per_user: 60,
        }
    }
}

impl UploadConfig {
    pub fn from_env() -> Self {
        Self {
            max_bytes: env_usize("UPLOAD_MAX_BYTES", 10 * 1024 * 1024).max(1),
            allowed_content_types: env_content_types(
                "UPLOAD_ALLOWED_CONTENT_TYPES",
                &[
                    "image/png",
                    "image/jpeg",
                    "image/gif",
                    "image/webp",
                    "application/pdf",
                    "text/plain",
                    "text/csv",
                    "application/json",
                    "application/zip",
                ],
            ),
        }
    }
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            max_bytes: 10 * 1024 * 1024,
            allowed_content_types: [
                "image/png",
                "image/jpeg",
                "image/gif",
                "image/webp",
                "application/pdf",
                "text/plain",
                "text/csv",
                "application/json",
                "application/zip",
            ]
            .iter()
            .map(|item| item.to_string())
            .collect(),
        }
    }
}

fn env_bool(key: &str) -> Option<bool> {
    std::env::var(key)
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"))
}

fn default_attachment_cache_dir(attachments_dir: &str) -> String {
    Path::new(attachments_dir)
        .join("gitlab-cache")
        .to_string_lossy()
        .to_string()
}

fn env_u32(key: &str, default_value: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default_value)
}

fn env_u64(key: &str, default_value: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default_value)
}

fn env_usize(key: &str, default_value: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default_value)
}

fn env_content_types(key: &str, default_values: &[&str]) -> Vec<String> {
    std::env::var(key)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(|item| item.trim().to_ascii_lowercase())
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| default_values.iter().map(|item| item.to_string()).collect())
}
