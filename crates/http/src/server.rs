use axum::extract::FromRef;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use lan_media_hub_core::{MediaIndex, SharedFolderManager};
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// 登录限速器（基于 IP）
pub struct RateLimiter {
    attempts: RwLock<HashMap<IpAddr, (usize, Instant)>>,
    max_attempts: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_attempts: usize, window: Duration) -> Self {
        Self {
            attempts: RwLock::new(HashMap::new()),
            max_attempts,
            window,
        }
    }

    /// 检查 IP 是否被限速
    pub async fn is_rate_limited(&self, ip: &IpAddr) -> bool {
        let attempts = self.attempts.read().await;
        if let Some((count, last_attempt)) = attempts.get(ip) {
            if last_attempt.elapsed() < self.window {
                return *count >= self.max_attempts;
            }
        }
        false
    }

    /// 记录失败尝试
    pub async fn record_failure(&self, ip: &IpAddr) {
        let mut attempts = self.attempts.write().await;
        let entry = attempts.entry(*ip).or_insert((0, Instant::now()));
        if entry.1.elapsed() >= self.window {
            *entry = (1, Instant::now());
        } else {
            entry.0 += 1;
            entry.1 = Instant::now();
        }
    }

    /// 清除成功登录的记录
    pub async fn clear(&self, ip: &IpAddr) {
        let mut attempts = self.attempts.write().await;
        attempts.remove(ip);
    }
}

#[derive(Clone)]
pub struct HttpServerConfig {
    pub port: u16,
    pub host: String,
}

#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<RwLock<SharedFolderManager>>,
    pub index: Arc<RwLock<MediaIndex>>,
    pub password: Arc<RwLock<String>>,
    pub jwt_secret: Arc<RwLock<String>>,
    pub rate_limiter: Arc<RateLimiter>,
}

impl FromRef<AppState> for Arc<RwLock<String>> {
    fn from_ref(state: &AppState) -> Self { state.password.clone() }
}

pub struct HttpServer {
    config: HttpServerConfig,
    state: AppState,
}

impl HttpServer {
    pub fn new(
        config: HttpServerConfig,
        manager: Arc<RwLock<SharedFolderManager>>,
        index: Arc<RwLock<MediaIndex>>,
        password: Arc<RwLock<String>>,
        jwt_secret: Arc<RwLock<String>>,
    ) -> Self {
        Self {
            config,
            state: AppState {
                manager,
                index,
                password,
                jwt_secret,
                rate_limiter: Arc::new(RateLimiter::new(10, Duration::from_secs(300))), // 5分钟内最多10次
            }
        }
    }

    pub async fn start(self) -> Result<(), anyhow::Error> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse().expect("Invalid address");

        let app = crate::routes::create_router().with_state(self.state);

        tracing::info!("HTTP server starting on {}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}