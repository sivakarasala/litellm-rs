use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Sliding window rate limiter (in-memory, resets on restart).
#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<RateLimiterInner>>,
}

struct RateLimiterInner {
    /// RPM windows: key_id -> list of request timestamps
    rpm_windows: HashMap<Uuid, Vec<Instant>>,
    /// TPM windows: key_id -> list of (timestamp, token_count)
    tpm_windows: HashMap<Uuid, Vec<(Instant, u32)>>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RateLimiterInner {
                rpm_windows: HashMap::new(),
                tpm_windows: HashMap::new(),
            })),
        }
    }

    /// Check if a request is allowed under RPM limit.
    /// Returns Ok(()) if allowed, Err(message) if rate limited.
    pub fn check_rpm(&self, key_id: Uuid, rpm_limit: i32) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        let window = Duration::from_secs(60);
        let now = Instant::now();

        let entries = inner.rpm_windows.entry(key_id).or_default();

        // Remove expired entries
        entries.retain(|t| now.duration_since(*t) < window);

        if entries.len() >= rpm_limit as usize {
            Err(format!(
                "Rate limit exceeded: {} requests per minute",
                rpm_limit
            ))
        } else {
            entries.push(now);
            Ok(())
        }
    }

    /// Check if estimated tokens are within TPM limit.
    pub fn check_tpm(
        &self,
        key_id: Uuid,
        tpm_limit: i32,
        estimated_tokens: u32,
    ) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        let window = Duration::from_secs(60);
        let now = Instant::now();

        let entries = inner.tpm_windows.entry(key_id).or_default();

        // Remove expired entries
        entries.retain(|(t, _)| now.duration_since(*t) < window);

        let current_tokens: u32 = entries.iter().map(|(_, t)| t).sum();
        if current_tokens + estimated_tokens > tpm_limit as u32 {
            Err(format!(
                "Token rate limit exceeded: {} tokens per minute",
                tpm_limit
            ))
        } else {
            entries.push((now, estimated_tokens));
            Ok(())
        }
    }

    /// Record actual token usage after a request completes (for TPM tracking).
    pub fn record_tokens(&self, key_id: Uuid, actual_tokens: u32) {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();

        let entries = inner.tpm_windows.entry(key_id).or_default();
        entries.push((now, actual_tokens));
    }
}

/// Global rate limiter instance.
static RATE_LIMITER: std::sync::OnceLock<RateLimiter> = std::sync::OnceLock::new();

pub fn rate_limiter() -> &'static RateLimiter {
    RATE_LIMITER.get_or_init(RateLimiter::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpm_allows_under_limit() {
        let limiter = RateLimiter::new();
        let key_id = Uuid::new_v4();
        assert!(limiter.check_rpm(key_id, 5).is_ok());
        assert!(limiter.check_rpm(key_id, 5).is_ok());
    }

    #[test]
    fn rpm_rejects_over_limit() {
        let limiter = RateLimiter::new();
        let key_id = Uuid::new_v4();
        for _ in 0..3 {
            assert!(limiter.check_rpm(key_id, 3).is_ok());
        }
        let result = limiter.check_rpm(key_id, 3);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Rate limit exceeded"));
    }

    #[test]
    fn rpm_different_keys_independent() {
        let limiter = RateLimiter::new();
        let key1 = Uuid::new_v4();
        let key2 = Uuid::new_v4();

        for _ in 0..3 {
            assert!(limiter.check_rpm(key1, 3).is_ok());
        }
        // key1 is at limit, but key2 should be fine
        assert!(limiter.check_rpm(key1, 3).is_err());
        assert!(limiter.check_rpm(key2, 3).is_ok());
    }

    #[test]
    fn tpm_allows_under_limit() {
        let limiter = RateLimiter::new();
        let key_id = Uuid::new_v4();
        assert!(limiter.check_tpm(key_id, 1000, 500).is_ok());
    }

    #[test]
    fn tpm_rejects_over_limit() {
        let limiter = RateLimiter::new();
        let key_id = Uuid::new_v4();
        assert!(limiter.check_tpm(key_id, 1000, 800).is_ok());
        let result = limiter.check_tpm(key_id, 1000, 300);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Token rate limit"));
    }

    #[test]
    fn tpm_accumulates_across_requests() {
        let limiter = RateLimiter::new();
        let key_id = Uuid::new_v4();
        assert!(limiter.check_tpm(key_id, 1000, 400).is_ok());
        assert!(limiter.check_tpm(key_id, 1000, 400).is_ok());
        // 800 + 300 > 1000
        assert!(limiter.check_tpm(key_id, 1000, 300).is_err());
    }
}
