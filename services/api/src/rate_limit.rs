use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use warp::{Filter, Rejection};

/// In-memory sliding window bucket for a single key.
#[derive(Debug, Clone)]
struct SlidingWindow {
    timestamps: Vec<Instant>,
}

impl SlidingWindow {
    fn new() -> Self {
        Self {
            timestamps: Vec::with_capacity(256),
        }
    }

    fn purge(&mut self, window: std::time::Duration) {
        let cutoff = Instant::now() - window;
        let keep = self
            .timestamps
            .iter()
            .position(|t| *t > cutoff)
            .unwrap_or(self.timestamps.len());
        self.timestamps.drain(0..keep);
    }

    fn len(&self) -> usize {
        self.timestamps.len()
    }

    fn push(&mut self) {
        self.timestamps.push(Instant::now());
    }
}

/// Rate limiter backend: in-memory or Redis.
enum Backend {
    InMemory(Arc<RwLock<HashMap<String, SlidingWindow>>>),
    Redis(redis::aio::MultiplexedConnection),
}

/// A rate limiter backed by either local memory or Redis sorted sets.
///
/// **In-memory** (default): sliding window per IP via `Vec<Instant>`.
///
/// **Redis** (when created via `new_redis`): sliding window per key using
/// `ZREMRANGEBYSCORE` + `ZADD` + `ZCOUNT`, so the state is shared across
/// all API instances behind a load balancer.
#[derive(Clone)]
pub struct RateLimiter {
    pub max_requests: usize,
    pub window_secs: u64,
    backend: Arc<Backend>,
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter")
            .field("max_requests", &self.max_requests)
            .field("window_secs", &self.window_secs)
            .field(
                "backend",
                &match &*self.backend {
                    Backend::InMemory(_) => "in_memory",
                    Backend::Redis(_) => "redis",
                },
            )
            .finish()
    }
}

impl RateLimiter {
    /// Create an in-memory rate limiter.
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            max_requests,
            window_secs,
            backend: Arc::new(Backend::InMemory(Arc::new(RwLock::new(HashMap::new())))),
        }
    }

    /// Create a Redis-backed rate limiter.
    pub fn new_redis(
        max_requests: usize,
        window_secs: u64,
        conn: redis::aio::MultiplexedConnection,
    ) -> Self {
        Self {
            max_requests,
            window_secs,
            backend: Arc::new(Backend::Redis(conn)),
        }
    }

    /// Returns `true` if the request is allowed, `false` if rate-limited.
    pub async fn check_and_consume(&self, key: &str) -> bool {
        match &*self.backend {
            Backend::InMemory(buckets) => {
                let window_dur = std::time::Duration::from_secs(self.window_secs);
                let mut buckets = buckets.write().await;
                let entry = buckets
                    .entry(key.to_string())
                    .or_insert_with(SlidingWindow::new);
                entry.purge(window_dur);
                if entry.len() >= self.max_requests {
                    return false;
                }
                entry.push();
                true
            }
            Backend::Redis(conn) => {
                let mut conn = conn.clone();
                let now_ms = chrono::Utc::now().timestamp_millis();
                let window_ms = (self.window_secs * 1000) as i64;
                let cutoff = now_ms - window_ms;

                let count: Result<usize, _> = redis::pipe()
                    .zrembyscore(key, 0, cutoff)
                    .ignore()
                    .zadd(key, now_ms, now_ms)
                    .ignore()
                    .zcount(key, cutoff, now_ms)
                    .expire(key, self.window_secs as i64)
                    .ignore()
                    .query_async(&mut conn)
                    .await;

                match count {
                    Ok(n) => n <= self.max_requests,
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            key = %key,
                            "Redis rate-limiter check failed — allowing request"
                        );
                        true
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct RateLimited;

impl warp::reject::Reject for RateLimited {}

pub fn with_rate_limit(
    limiter: RateLimiter,
) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    let limiter_for_filter = limiter.clone();
    warp::any()
        .and(warp::addr::remote())
        .and_then(move |addr: Option<std::net::SocketAddr>| {
            let limiter = limiter_for_filter.clone();
            async move {
                let key = addr
                    .map(|a| a.ip().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                if limiter.check_and_consume(&key).await {
                    Ok(())
                } else {
                    Err(warp::reject::custom(RateLimited))
                }
            }
        })
        .untuple_one()
}
