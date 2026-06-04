use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::IpAddr;
use crate::server::AppState;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

/// 从请求头获取客户端 IP
fn get_client_ip(headers: &axum::http::HeaderMap) -> IpAddr {
    headers
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or_else(|| "0.0.0.0".parse().unwrap())
}

/// 密码验证接口（每次读取最新密码）
pub async fn verify_password(
    Query(params): Query<HashMap<String, String>>,
    headers: axum::http::HeaderMap,
    State(state): State<AppState>,
) -> Response {
    let ip = get_client_ip(&headers);

    // 检查限速
    if state.rate_limiter.is_rate_limited(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({ "error": "Too many attempts, try again later" }))).into_response();
    }

    let current_pw = state.password.read().await;
    if current_pw.is_empty() {
        return (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response();
    }

    let is_valid = params.get("pw").map(|p| p == &*current_pw).unwrap_or(false);
    if is_valid {
        state.rate_limiter.clear(&ip).await;
        (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response()
    } else {
        state.rate_limiter.record_failure(&ip).await;
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "wrong password" }))).into_response()
    }
}

/// JWT 登录接口（POST JSON: { "password": "..." }）
pub async fn login(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Response {
    let ip = get_client_ip(&headers);

    // 检查限速
    if state.rate_limiter.is_rate_limited(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({ "error": "Too many attempts, try again later" }))).into_response();
    }

    let pw = state.password.read().await;
    if pw.is_empty() || body.password == *pw {
        state.rate_limiter.clear(&ip).await;
        let secret = state.jwt_secret.read().await;
        match crate::auth::create_token(&secret) {
            Ok(token) => (StatusCode::OK, Json(serde_json::json!({ "token": token }))).into_response(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "token creation failed" }))).into_response(),
        }
    } else {
        state.rate_limiter.record_failure(&ip).await;
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "wrong password" }))).into_response()
    }
}
