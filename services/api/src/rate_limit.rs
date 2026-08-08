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

                let count: Result<(usize,), _> = redis::pipe()
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
                    Ok((n,)) => n <= self.max_requests,
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

/// Endereço de dentro do cluster (RFC1918, loopback ou ULA IPv6).
///
/// Só destes aceitamos `X-Forwarded-For`: o serviço da API é NodePort, então
/// confiar no header vindo de qualquer origem deixaria qualquer cliente externo
/// forjar uma identidade nova a cada requisição e ignorar o limite.
fn is_trusted_proxy(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => v4.is_private() || v4.is_loopback(),
        // fc00::/7 (unique local) — `is_unique_local` ainda não é estável.
        std::net::IpAddr::V6(v6) => v6.is_loopback() || (v6.octets()[0] & 0xfe) == 0xfc,
    }
}

/// Chave do balde: o cliente REAL, não o proxy.
///
/// O front do Next reescreve `/api/*` no servidor, então sem isto todas as abas
/// de todos os operadores chegam com o IP do pod web e dividem um único balde —
/// um poller a mais na tela derrubava a API inteira com 429.
pub(crate) fn client_key(addr: Option<std::net::SocketAddr>, forwarded: Option<&str>) -> String {
    let remote = match addr {
        Some(a) => a.ip(),
        None => return "unknown".to_string(),
    };
    if is_trusted_proxy(&remote) {
        // O primeiro da lista é o cliente original; os demais são saltos.
        if let Some(first) = forwarded
            .and_then(|h| h.split(',').next())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return first.to_string();
        }
    }
    remote.to_string()
}

pub fn with_rate_limit(
    limiter: RateLimiter,
) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    let limiter_for_filter = limiter.clone();
    warp::any()
        .and(warp::addr::remote())
        .and(warp::header::optional::<String>("x-forwarded-for"))
        .and_then(
            move |addr: Option<std::net::SocketAddr>, fwd: Option<String>| {
                let limiter = limiter_for_filter.clone();
                async move {
                    let key = client_key(addr, fwd.as_deref());
                    if limiter.check_and_consume(&key).await {
                        Ok(())
                    } else {
                        Err(warp::reject::custom(RateLimited))
                    }
                }
            },
        )
        .untuple_one()
}

#[cfg(test)]
mod tests {
    use super::client_key;
    use std::net::SocketAddr;

    fn addr(s: &str) -> Option<SocketAddr> {
        Some(s.parse().expect("socket addr"))
    }

    /// O caso que quebrou a tela: o proxy do Next é interno, e sem ler o header
    /// todas as abas dividiriam o mesmo balde.
    #[test]
    fn internal_proxy_is_trusted_for_the_forwarded_header() {
        assert_eq!(
            client_key(addr("10.244.0.7:53124"), Some("203.0.113.9")),
            "203.0.113.9"
        );
    }

    /// Cadeia de proxies: o primeiro é o cliente, o resto são saltos.
    #[test]
    fn first_hop_in_the_chain_is_the_client() {
        assert_eq!(
            client_key(addr("172.18.0.4:1"), Some("203.0.113.9, 10.244.0.7")),
            "203.0.113.9"
        );
    }

    /// A API é NodePort. Se um cliente EXTERNO pudesse forjar o header, trocaria
    /// de identidade a cada requisição e o limite deixaria de existir.
    #[test]
    fn external_client_cannot_forge_its_identity() {
        assert_eq!(
            client_key(addr("198.51.100.20:44321"), Some("1.2.3.4")),
            "198.51.100.20"
        );
    }

    #[test]
    fn falls_back_to_the_socket_when_there_is_no_header() {
        assert_eq!(client_key(addr("10.244.0.7:1"), None), "10.244.0.7");
        assert_eq!(client_key(addr("10.244.0.7:1"), Some("  ")), "10.244.0.7");
        assert_eq!(client_key(None, Some("1.2.3.4")), "unknown");
    }
}
