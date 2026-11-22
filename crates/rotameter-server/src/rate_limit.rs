#![forbid(unsafe_code)]
//! Bounded in-memory per-IP token-bucket rate limiter for unauthenticated collection endpoints.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config::RateLimitConfig;

const STALE_TTL: Duration = Duration::from_secs(120);

#[derive(Clone, Copy)]
struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

pub(crate) struct IpRateLimiter {
    config: RateLimitConfig,
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl IpRateLimiter {
    pub(crate) fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(crate) fn check(&self, ip: &str) -> Result<(), u64> {
        if !self.config.enabled {
            return Ok(());
        }
        if self.config.burst == 0 {
            return Err(60);
        }

        let now = Instant::now();
        let max_tokens = f64::from(self.config.burst);
        let rate_per_sec = f64::from(self.config.requests_per_minute) / 60.0;

        let Ok(mut map) = self.buckets.lock() else {
            return Ok(());
        };

        if !map.contains_key(ip) && map.len() >= self.config.max_tracked_ips.max(1) {
            evict_if_full(&mut map, now);
        }

        let entry = map.entry(ip.to_string()).or_insert(Bucket {
            tokens: max_tokens,
            last_refill: now,
        });

        let elapsed = now
            .saturating_duration_since(entry.last_refill)
            .as_secs_f64();
        if rate_per_sec > 0.0 && elapsed > 0.0 {
            entry.tokens = (entry.tokens + elapsed * rate_per_sec).min(max_tokens);
        }
        entry.last_refill = now;

        if entry.tokens >= 1.0 {
            entry.tokens -= 1.0;
            Ok(())
        } else if rate_per_sec <= 0.0 {
            Err(60)
        } else {
            let wait_secs = ((1.0 - entry.tokens) / rate_per_sec)
                .ceil()
                .clamp(1.0, 3600.0);
            Err(wait_secs as u64)
        }
    }
}

fn evict_if_full(map: &mut HashMap<String, Bucket>, now: Instant) {
    map.retain(|_, bucket| now.saturating_duration_since(bucket.last_refill) < STALE_TTL);
    if !map.is_empty()
        && let Some(oldest_key) = map
            .iter()
            .min_by_key(|(_, bucket)| bucket.last_refill)
            .map(|(k, _)| k.clone())
    {
        map.remove(&oldest_key);
    }
}
