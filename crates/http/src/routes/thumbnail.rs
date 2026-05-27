use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use axum::body::Body;
use std::path::PathBuf;
use crate::server::AppState;
use super::share::parse_rest;

#[derive(serde::Deserialize)]
pub struct ThumbParams { #[serde(default = "default_size")] size: u32 }
fn default_size() -> u32 { 480 }

fn ffmpeg_path() -> String {
    ffmpeg_sidecar::paths::ffmpeg_path().to_string_lossy().to_string()
}

pub async fn get_thumbnail(
    _auth: crate::auth::Auth,
    Path(rest): Path<String>,
    Query(params): Query<ThumbParams>,
    State(state): State<AppState>,
) -> Response {
    let (id, file_path) = parse_rest(&rest);
    let manager = state.manager.read().await;
    let uuid = match uuid::Uuid::parse_str(&id) { Ok(u) => u, Err(_) => return (StatusCode::BAD_REQUEST, "Bad ID").into_response() };
    let share = match manager.get_share(uuid) { Some(s) => s, None => return (StatusCode::NOT_FOUND, "Not found").into_response() };
    let full_path = if file_path.is_empty() { share.config.path.clone() } else { share.config.path.join(&file_path) };
    let clean: PathBuf = path_clean::PathClean::clean(&full_path);
    if !clean.starts_with(&share.config.path) { return (StatusCode::FORBIDDEN, "Out of bounds").into_response() }
    if !clean.exists() { return (StatusCode::NOT_FOUND, "Not found").into_response() }

    let ext = clean.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    // .bc! 后缀 → 看前面的扩展名
    let real_ext = if ext == "bc!" { clean.with_extension("").extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default() } else { ext.clone() };
    if matches!(real_ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp") {
        return serve_file(&clean, &format!("image/{}", if ext == "jpg" { "jpeg" } else { &ext })).await;
    }

    if matches!(real_ext.as_str(), "mp4" | "mkv" | "avi" | "mov" | "webm" | "wmv" | "flv" | "mpg" | "mpeg" | "ts" | "mts" | "m2ts" | "vob" | "rm" | "rmvb" | "3gp" | "asf" | "divx" | "ogv" | "m4v") {
        let cache_key = format!("{:x}_{}", md5::compute(clean.to_string_lossy().as_bytes()), params.size);
        let cache_dir = get_thumb_cache_dir();
        let cache_path = cache_dir.join(format!("{}.jpg", cache_key));
        if cache_path.exists() { return serve_file(&cache_path, "image/jpeg").await; }

        let output = cache_dir.join(format!("{}.tmp.jpg", cache_key));
        let result = tokio::process::Command::new(ffmpeg_path())
            .arg("-y")
            .arg("-err_detect").arg("ignore_err")  // 容忍不完整文件（.bc! 等）
            .arg("-ss").arg("3")
            .arg("-i").arg(clean.to_string_lossy().to_string())
            .arg("-vframes").arg("1")
            .arg("-vf").arg(format!("scale={}:-1", params.size))
            .arg("-q:v").arg("2")
            .arg("-f").arg("image2")
            .arg(output.to_string_lossy().to_string())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output().await;

        match result {
            Ok(o) if o.status.success() => {
                let _ = tokio::fs::rename(&output, &cache_path).await;
                return serve_file(&cache_path, "image/jpeg").await;
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                tracing::warn!("Thumbnail ffmpeg failed for {:?}: {}", clean, stderr.lines().last().unwrap_or("unknown error"));
                let _ = tokio::fs::remove_file(&output).await;
                return (StatusCode::INTERNAL_SERVER_ERROR, "Thumbnail failed").into_response();
            }
            Err(e) => {
                tracing::warn!("Thumbnail ffmpeg spawn failed for {:?}: {}", clean, e);
                let _ = tokio::fs::remove_file(&output).await;
                return (StatusCode::INTERNAL_SERVER_ERROR, "Thumbnail failed").into_response();
            }
        }
    }

    (StatusCode::NOT_FOUND, "No thumbnail").into_response()
}

async fn serve_file(path: &PathBuf, mime: &str) -> Response {
    match tokio::fs::read(path).await {
        Ok(data) => Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE, mime).header(header::CACHE_CONTROL, "public, max-age=86400").body(Body::from(data)).unwrap(),
        Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}

fn get_thumb_cache_dir() -> PathBuf {
    let dir = if let Ok(a) = std::env::var("LOCALAPPDATA") { PathBuf::from(a).join("LanMediaHub").join("thumbnails") } else { PathBuf::from("thumbnails") };
    let _ = std::fs::create_dir_all(&dir); dir
}