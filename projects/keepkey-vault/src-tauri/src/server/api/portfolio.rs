// Portfolio API endpoints
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::{
    server::ServerState,
    pioneer_api::{PortfolioBalance, Dashboard},
};
use rusqlite::params;

/// Query parameters for portfolio endpoints
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioQuery {
    /// Force refresh from API instead of cache
    pub refresh: Option<bool>,
    /// Cache TTL in minutes (default: 10)
    pub ttl: Option<i64>,
}

/// Portfolio response wrapper
#[derive(Debug, Serialize, ToSchema)]
pub struct PortfolioResponse {
    pub success: bool,
    pub device_id: Option<String>,
    pub balances: Vec<PortfolioBalance>,
    pub dashboard: Option<Dashboard>,
    pub cached: bool,
    pub last_updated: Option<i64>,
}

/// Enhanced portfolio response with historical data
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnhancedPortfolioResponse {
    pub success: bool,
    pub device_id: Option<String>,
    pub total_value_usd: f64,
    pub last_updated: i64,
    pub change_from_previous: Option<f64>,
    pub change_24h: Option<f64>,
    pub balances: Vec<PortfolioBalance>,
    pub history: Vec<(i64, f64)>,
    pub cached: bool,
    pub refreshing: bool,
}

/// Response for aggregated portfolio across ALL paired devices
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AllDevicesPortfolioResponse {
    pub success: bool,
    /// THE NUMBER THAT MATTERS - total USD value across ALL paired devices
    pub total_value_usd: f64,
    pub paired_devices: usize,
    pub devices: Vec<DevicePortfolioSummary>,
    pub last_updated: i64,
    pub cached: bool,
}

/// Summary of portfolio for a single device
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DevicePortfolioSummary {
    pub device_id: String,
    pub label: String,
    pub short_id: String, // Last 8 chars for easy identification
    pub total_value_usd: f64,
    pub balance_count: usize,
}

