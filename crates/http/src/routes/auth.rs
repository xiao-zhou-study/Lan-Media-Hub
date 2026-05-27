use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::server::AppState;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

/// 密码验证接口（每次读取最新密码）
pub async fn verify_password(
    Query(params): Query<HashMap<String, String>>,
    pw_state: State<Arc<RwLock<String>>>,
) -> Response {
    let current_pw = pw_state.read().await;
    if current_pw.is_empty() {
        return (StatusCode::OK, "ok").into_response();
    }
    if params.get("pw").map(|p| p == &*current_pw).unwrap_or(false) {
        (StatusCode::OK, "ok").into_response()
    } else {
        (StatusCode::UNAUTHORIZED, "wrong password").into_response()
    }
}

/// JWT 登录接口（POST JSON: { "password": "..." }）
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Response {
    let pw = state.password.read().await;
    if pw.is_empty() || body.password == *pw {
        let secret = state.jwt_secret.read().await;
        match crate::auth::create_token(&secret) {
            Ok(token) => (StatusCode::OK, Json(serde_json::json!({ "token": token }))).into_response(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "token creation failed" }))).into_response(),
        }
    } else {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "wrong password" }))).into_response()
    }
}
