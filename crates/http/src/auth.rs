use axum::{
    extract::{FromRequestParts, FromRef},
    http::{request::Parts, StatusCode},
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::future::Future;
use crate::server::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims { pub sub: String, pub exp: usize, pub iat: usize }

pub struct Auth;

fn get_query_param(uri: &axum::http::Uri, key: &str) -> Option<String> {
    uri.query().and_then(|q| {
        q.split('&').find_map(|p| {
            let mut kv = p.splitn(2, '=');
            if kv.next()? == key { kv.next().map(|v| v.to_string()) } else { None }
        })
    })
}

impl<S> FromRequestParts<S> for Auth
where AppState: FromRef<S>, S: Send + Sync
{
    type Rejection = StatusCode;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        state: &'life1 S,
    ) -> Pin<Box<dyn Future<Output = Result<Self, Self::Rejection>> + Send + 'async_trait>>
    where 'life0: 'async_trait, 'life1: 'async_trait, Self: 'async_trait
    {
        let state = AppState::from_ref(state);
        let query_token = get_query_param(&parts.uri, "token");
        let query_pw = get_query_param(&parts.uri, "pw").unwrap_or_default();

        Box::pin(async move {
            let current_pw = state.password.read().await;
            if current_pw.is_empty() { return Ok(Auth); }
            let secret = state.jwt_secret.read().await;

            // Bearer header
            if let Some(t) = parts.headers.get("Authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
            {
                return decode::<Claims>(t, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default())
                    .map(|_| Auth).map_err(|_| StatusCode::UNAUTHORIZED);
            }

            // ?token= JWT
            if let Some(ref t) = query_token {
                return decode::<Claims>(t, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default())
                    .map(|_| Auth).map_err(|_| StatusCode::UNAUTHORIZED);
            }

            // ?pw= 明文密码
            if query_pw == *current_pw { return Ok(Auth); }

            Err(StatusCode::UNAUTHORIZED)
        })
    }
}

pub fn create_token(secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as usize;
    encode(&Header::default(), &Claims { sub: "lan-media-hub".into(), iat: now, exp: now + 86400 }, &EncodingKey::from_secret(secret.as_bytes()))
}

pub fn generate_secret() -> String {
    use sha2::{Sha256, Digest};
    let seed = format!("{}:{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(), uuid::Uuid::new_v4());
    format!("{:x}", Sha256::digest(seed.as_bytes()))
}
