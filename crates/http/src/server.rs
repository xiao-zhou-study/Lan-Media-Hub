use axum::extract::FromRef;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use lan_media_hub_core::{MediaIndex, SharedFolderManager};

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
        Self { config, state: AppState { manager, index, password, jwt_secret } }
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