/// UNIFIED PORTFOLIO ENDPOINT - The one pioneer-sdk expects for INSTANT loading!
/// This is the magic endpoint that makes portfolio loading go from 17s -> <1s
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedPortfolioResponse {
    pub success: bool,
    pub summary: PortfolioSummary,
    pub combined: CombinedPortfolio,
    pub devices: std::collections::HashMap<String, DevicePortfolio>,
    pub performance: PerformanceMetrics,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioSummary {
    pub total_usd_value: f64,
    pub device_count: usize,
    pub asset_count: usize,
    pub last_updated: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CombinedPortfolio {
    pub assets: Vec<UnifiedAsset>,
    pub by_chain: std::collections::HashMap<String, f64>,
    pub by_type: std::collections::HashMap<String, f64>,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedAsset {
    pub caip: String,
    pub symbol: String,
    pub name: String,
    pub balance: String,
    pub usd_value: f64,
    pub price: f64,
    pub chain: String,
    pub icon: Option<String>,
    pub contract: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DevicePortfolio {
    pub device_id: String,
    pub label: String,
    pub total_usd: f64,
    pub assets: Vec<UnifiedAsset>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceMetrics {
    pub load_time_ms: u128,
    pub data_age: i64, // seconds since last update
    pub cache_hit: bool,
}

/// Get combined portfolio across all devices
#[utoipa::path(
    get,
    path = "/api/portfolio",
    responses(
        (status = 200, description = "Combined portfolio data", body = PortfolioResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "portfolio"
)]
pub async fn get_combined_portfolio(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<PortfolioQuery>,
) -> Result<Json<PortfolioResponse>, StatusCode> {
    // Get cache manager
    let cache = crate::commands::get_cache_manager(&state.cache_manager).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Check if we should force refresh
    let force_refresh = params.refresh.unwrap_or(false);
    let _ttl_minutes = params.ttl.unwrap_or(10);
    
    // For combined portfolio, check if any device needs refresh
    let needs_refresh = if force_refresh {
        true
    } else {
        // Check cache staleness for all devices
        false // For now, use cached data if available
    };
    
    if !needs_refresh {
        // Try to get from cache
        match cache.get_combined_portfolio().await {
            Ok(balances) if !balances.is_empty() => {
                // Build dashboard from cached balances
                let dashboard = build_dashboard_from_balances(&balances);
                
                return Ok(Json(PortfolioResponse {
                    success: true,
                    device_id: None,
                    balances,
                    dashboard: Some(dashboard),
                    cached: true,
                    last_updated: Some(chrono::Utc::now().timestamp()),
                }));
            }
            _ => {
                // Cache miss or empty, will need to fetch
            }
        }
    }
    
    // If we get here, need to fetch fresh data
    // For now, return empty portfolio
    Ok(Json(PortfolioResponse {
        success: true,
        device_id: None,
        balances: vec![],
        dashboard: None,
        cached: false,
        last_updated: Some(chrono::Utc::now().timestamp()),
    }))
}

/// Get portfolio for a specific device
#[utoipa::path(
    get,
    path = "/api/portfolio/{device_id}",
    params(
        ("device_id" = String, Path, description = "Device ID to get portfolio for")
    ),
    responses(
        (status = 200, description = "Device portfolio data", body = PortfolioResponse),
        (status = 404, description = "Device not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "portfolio"
)]
pub async fn get_device_portfolio(
    State(state): State<Arc<ServerState>>,
    Path(device_id): Path<String>,
    Query(params): Query<PortfolioQuery>,
) -> Result<Json<PortfolioResponse>, StatusCode> {
    let cache = crate::commands::get_cache_manager(&state.cache_manager).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let force_refresh = params.refresh.unwrap_or(false);
    let ttl_minutes = params.ttl.unwrap_or(10);
    
    // Check if cache is stale
    let needs_refresh = if force_refresh {
        true
    } else {
        cache.is_portfolio_stale(&device_id, ttl_minutes).await
            .unwrap_or(true)
    };
    
    if !needs_refresh {
        // Get from cache
        match cache.get_device_portfolio(&device_id).await {
            Ok(balances) if !balances.is_empty() => {
                // Also try to get dashboard
                let dashboard = cache.get_dashboard(&device_id).await.ok().flatten();
                
                return Ok(Json(PortfolioResponse {
                    success: true,
                    device_id: Some(device_id),
                    balances,
                    dashboard,
                    cached: true,
                    last_updated: Some(chrono::Utc::now().timestamp()),
                }));
            }
            _ => {
                // Cache miss, need to fetch
            }
        }
    }
    
    // Trigger portfolio refresh in background
    let device_id_clone = device_id.clone();
    let cache_clone = cache.clone();
    let state_clone = state.clone();
    
    tokio::spawn(async move {
        if let Err(e) = refresh_device_portfolio(&state_clone, &cache_clone, &device_id_clone).await {
            log::error!("Failed to refresh portfolio for device {}: {}", device_id_clone, e);
        }
    });
    
    // Return current cached data (might be empty)
    let balances = cache.get_device_portfolio(&device_id).await
        .unwrap_or_default();
    let dashboard = cache.get_dashboard(&device_id).await.ok().flatten();
    
    let is_cached = !balances.is_empty();
    
    Ok(Json(PortfolioResponse {
        success: true,
        device_id: Some(device_id),
        balances,
        dashboard,
        cached: is_cached,
        last_updated: Some(chrono::Utc::now().timestamp()),
    }))
}

/// Get instant portfolio with historical data
#[utoipa::path(
    get,
    path = "/api/portfolio/instant/{device_id}",
    params(
        ("device_id" = String, Path, description = "Device ID to get portfolio for")
    ),
    responses(
        (status = 200, description = "Instant portfolio data with history", body = EnhancedPortfolioResponse),
        (status = 404, description = "Device not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "portfolio"
)]
pub async fn get_instant_portfolio(
    State(state): State<Arc<ServerState>>,
    Path(device_id): Path<String>,
) -> Result<Json<EnhancedPortfolioResponse>, StatusCode> {
    let cache = crate::commands::get_cache_manager(&state.cache_manager).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Get last cached value immediately
    let (total_value_usd, last_updated, change_from_previous) = 
        match cache.get_last_portfolio_value(&device_id).await {
            Ok(Some(data)) => data,
            Ok(None) => {
                // No cached value, return empty response and trigger refresh
                trigger_background_refresh(state.clone(), cache.clone(), device_id.clone());
                return Ok(Json(EnhancedPortfolioResponse {
                    success: true,
                    device_id: Some(device_id),
                    total_value_usd: 0.0,
                    last_updated: 0,
                    change_from_previous: None,
                    change_24h: None,
                    balances: vec![],
                    history: vec![],
                    cached: false,
                    refreshing: true,
                }));
            }
            Err(e) => {
                log::error!("Failed to get cached portfolio value: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
    
    // Get current balances
    let balances = cache.get_device_portfolio(&device_id).await
        .unwrap_or_default();
    
    // Get portfolio history (last 7 days)
    let week_ago = chrono::Utc::now().timestamp() - (7 * 24 * 3600);
    let history = cache.get_portfolio_history(&device_id, Some(week_ago), None).await
        .unwrap_or_default();
    
    // Check if data is stale (> 10 minutes old)
    let now = chrono::Utc::now().timestamp();
    let is_stale = (now - last_updated) > 600;
    
    // Trigger background refresh if stale
    if is_stale {
        trigger_background_refresh(state.clone(), cache.clone(), device_id.clone());
    }
    
    Ok(Json(EnhancedPortfolioResponse {
        success: true,
        device_id: Some(device_id),
        total_value_usd,
        last_updated,
        change_from_previous,
        change_24h: None, // TODO: Calculate from history
        balances,
        history,
        cached: true,
        refreshing: is_stale,
    }))
}

/// Get portfolio history for a device
#[utoipa::path(
    get,
    path = "/api/portfolio/history/{device_id}",
    params(
        ("device_id" = String, Path, description = "Device ID to get history for"),
        ("from" = Option<i64>, Query, description = "From timestamp (unix epoch)"),
        ("to" = Option<i64>, Query, description = "To timestamp (unix epoch)")
    ),
    responses(
        (status = 200, description = "Portfolio history data", body = Vec<(i64, f64)>),
        (status = 404, description = "Device not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "portfolio"
)]
pub async fn get_portfolio_history(
    State(state): State<Arc<ServerState>>,
    Path(device_id): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<(i64, f64)>>, StatusCode> {
    let cache = crate::commands::get_cache_manager(&state.cache_manager).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Parse query params
    let from_timestamp = params.get("from")
        .and_then(|v| v.parse::<i64>().ok());
    let to_timestamp = params.get("to")
        .and_then(|v| v.parse::<i64>().ok());
    
    // Get history
    let history = cache.get_portfolio_history(&device_id, from_timestamp, to_timestamp)
        .await
        .unwrap_or_default();
    
    Ok(Json(history))
}

/// Get aggregated portfolio for ALL paired devices - THE NUMBER THAT MATTERS!
/// No device ID needed - returns total USD value across all paired devices
#[utoipa::path(
    get,
    path = "/api/portfolio",
    responses(
        (status = 200, description = "Portfolio data aggregated across all paired devices", body = AllDevicesPortfolioResponse),
        (status = 500, description = "Internal server error"),
    ),
    tag = "portfolio"
)]
pub async fn get_all_devices_portfolio(
    State(state): State<Arc<ServerState>>,
    Query(_params): Query<PortfolioQuery>,
) -> Result<Json<AllDevicesPortfolioResponse>, StatusCode> {
    let cache = state.cache_manager.get()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all device metadata to know which devices are paired
    let all_metadata = cache.get_all_device_metadata().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if all_metadata.is_empty() {
        return Ok(Json(AllDevicesPortfolioResponse {
            success: true,
            total_value_usd: 0.0,
            paired_devices: 0,
            devices: vec![],
            last_updated: chrono::Utc::now().timestamp(),
            cached: true,
        }));
    }

    // Get portfolio data for each device and aggregate
    let mut total_value_usd = 0.0;
    let mut devices = Vec::new();
    let mut latest_update = 0i64;

    for metadata in &all_metadata {
        match cache.get_device_portfolio(&metadata.device_id).await {
            Ok(balances) => {
                // Debug logging to track duplicates
                log::info!("🔍 Processing device {} ({} balances)", 
                    &metadata.device_id[metadata.device_id.len().saturating_sub(8)..], 
                    balances.len()
                );
                
                let mut device_total = 0.0;
                let mut balance_details = Vec::new();
                
                for balance in &balances {
                    if let Ok(value) = balance.value_usd.parse::<f64>() {
                        if value > 0.0 {
                            device_total += value;
                            balance_details.push((
                                balance.ticker.clone().unwrap_or_else(|| "UNKNOWN".to_string()),
                                balance.balance.clone(),
                                value,
                                balance.caip.clone(),
                            ));
                        }
                    }
                }
                
                // Log top balances for this device
                balance_details.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
                log::info!("  💰 Device total: ${:.2} USD", device_total);
                for (i, (ticker, balance, value, caip)) in balance_details.iter().take(5).enumerate() {
                    log::info!("    {}. {}: {} = ${:.2} USD ({})", i+1, ticker, balance, value, caip);
                }
                if balance_details.len() > 5 {
                    log::info!("    ... and {} more balances", balance_details.len() - 5);
                }
                
                total_value_usd += device_total;
                
                devices.push(DevicePortfolioSummary {
                    device_id: metadata.device_id.clone(),
                    label: metadata.label.clone().unwrap_or_else(|| "Unknown Device".to_string()),
                    short_id: metadata.device_id.chars().rev().take(8).collect::<String>().chars().rev().collect(),
                    total_value_usd: device_total,
                    balance_count: balances.len(),
                });

                // Track latest update time
                if let Ok(Some((_, timestamp, _))) = cache.get_last_portfolio_value(&metadata.device_id).await {
                    latest_update = latest_update.max(timestamp);
                }
            }
            Err(e) => {
                log::warn!("Failed to get portfolio for device {}: {}", metadata.device_id, e);
            }
        }
    }

    log::info!("💰 TOTAL PORTFOLIO VALUE: ${:.2} USD across {} paired devices", total_value_usd, devices.len());

    Ok(Json(AllDevicesPortfolioResponse {
        success: true,
        total_value_usd,
        paired_devices: devices.len(),
        devices,
        last_updated: if latest_update > 0 { latest_update } else { chrono::Utc::now().timestamp() },
        cached: true,
    }))
}

/// THE MAGIC ENDPOINT - Unified portfolio for instant loading
/// This is what pioneer-sdk calls when it detects kkapi:// vault
#[utoipa::path(
    get,
    path = "/api/v1/portfolio/all",
    responses(
        (status = 200, description = "Unified portfolio data for instant loading", body = UnifiedPortfolioResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "portfolio"
)]
pub async fn get_unified_portfolio(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<UnifiedPortfolioResponse>, StatusCode> {
    let start_time = std::time::Instant::now();
    log::info!("🚀 [UNIFIED PORTFOLIO] Fast load request received");
    
    let cache = state.cache_manager.get()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all device metadata
    let all_metadata = cache.get_all_device_metadata().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if all_metadata.is_empty() {
        log::warn!("🚀 [UNIFIED PORTFOLIO] No devices found");
        return Ok(Json(UnifiedPortfolioResponse {
            success: true,
            summary: PortfolioSummary {
                total_usd_value: 0.0,
                device_count: 0,
                asset_count: 0,
                last_updated: chrono::Utc::now().timestamp(),
            },
            combined: CombinedPortfolio {
                assets: vec![],
                by_chain: std::collections::HashMap::new(),
                by_type: std::collections::HashMap::new(),
            },
            devices: std::collections::HashMap::new(),
            performance: PerformanceMetrics {
                load_time_ms: start_time.elapsed().as_millis(),
                data_age: 0,
                cache_hit: true,
            },
        }));
    }

    // Aggregate all portfolio data
    let mut total_usd_value = 0.0;
    let mut all_assets = Vec::new();
    let mut devices = std::collections::HashMap::new();
    let mut by_chain = std::collections::HashMap::new();
    let mut latest_update = 0i64;

    for metadata in &all_metadata {
        match cache.get_device_portfolio(&metadata.device_id).await {
            Ok(balances) => {
                let mut device_assets = Vec::new();
                let mut device_total = 0.0;

                for balance in balances {
                    let usd_value = balance.value_usd.parse::<f64>().unwrap_or(0.0);
                    device_total += usd_value;

                    // Extract chain from CAIP
                    let chain = extract_chain_from_caip(&balance.caip);
                    *by_chain.entry(chain.clone()).or_insert(0.0) += usd_value;

                    let asset = UnifiedAsset {
                        caip: balance.caip.clone(),
                        symbol: balance.ticker.clone().unwrap_or_else(|| "UNKNOWN".to_string()),
                        name: balance.name.clone().unwrap_or_else(|| balance.ticker.clone().unwrap_or_else(|| "Unknown Asset".to_string())),
                        balance: balance.balance.clone(),
                        usd_value,
                        price: balance.price_usd.parse::<f64>().unwrap_or(0.0),
                        chain,
                        icon: None, // TODO: Get from assets map
                        contract: balance.contract.clone(),
                    };

                    device_assets.push(asset.clone());
                    all_assets.push(asset);
                }

                total_usd_value += device_total;

                devices.insert(metadata.device_id.clone(), DevicePortfolio {
                    device_id: metadata.device_id.clone(),
                    label: metadata.label.clone().unwrap_or_else(|| "Unknown Device".to_string()),
                    total_usd: device_total,
                    assets: device_assets,
                });

                // Track latest update
                if let Ok(Some((_, timestamp, _))) = cache.get_last_portfolio_value(&metadata.device_id).await {
                    latest_update = latest_update.max(timestamp);
                }
            }
            Err(e) => {
                log::warn!("Failed to get portfolio for device {}: {}", metadata.device_id, e);
            }
        }
    }

    let load_time = start_time.elapsed().as_millis();
    let data_age = if latest_update > 0 {
        chrono::Utc::now().timestamp() - latest_update
    } else {
        0
    };

    log::info!("🚀 [UNIFIED PORTFOLIO] Loaded ${:.2} USD across {} devices in {}ms", 
        total_usd_value, devices.len(), load_time);

    Ok(Json(UnifiedPortfolioResponse {
        success: true,
        summary: PortfolioSummary {
            total_usd_value,
            device_count: devices.len(),
            asset_count: all_assets.len(),
            last_updated: latest_update,
        },
        combined: CombinedPortfolio {
            assets: all_assets,
            by_chain,
            by_type: std::collections::HashMap::new(), // TODO: Categorize by asset type
        },
        devices,
        performance: PerformanceMetrics {
            load_time_ms: load_time,
            data_age,
            cache_hit: true,
        },
    }))
}

// Helper function to extract chain from CAIP
fn extract_chain_from_caip(caip: &str) -> String {
    if let Some(colon_pos) = caip.find(':') {
        if let Some(slash_pos) = caip.find('/') {
            // Format: "namespace:reference/asset_namespace:asset_reference"
            caip[..slash_pos].to_string()
        } else {
            // Format: "namespace:reference"
            caip[..colon_pos].to_string()
        }
    } else {
        caip.to_string()
    }
}

fn trigger_background_refresh(
    state: Arc<ServerState>,
    cache: Arc<crate::cache::CacheManager>,
    device_id: String,
) {
    tokio::spawn(async move {
        if let Err(e) = refresh_device_portfolio(&state, &cache, &device_id).await {
            log::error!("Background portfolio refresh failed: {}", e);
        }
    });
}

/// Refresh portfolio for a device
async fn refresh_device_portfolio(
    _state: &Arc<ServerState>,
    cache: &Arc<crate::cache::CacheManager>,
    device_id: &str,
) -> Result<(), anyhow::Error> {
    log::info!("🔄 Refreshing portfolio for device: {}", device_id);
    
    // Get device pubkey data from cached_pubkeys table (NOT wallet_xpubs!)
    let pubkey_data = {
        let db = cache.db.lock().await;
        
        // Query both xpubs and addresses from cached_pubkeys
        let mut stmt = db.prepare("
            SELECT DISTINCT 
                coin_name,
                xpub,
                address,
                script_type
            FROM cached_pubkeys 
            WHERE device_id = ?1 
            AND (xpub IS NOT NULL OR address IS NOT NULL)
        ")?;
        
        let rows = stmt.query_map(params![device_id], |row| {
            Ok((
                row.get::<_, String>(0)?,  // coin_name
                row.get::<_, Option<String>>(1)?,  // xpub
                row.get::<_, Option<String>>(2)?,  // address
                row.get::<_, Option<String>>(3)?,  // script_type
            ))
        })?;
        rows.collect::<Result<Vec<(String, Option<String>, Option<String>, Option<String>)>, _>>()?
    }; // Drop db lock here
    
    if pubkey_data.is_empty() {
        log::warn!("No cached pubkeys found for device {}", device_id);
        return Ok(());
    }
    
    log::info!("📊 Found {} cached pubkey entries for portfolio refresh", pubkey_data.len());
    
    // Get API key from environment
    let api_key = std::env::var("PIONEER_API_KEY").ok();
    
    // Create Pioneer client
    let pioneer_client = crate::pioneer_api::create_client(api_key)?;
    
    // Build pubkey info for Pioneer API - use addresses for Cosmos chains, xpubs for others
    let mut pubkey_infos = Vec::new();
    for (coin_name, xpub, address, _script_type) in &pubkey_data {
        let (pubkey, caip) = match coin_name.to_lowercase().as_str() {
            // Cosmos chains need addresses, not xpubs
            "cosmos" => {
                if let Some(addr) = address {
                    (addr.clone(), "cosmos:cosmoshub-4/slip44:118".to_string())
                } else {
                    log::warn!("⚠️ No address found for cosmos pubkey, skipping");
                    continue;
                }
            },
            "osmosis" => {
                if let Some(addr) = address {
                    (addr.clone(), "cosmos:osmosis-1/slip44:118".to_string())
                } else {
                    log::warn!("⚠️ No address found for osmosis pubkey, skipping");
                    continue;
                }
            },
            "thorchain" => {
                if let Some(addr) = address {
                    (addr.clone(), "cosmos:thorchain-mainnet-v1/slip44:931".to_string())
                } else {
                    log::warn!("⚠️ No address found for thorchain pubkey, skipping");
                    continue;
                }
            },
            "mayachain" => {
                if let Some(addr) = address {
                    (addr.clone(), "cosmos:mayachain-mainnet-v1/slip44:931".to_string())
                } else {
                    log::warn!("⚠️ No address found for mayachain pubkey, skipping");
                    continue;
                }
            },
            // Bitcoin-like chains use xpubs
            "bitcoin" => {
                if let Some(xpub_val) = xpub {
                    (xpub_val.clone(), "bip122:000000000019d6689c085ae165831e93/slip44:0".to_string())
                } else {
                    log::warn!("⚠️ No xpub found for bitcoin pubkey, skipping");
                    continue;
                }
            },
            "ethereum" => {
                if let Some(xpub_val) = xpub {
                    (xpub_val.clone(), "eip155:1/slip44:60".to_string())
                } else {
                    log::warn!("⚠️ No xpub found for ethereum pubkey, skipping");
                    continue;
                }
            },
            "litecoin" => {
                if let Some(xpub_val) = xpub {
                    (xpub_val.clone(), "bip122:12a765e31ffd4059bada1e25190f6e98/slip44:2".to_string())
                } else {
                    log::warn!("⚠️ No xpub found for litecoin pubkey, skipping");
                    continue;
                }
            },
            "dogecoin" => {
                if let Some(xpub_val) = xpub {
                    (xpub_val.clone(), "bip122:1a91e3dace36e2be3bf030a65679fe82/slip44:3".to_string())
                } else {
                    log::warn!("⚠️ No xpub found for dogecoin pubkey, skipping");
                    continue;
                }
            },
            "bitcoincash" => {
                if let Some(xpub_val) = xpub {
                    (xpub_val.clone(), "bip122:000000000000000000651ef99cb9fcbe/slip44:145".to_string())
                } else {
                    log::warn!("⚠️ No xpub found for bitcoincash pubkey, skipping");
                    continue;
                }
            },
            "dash" => {
                if let Some(xpub_val) = xpub {
                    (xpub_val.clone(), "bip122:0000ffd590b1485b3caadc19b22e637/slip44:5".to_string())
                } else {
                    log::warn!("⚠️ No xpub found for dash pubkey, skipping");
                    continue;
                }
            },
            "ripple" => {
                if let Some(addr) = address {
                    (addr.clone(), "ripple:1/slip44:144".to_string())
                } else {
                    log::warn!("⚠️ No address found for ripple pubkey, skipping");
                    continue;
                }
            },
            _ => {
                log::warn!("⚠️ Unknown coin type: {}, skipping", coin_name);
                continue;
            }
        };
        
        log::info!("📊 Adding to Pioneer API request: {} -> {} ({})", coin_name, caip, 
            if coin_name.starts_with("cosmos") || coin_name.ends_with("chain") { "address" } else { "xpub" });
        
        pubkey_infos.push(crate::pioneer_api::PubkeyInfo {
            pubkey,
            networks: vec![caip],
            path: None,
            address: None,
        });
    }
    
    if pubkey_infos.is_empty() {
        log::warn!("No valid pubkey data found for Pioneer API request");
        return Ok(());
    }
    
    log::info!("🚀 Sending {} pubkey entries to Pioneer API for portfolio fetch", pubkey_infos.len());
    
    // Fetch balances using simplified Pioneer API
    let balances = pioneer_client.get_portfolio_balances(pubkey_infos).await?;
    
    log::info!("✅ Received {} balances from Pioneer API", balances.len());
    
    // Save to cache
    for balance in &balances {
        cache.save_portfolio_balance(balance, device_id).await?;
    }
    
    // Build and save dashboard
    let dashboard = build_dashboard_from_balances(&balances);
    cache.update_dashboard(device_id, &dashboard).await?;
    
    // Save history snapshot
    cache.save_portfolio_snapshot(device_id, dashboard.total_value_usd).await?;
    
    log::info!("✅ Portfolio refresh complete for device {}: {} balances, ${:.2} total value", 
        device_id, balances.len(), dashboard.total_value_usd);
    
    Ok(())
}

/// Build dashboard from balances
pub fn build_dashboard_from_balances(balances: &[PortfolioBalance]) -> Dashboard {
    use std::collections::HashMap;
    use crate::pioneer_api::{NetworkSummary, AssetSummary};
    
    let mut total_value = 0.0;
    let mut network_totals: HashMap<String, f64> = HashMap::new();
    let mut asset_totals: HashMap<String, (f64, String)> = HashMap::new();
    
    // Calculate total value and aggregate by network and asset
    for balance in balances {
        if let Ok(value) = balance.value_usd.parse::<f64>() {
            total_value += value;
            
            // Aggregate by network if network_id exists
            if let Some(network_id) = &balance.network_id {
                *network_totals.entry(network_id.clone()).or_insert(0.0) += value;
            }
            
            // Aggregate by asset if ticker exists
            if let Some(ticker) = &balance.ticker {
                let asset_entry = asset_totals.entry(ticker.clone())
                    .or_insert((0.0, balance.name.clone().unwrap_or_else(|| ticker.clone())));
                asset_entry.0 += value;
            }
        }
    }
    
    let mut networks = Vec::new();
    for (network_id, value_usd) in network_totals {
        let percentage = if total_value > 0.0 {
            (value_usd / total_value) * 100.0
        } else {
            0.0
        };
        networks.push(NetworkSummary {
            network_id: network_id.clone(),
            name: get_network_name(&network_id),
            value_usd,
            percentage,
        });
    }
    
    let mut assets = Vec::new();
    for (ticker, (value_usd, balance)) in asset_totals {
        let percentage = if total_value > 0.0 {
            (value_usd / total_value) * 100.0
        } else {
            0.0
        };
        assets.push(AssetSummary {
            ticker: ticker.clone(),
            name: ticker.clone(),
            balance,
            value_usd,
            percentage,
        });
    }
    
    networks.sort_by(|a, b| b.value_usd.partial_cmp(&a.value_usd).unwrap());
    assets.sort_by(|a, b| b.value_usd.partial_cmp(&a.value_usd).unwrap());
    
    Dashboard {
        total_value_usd: total_value,
        networks,
        assets,
    }
}

fn get_network_name(network_id: &str) -> String {
    match network_id {
        "eip155:1" => "Ethereum".to_string(),
        "bip122:000000000019d6689c085ae165831e93" => "Bitcoin".to_string(),
        "cosmos:cosmoshub-4" => "Cosmos Hub".to_string(),
        "cosmos:osmosis-1" => "Osmosis".to_string(),
        _ => network_id.to_string(),
    }
} 