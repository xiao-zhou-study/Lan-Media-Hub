#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod app_state;
mod commands;

use app_state::AppState;
use lan_media_hub_core::db::Database;
use lan_media_hub_config::Settings;
use lan_media_hub_http::{HttpServer, HttpServerConfig};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::path::PathBuf;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let app_state = rt.block_on(async {
        let db_path = get_db_path();
        let db = Database::new(&db_path).await.expect("Failed to initialize database");

        // 从 JSON 文件加载设置
        let settings = load_settings();

        // 从 DB 加载密码（优先），否则用 settings.json 中的
        let password = if let Ok(Some(db_pw)) = db.get_kv("password").await {
            db_pw
        } else {
            let pw = settings.password.clone();
            if !pw.is_empty() {
                let _ = db.set_kv("password", &pw).await;
            }
            pw
        };

        // JWT secret：从 DB 加载或生成新的
        let jwt_secret = match db.get_kv("jwt_secret").await {
            Ok(Some(s)) => s,
            _ => {
                let s = lan_media_hub_http::auth::generate_secret();
                let _ = db.set_kv("jwt_secret", &s).await;
                s
            }
        };

        let state = AppState::new(settings, db);

        // 恢复到共享 Arc
        {
            let mut pw = state.password.write().await;
            *pw = password;
        }
        {
            let mut js = state.jwt_secret.write().await;
            *js = jwt_secret;
        }

        // 从数据库恢复共享和索引
        if let Some(db) = &state.db {
            if let Err(e) = state.shared_folders.write().await.load_from_db(db).await {
                tracing::warn!("Failed to load shares from DB: {}", e);
            }
            if let Err(e) = state.media_index.write().await.load_from_db(db).await {
                tracing::warn!("Failed to load media index from DB: {}", e);
            }
        }

        state
    });
    drop(rt);

    let state = Arc::new(RwLock::new(app_state));

    // 自动启动 HTTP 服务
    {
        let s = state.clone();
        tauri::async_runtime::spawn(async move {
            let (port, pw_arc, jwt_arc, manager, index) = {
                let app_state = s.read().await;
                let settings = app_state.settings.read().await;
                let port = settings.port;
                let pw_arc = app_state.password.clone();
                let jwt_arc = app_state.jwt_secret.clone();
                let manager = app_state.shared_folders.clone();
                let index = app_state.media_index.clone();
                (port, pw_arc, jwt_arc, manager, index)
            };

            let config = HttpServerConfig {
                port,
                host: "0.0.0.0".to_string(),
            };
            let server = HttpServer::new(config, manager, index, pw_arc, jwt_arc);

            {
                let app_state = s.write().await;
                let mut running = app_state.server_running.write().await;
                *running = true;
            }

            tracing::info!("HTTP server auto-started on port {}", port);
            if let Err(e) = server.start().await {
                tracing::error!("HTTP server error: {}", e);
            }
        });
    }


        // 文件监控 + 防抖：5 秒无新事件触发增量扫描
        {
            let s = state.clone();
            tauri::async_runtime::spawn(async move {
                use lan_media_hub_core::{FolderWatcher, IndexScanner};
                let debounce_dur = std::time::Duration::from_secs(5);
                let shares: Vec<(uuid::Uuid, std::path::PathBuf)> = {
                    let app = s.read().await;
                    let manager = app.shared_folders.read().await;
                    manager.get_all_shares().iter().map(|s| (s.config.id, s.config.path.clone())).collect()
                };
                for (share_id, path) in shares {
                    let s3 = s.clone();
                    tauri::async_runtime::spawn(async move {
                        let mut w = match FolderWatcher::new(&path) {
                            Ok(w) => w,
                            Err(_) => return,
                        };
                        loop {
                            if w.next_event().await.is_none() { break; }
                            let deadline = tokio::time::Instant::now() + debounce_dur;
                            loop {
                                match tokio::time::timeout_at(deadline, w.next_event()).await {
                                    Ok(Some(_)) => continue,
                                    _ => break,
                                }
                            }
                            tracing::info!("File change in share {}, rescanning...", share_id);
                            let app = s3.read().await;
                            if let Some(db) = &app.db {
                                let mut index = app.media_index.write().await;
                                let scanner = IndexScanner::new();
                                let _ = scanner.scan_share(share_id, &path, &mut index, db).await;
                            }
                        }
                    });
                }
            });
        }
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_shares,
            commands::add_share,
            commands::remove_share,
            commands::browse_folder,
            commands::get_settings,
            commands::update_settings,
            commands::start_server,
            commands::stop_server,
            commands::get_status,
            commands::generate_qr,
            commands::get_hostname,
            commands::get_server_info,
            commands::set_password,
            commands::get_password,
        ])
        .manage(state)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn get_db_path() -> PathBuf {
    let dir = app_data_dir();
    dir.join("lan_media_hub.db")
}

fn get_settings_path() -> PathBuf {
    app_data_dir().join("settings.json")
}

fn app_data_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
        let dir = PathBuf::from(appdata).join("LanMediaHub");
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }
    PathBuf::from(".")
}

fn load_settings() -> Settings {
    let path = get_settings_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(s) = serde_json::from_str::<Settings>(&content) {
                tracing::info!("Loaded settings from {:?}", path);
                return s;
            }
        }
    }
    Settings::default()
}