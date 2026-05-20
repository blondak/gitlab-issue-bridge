use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::http::{HeaderMap, StatusCode};

use crate::error::{ApiError, ApiResult};

#[derive(Clone, Default)]
pub struct RateLimiter {
    buckets: Arc<Mutex<HashMap<String, Bucket>>>,
}

#[derive(Clone)]
struct Bucket {
    window_started_at: Instant,
    count: u32,
}

impl RateLimiter {
    pub fn check(
        &self,
        enabled: bool,
        window_seconds: u64,
        limit: u32,
        key: impl Into<String>,
    ) -> ApiResult<()> {
        if !enabled || limit == 0 {
            return Ok(());
        }

        let now = Instant::now();
        let window = Duration::from_secs(window_seconds.max(1));
        let key = key.into();
        let mut buckets = self
            .buckets
            .lock()
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Rate limiter lock failed"))?;

        if buckets.len() > 10_000 {
            buckets.retain(|_, bucket| now.duration_since(bucket.window_started_at) < window);
        }

        let bucket = buckets.entry(key).or_insert(Bucket {
            window_started_at: now,
            count: 0,
        });

        if now.duration_since(bucket.window_started_at) >= window {
            bucket.window_started_at = now;
            bucket.count = 0;
        }

        if bucket.count >= limit {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded. Try again later.",
            ));
        }

        bucket.count += 1;
        Ok(())
    }
}

pub fn client_identifier(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or("unknown")
        .to_ascii_lowercase()
}

pub fn key_part(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || matches!(char, '.' | '-' | '_' | '@') {
                char
            } else {
                '_'
            }
        })
        .collect()
}
