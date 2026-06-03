use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::http::HeaderMap;

use crate::config::AppConfig;
use crate::errors::ApiError;

#[derive(Clone)]
pub struct RateLimiter {
    enabled: bool,
    inner: Arc<Mutex<HashMap<String, Bucket>>>,
}

#[derive(Clone)]
struct Bucket {
    count: u32,
    reset_at: Instant,
}

impl RateLimiter {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            enabled: config.rate_limit_enabled,
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn check(
        &self,
        scope: &str,
        headers: &HeaderMap,
        max_requests: u32,
        window_seconds: u64,
    ) -> Result<(), ApiError> {
        if !self.enabled {
            return Ok(());
        }

        let key = format!("{scope}:{}", client_ip(headers));
        let now = Instant::now();
        let reset_at = now + Duration::from_secs(window_seconds);
        let mut buckets = self
            .inner
            .lock()
            .map_err(|_| ApiError::internal("Rate limiter indisponible"))?;

        let bucket = buckets.entry(key).or_insert(Bucket { count: 0, reset_at });
        if now >= bucket.reset_at {
            bucket.count = 0;
            bucket.reset_at = reset_at;
        }

        bucket.count = bucket.count.saturating_add(1);
        if bucket.count > max_requests {
            return Err(ApiError::too_many_requests(
                "Trop de requetes, veuillez reessayer plus tard",
            ));
        }

        Ok(())
    }
}

pub fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string()
}
