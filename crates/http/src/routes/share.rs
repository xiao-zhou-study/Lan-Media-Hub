use axum::{
    extract::{Path, State, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;
use crate::server::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct ShareInfoResponse {
    pub id: String, pub name: String, pub path: String,
    pub file_count: u64, pub total_size: u64, pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileInfoResponse {
    pub name: String, pub path: String, pub size: u64,
    pub media_type: String, pub is_dir: bool,
    pub modified: String,
}

pub fn parse_rest(rest: &str) -> (String, String) {
    if let Some((id, sub)) = rest.split_once('/') { (id.to_string(), sub.to_string()) }
    else { (rest.to_string(), String::new()) }
}

pub async fn list_shares(_auth: crate::auth::Auth, State(state): State<AppState>) -> impl IntoResponse {
    let manager = state.manager.read().await;
    Json(manager.get_all_shares().iter().map(|s| ShareInfoResponse {
        id: s.config.id.to_string(), name: s.config.name.clone(),
        path: s.config.path.to_string_lossy().to_string(),
        file_count: s.file_count, total_size: s.total_size,
        status: status_str(s.status),
    }).collect::<Vec<_>>())
}

fn status_str(s: lan_media_hub_core::ShareStatus) -> String {
    match s { lan_media_hub_core::ShareStatus::Active=>"active".into(), lan_media_hub_core::ShareStatus::Paused=>"paused".into(), lan_media_hub_core::ShareStatus::Error=>"error".into() }
}

pub async fn get_share_info(_auth: crate::auth::Auth, Path(id): Path<String>, State(state): State<AppState>) -> Response {
    let uuid = uuid::Uuid::parse_str(&id).ok();
    let manager = state.manager.read().await;
    match uuid.and_then(|u| manager.get_share(u)) {
        Some(s) => (StatusCode::OK, Json(ShareInfoResponse {
            id: s.config.id.to_string(), name: s.config.name.clone(),
            path: s.config.path.to_string_lossy().to_string(),
            file_count: s.file_count, total_size: s.total_size, status: status_str(s.status),
        })).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"Not found"}))).into_response(),
    }
}

