use lan_media_hub_config::Settings;
use tauri::State;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use lan_media_hub_core::IndexScanner;

#[derive(Debug, Serialize, Deserialize)]
pub struct ShareResponse {
    pub id: String,
    pub name: String,
    pub path: String,
    pub file_count: u64,
    pub total_size: u64,
    pub status: String,
}

fn share_status_to_string(status: lan_media_hub_core::ShareStatus) -> String {
    match status {
        lan_media_hub_core::ShareStatus::Active => "active".to_string(),
        lan_media_hub_core::ShareStatus::Paused => "paused".to_string(),
        lan_media_hub_core::ShareStatus::Error => "error".to_string(),
    }
}

#[tauri::command]
pub async fn get_shares(
    state: State<'_, Arc<RwLock<super::app_state::AppState>>>,
) -> Result<Vec<ShareResponse>, String> {
    let app_state = state.read().await;
    let manager = app_state.shared_folders.read().await;
    let shares = manager.get_all_shares();

    Ok(shares
        .into_iter()
        .map(|s| ShareResponse {
            id: s.config.id.to_string(),
            name: s.config.name.clone(),
            path: s.config.path.to_string_lossy().to_string(),
            file_count: s.file_count,
            total_size: s.total_size,
            status: share_status_to_string(s.status),
        })
        .collect())
}

#[tauri::command]
pub async fn add_share(
    path: String,
    name: Option<String>,
    state: State<'_, Arc<RwLock<super::app_state::AppState>>>,
) -> Result<String, String> {
    let app_state = state.read().await;
    let path_buf = std::path::PathBuf::from(&path);
    let db = app_state.db.as_ref().ok_or("Database not initialized")?;

    // 1. 添加共享到管理器
    let id = {
        let mut manager = app_state.shared_folders.write().await;
        manager.add_share(path_buf.clone(), name, db).await
            .inspect_err(|e| tracing::error!("add_share failed: {:#}", e))
            .map_err(|e| format!("{:#}", e))?
    };

    // 2. 立即扫描目录
    let scanner = IndexScanner::new();
    let (file_count, total_size) = {
        let mut index = app_state.media_index.write().await;
        scanner.scan_share(id, &path_buf, &mut index, db)
            .await
            .map_err(|e| e.to_string())?
    };

    // 3. 更新统计
    {
        let mut manager = app_state.shared_folders.write().await;
        manager.update_stats(id, file_count, total_size);
    }

    tracing::info!("Share {} scanned: {} files, {} bytes", id, file_count, total_size);
    Ok(id.to_string())
}

#[tauri::command]
pub async fn remove_share(
    id: String,
    state: State<'_, Arc<RwLock<super::app_state::AppState>>>,
) -> Result<(), String> {
    let uuid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let app_state = state.read().await;
    let mut manager = app_state.shared_folders.write().await;
    let db = app_state.db.as_ref().ok_or("Database not initialized")?;

    manager.remove_share(uuid, db).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_settings(
    state: State<'_, Arc<RwLock<super::app_state::AppState>>>,
) -> Result<Settings, String> {
    let app_state = state.read().await;
    let settings = app_state.settings.read().await;
    Ok(settings.clone())
}

#[tauri::command]
pub async fn update_settings(
    port: Option<u16>,
    auto_start: Option<bool>,
    state: State<'_, Arc<RwLock<super::app_state::AppState>>>,
) -> Result<(), String> {
    let app_state = state.read().await;
    let mut settings = app_state.settings.write().await;

    if let Some(p) = port { settings.port = p; }
    if let Some(a) = auto_start { settings.auto_start = a; }

    Ok(())
}

#[tauri::command]
pub async fn set_password(
    password: String,
    state: State<'_, Arc<RwLock<super::app_state::AppState>>>,
) -> Result<(), String> {
    let app_state = state.read().await;
    let mut settings = app_state.settings.write().await;
    settings.password = password.clone();
    let mut pw = app_state.password.write().await;
    *pw = password.clone();

    // 持久化到文件 + DB
    let settings_path = get_settings_path();
    if let Ok(json) = serde_json::to_string_pretty(&*settings) {
        let _ = tokio::fs::write(&settings_path, json).await;
    }
    if let Some(db) = &app_state.db {
        let _ = db.set_kv("password", &password).await;
    }

    Ok(())
}

/// 获取当前密码（用于前端回显）
#[tauri::command]
pub async fn get_password(
    state: State<'_, Arc<RwLock<super::app_state::AppState>>>,
) -> Result<String, String> {
    let app_state = state.read().await;
    let pw = app_state.password.read().await;
    Ok(pw.clone())
}

fn get_settings_path() -> std::path::PathBuf {
    if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
        let dir = std::path::PathBuf::from(appdata).join("LanMediaHub");
        let _ = std::fs::create_dir_all(&dir);
        return dir.join("settings.json");
    }
    std::path::PathBuf::from("settings.json")
}

