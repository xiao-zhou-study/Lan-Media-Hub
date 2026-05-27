use axum::{http::header, response::IntoResponse};

/// 移动端 Web UI，在编译时嵌入
pub async fn serve_web_ui() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], MOBILE_UI_HTML)
}

const MOBILE_UI_HTML: &str = include_str!("../assets/mobile_ui.html");