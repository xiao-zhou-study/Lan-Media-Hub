use crate::routes::share::{list_shares, get_share_info, browse_share, stream_video, upload_file};
use crate::routes::auth::{verify_password, login};
use crate::routes::transcode::{transcode_video, hls_segment};
use crate::routes::thumbnail::get_thumbnail;
use crate::routes::info::get_file_info;
use axum::Router;
use crate::server::AppState;

/// 创建 API 路由
pub fn create_api_router() -> Router<AppState> {
    Router::new()
        .route("/auth", axum::routing::get(verify_password))
        .route("/login", axum::routing::post(login))
        .route("/shares", axum::routing::get(list_shares))
        .route("/shares/:id", axum::routing::get(get_share_info))
        .route("/browse/*rest", axum::routing::get(browse_share))
        .route("/stream/*rest", axum::routing::get(stream_video))
        .route("/transcode/*rest", axum::routing::get(transcode_video))
        .route("/hls/*rest", axum::routing::get(hls_segment))
        .route("/thumbnail/*rest", axum::routing::get(get_thumbnail))
        .route("/info/*rest", axum::routing::get(get_file_info))
        .route("/upload/*rest", axum::routing::post(upload_file))
}
