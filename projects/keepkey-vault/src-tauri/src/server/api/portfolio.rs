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
    let ttl_minutes = params.ttl.unwrap_or(10);
    
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
    state: &Arc<ServerState>,
    cache: &Arc<crate::cache::CacheManager>,
    device_id: &str,
) -> Result<(), anyhow::Error> {
    log::info!("🔄 Refreshing portfolio for device: {}", device_id);
    
    // Get device xpubs from cache database
    let xpubs = {
        let db = cache.db.lock().await;
        
        // Query xpubs for device
        let mut stmt = db.prepare("SELECT pubkey, caip FROM wallet_xpubs WHERE device_id = ?1")?;
        let rows = stmt.query_map(params![device_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<(String, String)>, _>>()?
    }; // Drop db lock here
    
    if xpubs.is_empty() {
        log::warn!("No xpubs found for device {}", device_id);
        return Ok(());
    }
    
    // Get API key from environment
    let api_key = std::env::var("PIONEER_API_KEY").ok();
    
    // Create Pioneer client
    let pioneer_client = crate::pioneer_api::create_client(api_key)?;
    
    // Build portfolio requests
    let mut requests = Vec::new();
    for (pubkey, caip) in &xpubs {
        requests.push(crate::pioneer_api::PortfolioRequest {
            caip: caip.clone(),
            pubkey: pubkey.clone(),
        });
    }
    
    // Fetch balances
    let balances = pioneer_client.get_portfolio_balances(requests).await?;
    
    // Save to cache
    for balance in &balances {
        cache.save_portfolio_balance(balance, device_id).await?;
    }
    
    // Build and save dashboard
    let dashboard = build_dashboard_from_balances(&balances);
    cache.update_dashboard(device_id, &dashboard).await?;
    
    // Save history snapshot
    cache.save_portfolio_snapshot(device_id, dashboard.total_value_usd).await?;
    
    log::info!("✅ Portfolio refresh complete for device {}: {} balances", device_id, balances.len());
    
    Ok(())
}

/// Build dashboard from balances
pub fn build_dashboard_from_balances(balances: &[PortfolioBalance]) -> Dashboard {
    use std::collections::HashMap;
    use crate::pioneer_api::{NetworkSummary, AssetSummary};
    
    let mut total_value_usd = 0.0;
    let mut network_totals: HashMap<String, f64> = HashMap::new();
    let mut asset_totals: HashMap<String, (f64, String)> = HashMap::new();
    
    for balance in balances {
        let value = balance.value_usd.parse::<f64>().unwrap_or(0.0);
        total_value_usd += value;
        
        *network_totals.entry(balance.network_id.clone()).or_insert(0.0) += value;
        
        let asset_entry = asset_totals.entry(balance.ticker.clone())
            .or_insert((0.0, "0".to_string()));
        asset_entry.0 += value;
        
        if let Ok(bal) = balance.balance.parse::<f64>() {
            let current = asset_entry.1.parse::<f64>().unwrap_or(0.0);
            asset_entry.1 = (current + bal).to_string();
        }
    }
    
    let mut networks = Vec::new();
    for (network_id, value_usd) in network_totals {
        let percentage = if total_value_usd > 0.0 {
            (value_usd / total_value_usd) * 100.0
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
        let percentage = if total_value_usd > 0.0 {
            (value_usd / total_value_usd) * 100.0
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
        total_value_usd,
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