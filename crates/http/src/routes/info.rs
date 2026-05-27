use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::path::PathBuf;
use tokio::process::Command;
use serde::Serialize;
use crate::server::AppState;
use super::share::parse_rest;

#[derive(Serialize)]
pub struct VideoInfo {
    pub duration: f64,       // 秒
    pub width: u32,
    pub height: u32,
    pub codec: String,
    pub size: u64,
}

/// 获取视频/文件详细信息（时长、分辨率等）
pub async fn get_file_info(
    _auth: crate::auth::Auth,
    Path(rest): Path<String>,
    State(state): State<AppState>,
) -> Response {
    let (id, file_path) = parse_rest(&rest);
    let manager = state.manager.read().await;
    let uuid = match uuid::Uuid::parse_str(&id) { Ok(u) => u, Err(_) => return (StatusCode::BAD_REQUEST, "Bad ID").into_response() };
    let share = match manager.get_share(uuid) { Some(s) => s, None => return (StatusCode::NOT_FOUND, "Not found").into_response() };
    let full_path = if file_path.is_empty() { share.config.path.clone() } else { share.config.path.join(&file_path) };
    let clean: PathBuf = path_clean::PathClean::clean(&full_path);
    if !clean.starts_with(&share.config.path) { return (StatusCode::FORBIDDEN, "Forbidden").into_response() }

    let size = std::fs::metadata(&clean).map(|m| m.len()).unwrap_or(0);
    let ext = clean.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    // 用 ffprobe 获取视频信息
    if matches!(ext.as_str(), "mp4" | "mkv" | "avi" | "mov" | "webm" | "wmv" | "flv" | "ts" | "m4v") {
        if let Ok(output) = Command::new("ffprobe")
            .args(["-v", "quiet", "-print_format", "json", "-show_format", "-show_streams"])
            .arg(clean.to_string_lossy().to_string())
            .output().await
        {
            if let Ok(probe) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                let mut info = VideoInfo { duration: 0.0, width: 0, height: 0, codec: String::new(), size };
                if let Some(f) = probe.get("format") {
                    info.duration = f.get("duration").and_then(|d| d.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                }
                if let Some(streams) = probe.get("streams").and_then(|s| s.as_array()) {
                    for stream in streams {
                        if stream.get("codec_type").and_then(|c| c.as_str()) == Some("video") {
                            info.width = stream.get("width").and_then(|w| w.as_u64()).unwrap_or(0) as u32;
                            info.height = stream.get("height").and_then(|h| h.as_u64()).unwrap_or(0) as u32;
                            info.codec = stream.get("codec_name").and_then(|c| c.as_str()).unwrap_or("").to_string();
                            break;
                        }
                    }
                }
                return (StatusCode::OK, Json(info)).into_response();
            }
        }
    }

    // 非视频文件返回基本信息
    (StatusCode::OK, Json(VideoInfo { duration: 0.0, width: 0, height: 0, codec: ext, size })).into_response()
}