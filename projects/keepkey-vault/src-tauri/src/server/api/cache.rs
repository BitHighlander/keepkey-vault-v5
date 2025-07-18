use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use std::sync::Arc;
use crate::server::ServerState;

#[derive(Serialize)]
pub struct CacheStatusResponse {
    pub available: bool,
    pub cached_pubkeys: usize,
    pub cached_balances: usize,
    pub last_updated: Option<i64>,
}

/// Get cache status for pioneer-sdk detection
pub async fn get_cache_status(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<CacheStatusResponse>, StatusCode> {
    let cache = crate::commands::get_cache_manager(&state.cache_manager).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Get actual cached counts from SQLite database
    let cached_pubkeys = match cache.count_cached_pubkeys().await {
        Ok(count) => count,
        Err(e) => {
            log::warn!("Failed to count cached pubkeys: {}", e);
            0
        }
    };
    
    let cached_balances = match cache.count_cached_balances().await {
        Ok(count) => count,
        Err(e) => {
            log::warn!("Failed to count cached balances: {}", e);
            0
        }
    };
    
    log::debug!("Cache status: {} pubkeys, {} balances", cached_pubkeys, cached_balances);
    
    Ok(Json(CacheStatusResponse {
        available: true,
        cached_pubkeys,
        cached_balances,
        last_updated: Some(chrono::Utc::now().timestamp()),
    }))
} 