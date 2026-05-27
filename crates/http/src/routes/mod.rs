pub mod api;
pub mod share;
pub mod auth;
pub mod transcode;
pub mod thumbnail;
pub mod info;
pub mod web_ui;

use axum::Router;
use axum::response::{Response, Html, IntoResponse};
use axum::body::Body;
use axum::routing::get;
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;
use tower_http::services::ServeDir;
use crate::server::AppState;
use std::path::PathBuf;

pub fn create_router() -> Router<AppState> {
    let web_dir = resolve_web_dir();

    let web_dir1 = web_dir.clone();
    let web_dir2 = web_dir.clone();
    Router::new()
        .nest("/api", api::create_api_router())
        .nest_service("/assets", ServeDir::new(web_dir.join("assets")))
        .route("/", get(move || serve_index(web_dir1)))
        .fallback(get(move || serve_index(web_dir2)))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http())
}

async fn serve_index(web_dir: PathBuf) -> Response {
    let index = web_dir.join("index.html");
    match tokio::fs::read_to_string(&index).await {
        Ok(html) => Html(html).into_response(),
        Err(_) => Response::builder().status(404).body(Body::from("Not Found - build web-ui first: cd web-ui && npm run build")).unwrap(),
    }
}

fn resolve_web_dir() -> PathBuf {
    let candidates = [PathBuf::from("web-ui/dist"), PathBuf::from("../web-ui/dist")];
    for p in &candidates { if p.join("index.html").exists() { return p.clone(); } }
    candidates[0].clone()
}