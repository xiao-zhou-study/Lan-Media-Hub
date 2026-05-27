use lan_media_hub_core::{MediaIndex, SharedFolderManager};
use lan_media_hub_config::Settings;
use lan_media_hub_core::db::Database;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct AppState {
    pub shared_folders: Arc<RwLock<SharedFolderManager>>,
    pub media_index: Arc<RwLock<MediaIndex>>,
    pub settings: Arc<RwLock<Settings>>,
    pub server_running: Arc<RwLock<bool>>,
    pub db: Option<Arc<Database>>,
    pub password: Arc<RwLock<String>>,
    pub jwt_secret: Arc<RwLock<String>>,
}

impl AppState {
    #[allow(dead_code)]
    pub fn new(settings: Settings, db: Database) -> Self {
        Self {
            shared_folders: Arc::new(RwLock::new(SharedFolderManager::new())),
            media_index: Arc::new(RwLock::new(MediaIndex::new())),
            settings: Arc::new(RwLock::new(settings)),
            server_running: Arc::new(RwLock::new(false)),
            db: Some(Arc::new(db)),
            password: Arc::new(RwLock::new(String::new())),
            jwt_secret: Arc::new(RwLock::new(lan_media_hub_http::auth::generate_secret())),
        }
    }
}