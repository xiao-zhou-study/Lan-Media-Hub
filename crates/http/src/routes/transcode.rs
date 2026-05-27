use axum::{
    extract::{Path, State, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use axum::body::Body;
use std::path::PathBuf;
use std::collections::HashMap;
use tokio_util::io::ReaderStream;
use crate::server::AppState;
use super::share::parse_rest;

fn ffmpeg_path() -> String {
    ffmpeg_sidecar::paths::ffmpeg_path().to_string_lossy().to_string()
}

fn ffprobe_path() -> String {
    let ffmpeg = ffmpeg_sidecar::paths::ffmpeg_path();
    ffmpeg.with_file_name(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" })
        .to_string_lossy().to_string()
}

/// ffprobe 检测源视频编码，判断是否能直接 remux（无需重编码）
async fn probe_video_codec(path: &PathBuf) -> String {
    match tokio::process::Command::new(ffprobe_path())
        .arg("-v").arg("error")
        .arg("-select_streams").arg("v:0")
        .arg("-show_entries").arg("stream=codec_name")
        .arg("-of").arg("csv=p=0")
        .arg(path.to_string_lossy().to_string())
        .output().await
    {
        Ok(o) => String::from_utf8(o.stdout).unwrap_or_default().trim().to_string(),
        Err(_) => String::new(),
    }
}

/// 实时流转码 / remux
/// - H.264 源：直接 remux（-c:v copy），CPU 占用 < 5%，秒开
/// - 其他编码：重编码到 H.264（-c:v libx264），CPU 高但兼容所有格式
pub async fn transcode_video(
    _auth: crate::auth::Auth,
    Path(rest): Path<String>,
    Query(params): Query<HashMap<String, String>>,
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

    let start = params.get("start").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);

    // 检测源编码：H.264 → remux，否则 → 重编码
    let codec = probe_video_codec(&clean).await;
    let can_remux = codec == "h264";
    tracing::debug!(?codec, can_remux, "transcode decision");

    let mut cmd = tokio::process::Command::new(ffmpeg_path());
    if start > 0.0 {
        cmd.arg("-ss").arg(format!("{:.3}", start));
    }
    cmd.arg("-i").arg(clean.to_string_lossy().to_string());

    if can_remux {
        cmd.arg("-c:v").arg("copy");
    } else {
        cmd.arg("-c:v").arg("libx264")
           .arg("-preset").arg("ultrafast")
           .arg("-crf").arg("23")
           .arg("-vf").arg("scale=trunc(iw/2)*2:trunc(ih/2)*2");
    }

    cmd.arg("-c:a").arg("aac")
       .arg("-b:a").arg("128k")
       .arg("-movflags").arg("frag_keyframe+empty_moov")
       .arg("-f").arg("mp4")
       .arg("pipe:1")
       .stdout(std::process::Stdio::piped())
       .stderr(std::process::Stdio::null());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "FFmpeg failed to start").into_response(),
    };

    let stdout = child.stdout.take().unwrap();

    tokio::spawn(async move { let _ = child.wait().await; });

    let stream = ReaderStream::with_capacity(stdout, 256 * 1024);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(stream))
        .unwrap()
}

/// HLS 分片兼容（旧缓存可继续使用，新方案不再生成）
pub async fn hls_segment(
    _auth: crate::auth::Auth,
    Path(rest): Path<String>,
    State(_state): State<AppState>,
) -> Response {
    let dir = if let Ok(a) = std::env::var("LOCALAPPDATA") { PathBuf::from(a).join("LanMediaHub").join("hls") } else { PathBuf::from("hls") };
    let (hash, seg) = if let Some((a, b)) = rest.split_once('/') { (a, b) } else { return (StatusCode::BAD_REQUEST, "Bad path").into_response() };
    let path = dir.join(hash).join(seg);
    match tokio::fs::read(&path).await {
        Ok(data) => Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE, "video/mp2t").header(header::CACHE_CONTROL, "public, max-age=3600").body(Body::from(data)).unwrap(),
        Err(_) => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}
