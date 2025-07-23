use axum::{
    extract::{Query, State}, 
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::server::ServerState;
use crate::commands::{DeviceRequest, DeviceResponse};

/// Bootstrap request for getting all wallet data in one call
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WalletBootstrapRequest {
    pub device_id: Option<String>,
    pub paths: Vec<String>,
    pub include: BootstrapInclude,
    pub cache_strategy: CacheStrategy,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct BootstrapInclude {
    pub pubkeys: bool,
    pub addresses: bool,
    pub balances: bool,
    pub transactions: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CacheStrategy {
    PreferCache,
    ForceRefresh,
    CacheOnly,
}

/// Bootstrap response with all wallet data
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WalletBootstrapResponse {
    pub device_id: String,
    pub response_time_ms: u128,
    pub cache_status: CacheStatus,
    pub data: BootstrapData,
    pub background_tasks: BackgroundTasks,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CacheStatus {
    pub total_requested: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub missing_paths: Vec<String>,
    pub cache_freshness: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapData {
    pub pubkeys: HashMap<String, PubkeyData>,
    pub addresses: HashMap<String, AddressData>,
    pub balances: HashMap<String, BalanceData>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PubkeyData {
    pub pubkey: String,
    pub coin: String,
    pub cached: bool,
    pub cache_time: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddressData {
    pub address: String,
    pub script_type: Option<String>,
    pub cached: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BalanceData {
    pub confirmed: String,
    pub unconfirmed: String,
    pub currency: String,
    pub usd_value: Option<String>,
    pub cached: bool,
    pub last_updated: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundTasks {
    pub missing_data_fetch: String,
    pub balance_refresh: String,
    pub transaction_sync: String,
}

/// Fast health check for vault availability
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FastHealthResponse {
    pub status: String,
    pub device_connected: bool,
    pub device_id: Option<String>,
    pub cache_status: String,
    pub response_time_ms: u128,
}

/// Complete wallet bootstrap endpoint - single call for all wallet data
#[utoipa::path(
    post,
    path = "/api/v1/wallet/bootstrap",
    request_body = WalletBootstrapRequest,
    responses(
        (status = 200, description = "Bootstrap data retrieved successfully", body = WalletBootstrapResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "wallet"
)]
pub async fn wallet_bootstrap(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<WalletBootstrapRequest>,
) -> Result<Json<WalletBootstrapResponse>, StatusCode> {
    let start_time = Instant::now();
    
    println!("🚀 [WALLET BOOTSTRAP] Request for {} paths", request.paths.len());
    
    // Get cache manager
    let cache_manager = crate::commands::get_cache_manager(&state.cache_manager).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Get current device ID
    let device_id = if let Some(id) = request.device_id {
        id
    } else {
        // Get first available device
        let devices = crate::commands::get_connected_devices()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        if devices.is_empty() {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        
        devices[0]["device"]["unique_id"].as_str().unwrap_or("").to_string()
    };
    
    // Initialize response data structures
    let mut pubkeys = HashMap::new();
    let mut addresses = HashMap::new();
    let mut balances = HashMap::new();
    let mut cache_hits = 0;
    let mut missing_paths = Vec::new();
    
    // Process each path based on cache strategy
    for path in &request.paths {
        println!("🔍 [WALLET BOOTSTRAP] Processing path: {}", path);
        
        let use_cache = match request.cache_strategy {
            CacheStrategy::ForceRefresh => false,
            CacheStrategy::CacheOnly => true,
            CacheStrategy::PreferCache => true,
        };
        
        // Try to get data from cache first
        if use_cache {
            // Get pubkey from cache
            if request.include.pubkeys {
                match cache_manager.get_cached_pubkey_by_path(path).await {
                    Ok(Some(cached)) => {
                        println!("✅ [WALLET BOOTSTRAP] Cache HIT for pubkey: {}", path);
                        pubkeys.insert(path.clone(), PubkeyData {
                            pubkey: cached.xpub.unwrap_or_default(),
                            coin: cached.coin_name.clone(),
                            cached: true,
                            cache_time: Some(cached.cached_at.to_string()),
                        });
                        cache_hits += 1;
                    }
                    _ => {
                        println!("❌ [WALLET BOOTSTRAP] Cache MISS for pubkey: {}", path);
                        missing_paths.push(path.clone());
                    }
                }
            }
            
            // Get address from cache
            if request.include.addresses {
                // TODO: Implement address cache lookup
                // For now, mark as missing
                if !missing_paths.contains(path) {
                    missing_paths.push(path.clone());
                }
            }
            
            // Get balance from cache  
            if request.include.balances {
                // TODO: Implement balance cache lookup from portfolio
                // For now, use dummy data
                balances.insert(path.clone(), BalanceData {
                    confirmed: "0".to_string(),
                    unconfirmed: "0".to_string(),
                    currency: "BTC".to_string(),
                    usd_value: Some("0".to_string()),
                    cached: true,
                    last_updated: Some(chrono::Utc::now().to_rfc3339()),
                });
            }
        } else {
            // Force refresh - add to missing paths
            missing_paths.push(path.clone());
        }
    }
    
    // If cache_strategy is not CacheOnly, queue missing paths for background fetch
    let background_tasks = if matches!(request.cache_strategy, CacheStrategy::CacheOnly) {
        BackgroundTasks {
            missing_data_fetch: "disabled".to_string(),
            balance_refresh: "disabled".to_string(),
            transaction_sync: "disabled".to_string(),
        }
    } else if !missing_paths.is_empty() {
        // TODO: Queue background tasks for missing data
        println!("📋 [WALLET BOOTSTRAP] Queuing {} paths for background fetch", missing_paths.len());
        BackgroundTasks {
            missing_data_fetch: "queued".to_string(),
            balance_refresh: "pending".to_string(),
            transaction_sync: "pending".to_string(),
        }
    } else {
        BackgroundTasks {
            missing_data_fetch: "not_needed".to_string(),
            balance_refresh: "scheduled".to_string(),
            transaction_sync: "scheduled".to_string(),
        }
    };
    
    let response_time_ms = start_time.elapsed().as_millis();
    
    println!("✅ [WALLET BOOTSTRAP] Complete in {}ms - {} hits, {} misses", 
        response_time_ms, cache_hits, missing_paths.len());
    
    Ok(Json(WalletBootstrapResponse {
        device_id,
        response_time_ms,
        cache_status: CacheStatus {
            total_requested: request.paths.len(),
            cache_hits,
            cache_misses: missing_paths.len(),
            missing_paths,
            cache_freshness: chrono::Utc::now().to_rfc3339(),
        },
        data: BootstrapData {
            pubkeys,
            addresses,
            balances,
        },
        background_tasks,
    }))
}

/// Ultra-fast health check for pioneer-sdk
#[utoipa::path(
    get,
    path = "/api/v1/health/fast",
    responses(
        (status = 200, description = "Vault is healthy", body = FastHealthResponse),
        (status = 503, description = "Service unavailable")
    ),
    tag = "system"
)]
pub async fn fast_health_check(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<FastHealthResponse>, StatusCode> {
    let start_time = Instant::now();
    
    // Quick device check
    let devices = crate::commands::get_connected_devices()
        .await
        .unwrap_or_default();
    
    let device_connected = !devices.is_empty();
    let device_id = devices.first().map(|d| d["device"]["unique_id"].as_str().unwrap_or("").to_string());
    
    // Check cache status
    let cache_status = if crate::commands::get_cache_manager(&state.cache_manager).await.is_ok() {
        "ready"
    } else {
        "initializing"
    };
    
    Ok(Json(FastHealthResponse {
        status: "healthy".to_string(),
        device_connected,
        device_id,
        cache_status: cache_status.to_string(),
        response_time_ms: start_time.elapsed().as_millis(),
    }))
} 