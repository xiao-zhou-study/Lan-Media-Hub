use axum::{
    extract::{Path, State, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use axum::body::Body;
use std::path::PathBuf;
use std::collections::HashMap;
use crate::server::AppState;
use super::share::parse_rest;

fn ffmpeg_path() -> String {
    ffmpeg_sidecar::paths::ffmpeg_path().to_string_lossy().to_string()
}

fn hls_cache_root() -> PathBuf {
    let dir = if let Ok(a) = std::env::var("LOCALAPPDATA") { PathBuf::from(a).join("LanMediaHub").join("hls") } else { PathBuf::from("hls") };
    let _ = std::fs::create_dir_all(&dir); dir
}

/// HLS 实时转码——返回 .m3u8，分片通过 /api/hls/ 提供
pub async fn transcode_video(
    _auth: crate::auth::Auth,
    Path(rest): Path<String>,
    Query(_params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Response {
    let (id, file_path) = parse_rest(&rest);
    let manager = state.manager.read().await;
    let uuid = match uuid::Uuid::parse_str(&id) { Ok(u)=>u, Err(_)=>return (StatusCode::BAD_REQUEST,"Bad ID").into_response() };
    let share = match manager.get_share(uuid) { Some(s)=>s, None=>return (StatusCode::NOT_FOUND,"Not found").into_response() };
    let full_path = if file_path.is_empty() { share.config.path.clone() } else { share.config.path.join(&file_path) };
    let clean: PathBuf = path_clean::PathClean::clean(&full_path);
    if !clean.starts_with(&share.config.path) { return (StatusCode::FORBIDDEN,"Out of bounds").into_response() }
    if !clean.exists() || !clean.is_file() { return (StatusCode::NOT_FOUND,"Not found").into_response() }

    let hash = format!("{:x}", md5::compute(clean.to_string_lossy().as_bytes()));
    let dir = hls_cache_root().join(&hash); let _ = std::fs::create_dir_all(&dir);
    let playlist = dir.join("playlist.m3u8");

    if playlist.exists() {
        if let Ok(mut content) = tokio::fs::read_to_string(&playlist).await {
            // 替换相对路径为绝对 API 路径
            content = content.replace("seg_", &format!("/api/hls/{}/seg_", hash));
            return (StatusCode::OK, [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")], content).into_response();
        }
    }

    let seg_pat = dir.join("seg_%03d.ts");
    let result = tokio::process::Command::new(ffmpeg_path())
        .arg("-y").arg("-i").arg(clean.to_string_lossy().to_string())
        .arg("-c:v").arg("libx264").arg("-preset").arg("ultrafast").arg("-crf").arg("23")
        .arg("-c:a").arg("aac").arg("-b:a").arg("128k")
        .arg("-vf").arg("scale=trunc(iw/2)*2:trunc(ih/2)*2")
        .arg("-f").arg("hls").arg("-hls_time").arg("4").arg("-hls_list_size").arg("0")
        .arg("-hls_segment_filename").arg(seg_pat.to_string_lossy().to_string())
        .arg(playlist.to_string_lossy().to_string())
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .status().await;

    match result {
        Ok(s) if s.success() => {
            if let Ok(mut content) = tokio::fs::read_to_string(&playlist).await {
                content = content.replace("seg_", &format!("/api/hls/{}/seg_", hash));
                return (StatusCode::OK, [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")], content).into_response();
            }
        }
        _ => {}
    }
    (StatusCode::INTERNAL_SERVER_ERROR, "Transcode failed").into_response()
}

/// 服务 HLS .ts 分片
pub async fn hls_segment(
    Path(rest): Path<String>,
    State(_state): State<AppState>,
) -> Response {
    let (hash, seg) = if let Some((a, b)) = rest.split_once('/') { (a, b) } else { return (StatusCode::BAD_REQUEST, "Bad path").into_response() };
    let path = hls_cache_root().join(hash).join(seg);
    match tokio::fs::read(&path).await {
        Ok(data) => Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE, "video/mp2t").header(header::CACHE_CONTROL, "public, max-age=3600").body(Body::from(data)).unwrap(),
        Err(_) => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}