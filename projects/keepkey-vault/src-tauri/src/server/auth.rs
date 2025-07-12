use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use utoipa::ToSchema;

use super::ServerState;

#[derive(Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PairingInfo {
    /// Application name requesting pairing
    pub name: String,
    /// Application URL or identifier  
    pub url: String,
    /// Application icon URL
    pub image_url: String,
    /// When this pairing was added (optional)
    pub added_on: Option<u64>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub api_key: String,
}

#[utoipa::path(
    get,
    path = "/auth/pair",
    responses(
        (status = 200, description = "Auth verification successful", body = AuthResponse),
        (status = 404, description = "No pairing found"),
    ),
    tag = "auth"
)]
pub async fn auth_verify(
    State(_state): State<Arc<ServerState>>,
) -> Result<Json<AuthResponse>, StatusCode> {
    // For now, return a dummy API key
    Ok(Json(AuthResponse {
        api_key: "keepkey-vault-api-key".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/auth/pair",
    request_body = PairingInfo,
    responses(
        (status = 200, description = "Pairing successful", body = AuthResponse),
        (status = 400, description = "Invalid pairing information"),
    ),
    tag = "auth"
)]
pub async fn auth_pair(
    State(_state): State<Arc<ServerState>>,
    Json(_pairing_info): Json<PairingInfo>,
) -> Result<Json<AuthResponse>, StatusCode> {
    // For now, return a dummy API key
    Ok(Json(AuthResponse {
        api_key: "keepkey-vault-api-key".to_string(),
    }))
} 