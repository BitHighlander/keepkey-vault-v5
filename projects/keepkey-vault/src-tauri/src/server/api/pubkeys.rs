use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::server::ServerState;

#[derive(Deserialize)]
pub struct PubkeyPath {
    pub address_n: Vec<u32>,
    pub script_type: Option<String>,
    pub networks: Vec<String>,
    #[serde(rename = "type")]
    pub path_type: Option<String>,
    pub note: Option<String>,
}

#[derive(Deserialize)]
pub struct BatchPubkeysRequest {
    pub paths: Vec<PubkeyPath>,
    pub context: String,
}

#[derive(Serialize)]
pub struct BatchPubkey {
    pub pubkey: String,
    pub address: String,
    pub path: String,
    #[serde(rename = "pathMaster")]
    pub path_master: String,
    #[serde(rename = "scriptType")]
    pub script_type: String,
    pub networks: Vec<String>,
    #[serde(rename = "type")]
    pub path_type: String,
    pub note: Option<String>,
    pub context: String,
}

#[derive(Serialize)]
pub struct BatchPubkeysResponse {
    pub pubkeys: Vec<BatchPubkey>,
    pub cached_count: usize,
    pub total_requested: usize,
    pub device_id: Option<String>,
}

/// Batch get pubkeys from cache - used for performance optimization
pub async fn batch_get_pubkeys(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<BatchPubkeysRequest>,
) -> Result<Json<BatchPubkeysResponse>, StatusCode> {
    println!("🚀 [VAULT BATCH] Received batch request for {} paths", request.paths.len());
    
    let cache = crate::commands::get_cache_manager(&state.cache_manager).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let mut cached_pubkeys = Vec::new();
    let mut cached_count = 0;
    
    // Look up each path in the SQLite cache
    for (index, path_request) in request.paths.iter().enumerate() {
        // Convert address_n to BIP32 path string for lookup
        let bip32_path = address_n_to_bip32(&path_request.address_n);
        
        println!("🔍 [VAULT BATCH] Checking cache for path: {}", bip32_path);
        
        // Try to get cached pubkey from database
        match cache.get_cached_pubkey_by_path(&bip32_path).await {
            Ok(Some(cached_pubkey)) => {
                println!("✅ [VAULT BATCH] Cache HIT for path: {}", bip32_path);
                
                // Convert cached pubkey to response format
                cached_pubkeys.push(BatchPubkey {
                    pubkey: cached_pubkey.xpub.clone().unwrap_or_else(|| cached_pubkey.address.clone().unwrap_or_default()),
                    address: cached_pubkey.address.clone().unwrap_or_default(),
                    path: bip32_path.clone(),
                    path_master: bip32_path, // For now, use same as path
                    script_type: cached_pubkey.script_type.unwrap_or_default(),
                    networks: path_request.networks.clone(),
                    path_type: "cached".to_string(),
                    note: Some(format!("Cached from device {}", cached_pubkey.device_id)),
                    context: request.context.clone(),
                });
                cached_count += 1;
            }
            Ok(None) => {
                println!("❌ [VAULT BATCH] Cache MISS for path: {}", bip32_path);
            }
            Err(e) => {
                println!("⚠️ [VAULT BATCH] Cache lookup error for path {}: {}", bip32_path, e);
            }
        }
    }
    
    println!("✅ [VAULT BATCH] Batch response: {} cached, {} total", cached_count, request.paths.len());
    
    Ok(Json(BatchPubkeysResponse {
        pubkeys: cached_pubkeys,
        cached_count,
        total_requested: request.paths.len(),
        device_id: Some("343737340F4736331F003B00".to_string()), // TODO: Get actual device ID
    }))
}

/// Convert address_n array to BIP32 path string
fn address_n_to_bip32(address_n: &[u32]) -> String {
    let mut path = "m".to_string();
    for &n in address_n {
        if n >= 0x80000000 {
            // Hardened derivation - subtract the hardened offset and add apostrophe
            path.push_str(&format!("/{}'", n - 0x80000000));
        } else {
            // Normal derivation
            path.push_str(&format!("/{}", n));
        }
    }
    path
} 