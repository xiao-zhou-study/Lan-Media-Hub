use axum::{
    extract::{Path, State, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use axum::body::Body;
use bytes::Bytes;
use std::path::PathBuf;
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_stream::wrappers::ReceiverStream;
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

fn transcode_cache_dir() -> PathBuf {
    let dir = if let Ok(a) = std::env::var("LOCALAPPDATA") { PathBuf::from(a).join("LanMediaHub").join("transcode") } else { PathBuf::from("transcode") };
    let _ = std::fs::create_dir_all(&dir); dir
}

/// 检测 Intel QSV 硬件加速是否可用（一次检测，缓存结果）
fn qsv_available() -> bool {
    use std::sync::OnceLock;
    static QSV: OnceLock<bool> = OnceLock::new();
    *QSV.get_or_init(|| {
        std::process::Command::new(ffmpeg_path())
            .args(["-hide_banner", "-encoders"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("h264_qsv"))
            .unwrap_or(false)
    })
}

/// ffprobe 检测源视频编码
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

/// 流式转码 + 磁盘缓存
/// - 首次播放：FFmpeg 边写缓存文件边通过 channel 流式传给客户端
/// - 后续播放/seek：直接走缓存文件，支持 Range
pub async fn transcode_video(
    _auth: crate::auth::Auth,
    Path(rest): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: axum::http::HeaderMap,
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

    // 先算缓存 key（只基于路径，不做 ffprobe），命中则直接返回
    let path_hash = format!("{:x}", md5::compute(clean.to_string_lossy().as_bytes()));
    // 尝试三种可能的缓存 key（未知编码策略时先试几个）
    for profile in &["remux", "qsv", "sw"] {
        let probe_path = transcode_cache_dir().join(format!("{}_{}", path_hash, profile));
        if probe_path.exists() {
            return super::share::serve_cached_file(&probe_path, &headers).await;
        }
    }

    let codec = probe_video_codec(&clean).await;
    let can_remux = codec == "h264";
    let is_bc = clean.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase() == "bc!";
    let use_qsv = !can_remux && qsv_available() && !is_bc;

    let profile = if can_remux { "remux" } else if use_qsv { "qsv" } else { "sw" };
    let cache_key = format!("{}_{}", path_hash, profile);
    let cache_path = transcode_cache_dir().join(&cache_key);

    // 缓存命中 → 直接走文件流
    if cache_path.exists() {
        return super::share::serve_cached_file(&cache_path, &headers).await;
    }

    let start = params.get("start").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);

    // 写临时文件，成功后 rename 到缓存路径（避免损坏文件被服务）
    let tmp_path = cache_path.with_extension("tmp.mp4");

    let mut cmd = tokio::process::Command::new(ffmpeg_path());

    // QSV 硬件加速：解码也走 GPU，零拷贝到编码器
    if use_qsv {
        cmd.arg("-hwaccel").arg("qsv")
           .arg("-hwaccel_output_format").arg("qsv");
    }

    if start > 0.0 {
        cmd.arg("-ss").arg(format!("{:.3}", start));
    }
    cmd.arg("-i").arg(clean.to_string_lossy().to_string());

    if can_remux {
        cmd.arg("-c:v").arg("copy");
    } else if use_qsv {
        cmd.arg("-c:v").arg("h264_qsv")
           .arg("-preset").arg("veryfast")
           .arg("-global_quality").arg("23");
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
       .arg(tmp_path.to_string_lossy().to_string())
       .stdout(std::process::Stdio::null())
       .stderr(std::process::Stdio::null());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "FFmpeg failed to start").into_response(),
    };

    // 等缓存文件开始写入
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let cache = tmp_path.clone();
    let final_path = cache_path.clone();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(64);

    // 后台任务：边读缓存文件边发到 channel，FFmpeg 退出后结束
    tokio::spawn(async move {
        match tokio::fs::File::open(&cache).await {
            Ok(mut file) => {
                let mut pos = 0u64;
                loop {
                    match file.metadata().await {
                        Ok(meta) if meta.len() > pos => {
                            let to_read = ((meta.len() - pos) as usize).min(256 * 1024);
                            if file.seek(std::io::SeekFrom::Start(pos)).await.is_err() { break; }
                            let mut buf = vec![0u8; to_read];
                            match file.read_exact(&mut buf).await {
                                Ok(_) => {
                                    pos += to_read as u64;
                                    if tx.send(Ok(Bytes::from(buf))).await.is_err() { break; }
                                }
                                Err(_) => break,
                            }
                        }
                        Err(_) => break,
                        _ => {}
                    }
                    // 检查 FFmpeg 是否还在跑；如果已退出且无新数据 → 结束
                    match child.try_wait() {
                        Ok(Some(_)) => {
                            // FFmpeg 退出，读完最后的数据
                            match file.metadata().await {
                                Ok(meta) if meta.len() > pos => continue,
                                _ => break,
                            }
                        }
                        Ok(None) => {} // 还在跑，继续等
                        Err(_) => break,
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                }
            }
            Err(_) => {}
        }
        // FFmpeg 成功后 rename，失败则清理 tmp
        if child.wait().await.map(|s| s.success()).unwrap_or(false) {
            let _ = tokio::fs::rename(&cache, &final_path).await;
        } else {
            let _ = tokio::fs::remove_file(&cache).await;
        }
        tracing::debug!("transcode cache done: {:?}", final_path);
    });

    // 清理旧缓存：总大小超过 5GB 时删除最旧文件
    tokio::spawn(cleanup_old_cache());

    let stream = ReceiverStream::new(rx);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(stream))
        .unwrap()
}

/// 清理旧转码缓存，总大小超 5GB 时删除最旧文件（同时只运行一个）
async fn cleanup_old_cache() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING.swap(true, Ordering::AcqRel) { return; }

    let dir = transcode_cache_dir();
    let mut entries: Vec<_> = match tokio::fs::read_dir(&dir).await {
        Ok(mut rd) => {
            let mut v = Vec::new();
            while let Ok(Some(e)) = rd.next_entry().await {
                if let Ok(meta) = e.metadata().await {
                    if meta.is_file() {
                        if let Ok(modified) = meta.modified() {
                            v.push((e.path(), meta.len(), modified));
                        }
                    }
                }
            }
            v
        }
        Err(_) => return,
    };

    let total: u64 = entries.iter().map(|(_, s, _)| s).sum();
    if total < 5 * 1024 * 1024 * 1024 { return; } // < 5GB

    entries.sort_by_key(|(_, _, m)| *m); // 按修改时间排序，最旧的在前
    let mut to_free = total - 4 * 1024 * 1024 * 1024; // 留 4GB
    for (p, s, _) in entries {
        if to_free == 0 { break; }
        let _ = tokio::fs::remove_file(&p).await;
        to_free = to_free.saturating_sub(s);
    }
    RUNNING.store(false, Ordering::Release);
}

/// HLS 分片兼容
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