pub async fn browse_share(
    _auth: crate::auth::Auth,
    Path(rest): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Response {
    let (id, path) = parse_rest(&rest);
    let manager = state.manager.read().await;
    let uuid = match uuid::Uuid::parse_str(&id) { Ok(u)=>u, Err(_)=>return (StatusCode::BAD_REQUEST,"Invalid ID").into_response() };
    let share = match manager.get_share(uuid) { Some(s)=>s, None=>return (StatusCode::NOT_FOUND,"Not found").into_response() };
    let full_path = if path.is_empty() { share.config.path.clone() } else { share.config.path.join(&path) };
    let clean: PathBuf = path_clean::PathClean::clean(&full_path);
    if !clean.starts_with(&share.config.path) { return (StatusCode::FORBIDDEN,"Out of bounds").into_response() }
    if !clean.exists() { return (StatusCode::NOT_FOUND,"Not found").into_response() }

    if clean.is_file() {
        return (StatusCode::OK, Json(FileInfoResponse {
            name: clean.file_name().and_then(|n|n.to_str()).unwrap_or("?").into(),
            path, size: std::fs::metadata(&clean).map(|m|m.len()).unwrap_or(0),
            media_type: classify(&clean).to_string(), is_dir: false,
            modified: file_modified(&clean),
        })).into_response();
    }

    let mut entries: Vec<FileInfoResponse> = std::fs::read_dir(&clean).into_iter().flatten().flatten().map(|e|{
        let p=e.path(); let d=p.is_dir();
        FileInfoResponse{ name:p.file_name().and_then(|n|n.to_str()).unwrap_or("?").into(), path:p.strip_prefix(&share.config.path).unwrap_or(&p).to_string_lossy().replace('\\',"/"), size:std::fs::metadata(&p).map(|m|m.len()).unwrap_or(0), media_type:if d{"directory".into()}else{classify(&p).to_string()}, is_dir:d, modified: file_modified(&p) }
    }).collect();

    // 排序：默认目录在前，文件在后。支持 ?sort=name|size|modified&order=asc|desc
    let sort_by = params.get("sort").map(|s| s.as_str()).unwrap_or("name");
    let ascending = params.get("order").map(|o| o.as_str()).unwrap_or("asc") == "asc";
    entries.sort_by(|a, b| {
        let cmp = if a.is_dir != b.is_dir { return if a.is_dir { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater } }
        else { match sort_by {
            "size" => a.size.cmp(&b.size),
            "modified" => a.modified.cmp(&b.modified),
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }};
        if ascending { cmp } else { cmp.reverse() }
    });

    (StatusCode::OK, Json(serde_json::json!({"path":path,"entries":entries}))).into_response()
}

pub async fn stream_video(
    _auth: crate::auth::Auth,
    Path(rest): Path<String>,
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Response {
    let (id, file_path) = parse_rest(&rest);
    let manager = state.manager.read().await;
    let uuid = match uuid::Uuid::parse_str(&id) { Ok(u)=>u, Err(_)=>return (StatusCode::BAD_REQUEST,"Bad ID").into_response() };
    let share = match manager.get_share(uuid) { Some(s)=>s, None=>return (StatusCode::NOT_FOUND,"Not found").into_response() };
    let full_path = if file_path.is_empty() { share.config.path.clone() } else { share.config.path.join(&file_path) };
    let clean: PathBuf = path_clean::PathClean::clean(&full_path);
    if !clean.starts_with(&share.config.path) { return (StatusCode::FORBIDDEN,"Out of bounds").into_response() }
    if !clean.exists() || !clean.is_file() { return (StatusCode::NOT_FOUND,"Not found").into_response() }
    stream_file(clean, &headers).await
}

/// 流式传输文件，支持 Range 请求（视频 seek 的关键）
async fn stream_file(path: PathBuf, headers: &axum::http::HeaderMap) -> Response {
    use tokio::fs::File;
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    use axum::body::Body;
    use tokio_util::io::ReaderStream;

    let mut file = match File::open(&path).await { Ok(f)=>f, Err(_)=>return (StatusCode::INTERNAL_SERVER_ERROR,"Open failed").into_response() };
    let file_size = match file.metadata().await { Ok(m)=>m.len(), Err(_)=>return (StatusCode::INTERNAL_SERVER_ERROR,"Metadata failed").into_response() };
    let mime = mime_type(&path);

    // Range 请求处理
    let range = headers.get(header::RANGE).and_then(|v|v.to_str().ok()).and_then(|s|parse_range(s,file_size));

    match range {
        Some((start, end)) => {
            let length = end - start + 1;
            // 大 Range 也流式传输，避免内存爆炸
            if length > 4 * 1024 * 1024 {
                // >4MB: 分块流式发送
                if file.seek(std::io::SeekFrom::Start(start)).await.is_err() { return (StatusCode::INTERNAL_SERVER_ERROR,"Seek failed").into_response() }
                let take = tokio::io::AsyncReadExt::take(file, length);
                let stream = ReaderStream::with_capacity(take, 256 * 1024);
                Response::builder().status(StatusCode::PARTIAL_CONTENT)
                    .header(header::CONTENT_TYPE, mime)
                    .header(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end, file_size))
                    .header(header::CONTENT_LENGTH, length)
                    .header(header::ACCEPT_RANGES, "bytes")
                    .header(header::CACHE_CONTROL, "public, max-age=3600")
                    .body(Body::from_stream(stream)).unwrap()
            } else {
                if file.seek(std::io::SeekFrom::Start(start)).await.is_err() { return (StatusCode::INTERNAL_SERVER_ERROR,"Seek failed").into_response() }
                let mut buf = vec![0u8; length as usize];
                if file.read_exact(&mut buf).await.is_err() { return (StatusCode::INTERNAL_SERVER_ERROR,"Read failed").into_response() }
                Response::builder().status(StatusCode::PARTIAL_CONTENT)
                    .header(header::CONTENT_TYPE, mime)
                    .header(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end, file_size))
                    .header(header::CONTENT_LENGTH, length)
                    .header(header::ACCEPT_RANGES, "bytes")
                    .header(header::CACHE_CONTROL, "public, max-age=3600")
                    .body(Body::from(buf)).unwrap()
            }
        }
        None => {
            let stream = ReaderStream::with_capacity(file, 256 * 1024);
            Response::builder().status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CONTENT_LENGTH, file_size)
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CACHE_CONTROL, "public, max-age=3600")
                .body(Body::from_stream(stream)).unwrap()
        }
    }
}