#[tauri::command]
pub async fn start_server(
    state: State<'_, Arc<RwLock<super::app_state::AppState>>>,
) -> Result<String, String> {
    let app_state = state.read().await;

    {
        let mut running = app_state.server_running.write().await;
        if *running {
            return Err("Server already running".to_string());
        }
        *running = true;
    }

    let settings = app_state.settings.read().await;
    let port = settings.port;

    let config = lan_media_hub_http::HttpServerConfig {
        port,
        host: "0.0.0.0".to_string(),
    };

    let server = lan_media_hub_http::HttpServer::new(
        config,
        app_state.shared_folders.clone(),
        app_state.media_index.clone(),
        app_state.password.clone(),
        app_state.jwt_secret.clone(),
    );

    tauri::async_runtime::spawn(async move {
        if let Err(e) = server.start().await {
            tracing::error!("HTTP server error: {}", e);
        }
    });

    Ok(settings.lan_url())
}

#[tauri::command]
pub async fn stop_server(
    state: State<'_, Arc<RwLock<super::app_state::AppState>>>,
) -> Result<(), String> {
    let app_state = state.read().await;
    let mut running = app_state.server_running.write().await;
    *running = false;
    Ok(())
}

#[tauri::command]
pub async fn get_status(
    state: State<'_, Arc<RwLock<super::app_state::AppState>>>,
) -> Result<serde_json::Value, String> {
    let app_state = state.read().await;
    let running = app_state.server_running.read().await;
    let settings = app_state.settings.read().await;
    let manager = app_state.shared_folders.read().await;

    Ok(serde_json::json!({
        "server_running": *running,
        "port": settings.port,
        "share_count": manager.active_count(),
        "url": settings.lan_url()
    }))
}
#[derive(Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub media_type: String,
}

/// 浏览共享文件夹内容
#[tauri::command]
pub async fn browse_folder(
    share_id: String,
    sub_path: Option<String>,
    state: State<'_, Arc<RwLock<super::app_state::AppState>>>,
) -> Result<Vec<FileEntry>, String> {
    let uuid = uuid::Uuid::parse_str(&share_id).map_err(|e| e.to_string())?;
    let app_state = state.read().await;
    let manager = app_state.shared_folders.read().await;

    let share = manager.get_share(uuid).ok_or("Share not found")?;
    let share_root = share.config.path.clone();

    let full_path = match &sub_path {
        Some(p) if !p.is_empty() => share_root.join(p),
        _ => share_root.clone(),
    };

    // 安全检查
    let clean = path_clean::PathClean::clean(&full_path);
    if !clean.starts_with(&share_root) {
        return Err("Path outside share boundary".to_string());
    }

    if !clean.exists() {
        return Err("Path not found".to_string());
    }

    let mut entries = Vec::new();
    if clean.is_dir() {
        if let Ok(read_dir) = std::fs::read_dir(&clean) {
            for entry in read_dir.flatten() {
                let p = entry.path();
                let is_dir = p.is_dir();
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
                let rel = p.strip_prefix(&share_root).unwrap_or(&p).to_string_lossy().to_string();
                entries.push(FileEntry {
                    name,
                    path: rel,
                    size: if is_dir { 0 } else { std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0) },
                    is_dir,
                    media_type: if is_dir { "directory".into() } else { classify(&p) },
                });
            }
        }
    }

    // 目录在前，文件在后
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(entries)
}

fn classify(p: &std::path::Path) -> String {
    match p.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("mp4") | Some("mkv") | Some("avi") | Some("mov") | Some("webm") | Some("wmv") | Some("flv") => "video".into(),
        Some("mp3") | Some("flac") | Some("wav") | Some("aac") | Some("ogg") | Some("m4a") | Some("wma") => "audio".into(),
        Some("jpg") | Some("jpeg") | Some("png") | Some("gif") | Some("bmp") | Some("webp") | Some("tiff") => "image".into(),
        _ => "file".into(),
    }
}

/// 生成 QR 码（PNG base64）
#[tauri::command]
pub async fn generate_qr(
    state: State<'_, Arc<RwLock<super::app_state::AppState>>>,
) -> Result<String, String> {
    let app_state = state.read().await;
    let settings = app_state.settings.read().await;
    let url = settings.lan_url();
    let qr = qrcode::QrCode::new(url.as_bytes()).map_err(|e| e.to_string())?;
    let img = qr.render::<qrcode::render::unicode::Dense1x2>().build();
    Ok(img)
}

/// 获取主机名
#[tauri::command]
pub async fn get_hostname() -> Result<String, String> {
    hostname::get()
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|_| "Invalid hostname".to_string())
}

/// 获取服务信息（主机名、URL、密码状态等）
#[tauri::command]
pub async fn get_server_info(
    state: State<'_, Arc<RwLock<super::app_state::AppState>>>,
) -> Result<serde_json::Value, String> {
    let app_state = state.read().await;
    let settings = app_state.settings.read().await;
    let running = app_state.server_running.read().await;

    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "Unknown".to_string());

    Ok(serde_json::json!({
        "hostname": hostname,
        "url": settings.lan_url(),
        "port": settings.port,
        "server_running": *running,
        "has_password": !settings.password.is_empty(),
    }))
}