fn parse_range(s: &str, file_size: u64) -> Option<(u64, u64)> {
    let s = s.strip_prefix("bytes=")?;
    let (start_s, end_s) = s.split_once('-')?;
    let start: u64 = start_s.parse().ok()?;
    if end_s.is_empty() { if start < file_size { Some((start, file_size - 1)) } else { None } }
    else { let end: u64 = end_s.parse().ok()?; if start <= end && end < file_size { Some((start, end)) } else { None } }
}

pub async fn upload_file(
    _auth: crate::auth::Auth,
    State(state): State<AppState>, Path(rest): Path<String>,
    mut multipart: axum::extract::Multipart,
) -> Response {
    let (id, target_path) = parse_rest(&rest);
    let manager = state.manager.read().await;
    let uuid = match uuid::Uuid::parse_str(&id) { Ok(u)=>u, Err(_)=>return (StatusCode::BAD_REQUEST,"Bad ID").into_response() };
    let share = match manager.get_share(uuid) { Some(s)=>s, None=>return (StatusCode::NOT_FOUND,"Not found").into_response() };

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.file_name().unwrap_or("unknown").to_string();
        let data = match field.bytes().await { Ok(d)=>d, Err(_)=>continue };
        let dest = if target_path.is_empty() { share.config.path.join(&name) } else { share.config.path.join(&target_path).join(&name) };
        let clean: PathBuf = path_clean::PathClean::clean(&dest);
        if !clean.starts_with(&share.config.path) { return (StatusCode::FORBIDDEN,"Out of bounds").into_response() }
        if tokio::fs::write(&clean, &data).await.is_err() { return (StatusCode::INTERNAL_SERVER_ERROR,"Write error").into_response() }
        return (StatusCode::OK, clean.to_string_lossy().to_string()).into_response();
    }
    (StatusCode::BAD_REQUEST, "No file").into_response()
}

fn classify(p: &PathBuf) -> &'static str {
    // 完整文件名（处理 .mp4.bc! 这种 BitComet 未完成下载）
    let _full_name = p.file_name().and_then(|n|n.to_str()).unwrap_or("").to_lowercase();
    let ext = p.extension().and_then(|e|e.to_str()).map(|e|e.to_lowercase()).unwrap_or_default();
    // 扩展名是 bc! 时，看前面的扩展名
    let real_ext = if ext == "bc!" {
        p.with_extension("").extension().and_then(|e|e.to_str()).map(|e|e.to_lowercase()).unwrap_or_default()
    } else { ext };

    match real_ext.as_str() {
        "mp4"|"mkv"|"avi"|"mov"|"webm"|"wmv"|"flv"
        |"mpg"|"mpeg"|"ts"|"mts"|"m2ts"|"vob"
        |"rm"|"rmvb"|"3gp"|"asf"|"divx"|"ogv"|"m4v"=>"video",
        "mp3"|"flac"|"wav"|"aac"|"ogg"|"m4a"=>"audio",
        "jpg"|"jpeg"|"png"|"gif"|"bmp"|"webp"=>"image",
        _=>"file",
    }
}

fn mime_type(p: &PathBuf) -> &'static str {
    match p.extension().and_then(|e|e.to_str()).map(|e|e.to_lowercase()).as_deref() {
        Some("mp4")|Some("m4v")=>"video/mp4", Some("mkv")=>"video/x-matroska", Some("webm")=>"video/webm",
        Some("avi")|Some("divx")=>"video/x-msvideo", Some("mov")=>"video/quicktime",
        Some("mpg")|Some("mpeg")=>"video/mpeg", Some("wmv")|Some("asf")=>"video/x-ms-wmv",
        Some("flv")=>"video/x-flv", Some("ogv")|Some("ogg")=>"video/ogg",
        Some("mp3")=>"audio/mpeg", Some("flac")=>"audio/flac", Some("wav")=>"audio/wav",
        Some("aac")=>"audio/aac", Some("m4a")=>"audio/mp4",
        Some("jpg")|Some("jpeg")=>"image/jpeg", Some("png")=>"image/png",
        Some("gif")=>"image/gif", Some("webp")=>"image/webp",
        _=>"application/octet-stream",
    }
}

fn file_modified(p: &PathBuf) -> String {
    if let Ok(meta) = std::fs::metadata(p) {
        if let Ok(m) = meta.modified() {
            if let Ok(d) = m.duration_since(std::time::UNIX_EPOCH) {
                if let Some(dt) = chrono::DateTime::from_timestamp(d.as_secs() as i64, 0) {
                    return dt.format("%Y-%m-%d %H:%M").to_string();
                }
            }
        }
    }
    String::new()
}