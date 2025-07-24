use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, error};

use crate::server::ServerState;

/// UNIFIED PORTFOLIO ENDPOINT - The one pioneer-sdk expects for INSTANT loading!
/// This is the magic endpoint that makes portfolio loading go from 17s -> <1s
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedPortfolioResponse {
    pub success: bool,
    pub summary: PortfolioSummary,
    pub combined: CombinedPortfolio,
    pub devices: HashMap<String, DevicePortfolio>,
    pub performance: PerformanceMetrics,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioSummary {
    pub total_usd_value: f64,
    pub device_count: usize,
    pub asset_count: usize,
    pub last_updated: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CombinedPortfolio {
    pub assets: Vec<UnifiedAsset>,
    pub by_chain: HashMap<String, f64>,
    pub by_type: HashMap<String, f64>,
}

#[derive(Debug, Serialize, Clone)]
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePortfolio {
    pub device_id: String,
    pub label: String,
    pub total_usd: f64,
    pub assets: Vec<UnifiedAsset>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceMetrics {
    pub load_time_ms: u128,
    pub data_age: i64, // seconds since last update
    pub cache_hit: bool,
}

/// THE MAGIC ENDPOINT - Unified portfolio for instant loading
/// This is what pioneer-sdk calls when it detects kkapi:// vault
pub async fn get_unified_portfolio(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<UnifiedPortfolioResponse>, StatusCode> {
    let start_time = std::time::Instant::now();
    info!("🚀 [UNIFIED PORTFOLIO] Fast load request received");
    
    let cache = state.cache_manager.get()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all device metadata
    let all_metadata = cache.get_all_device_metadata().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if all_metadata.is_empty() {
        info!("🚀 [UNIFIED PORTFOLIO] No devices found");
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
                by_chain: HashMap::new(),
                by_type: HashMap::new(),
            },
            devices: HashMap::new(),
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
    let mut devices = HashMap::new();
    let mut by_chain = HashMap::new();
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
                error!("Failed to get portfolio for device {}: {}", metadata.device_id, e);
            }
        }
    }

    let load_time = start_time.elapsed().as_millis();
    let data_age = if latest_update > 0 {
        chrono::Utc::now().timestamp() - latest_update
    } else {
        0
    };

    info!("🚀 [UNIFIED PORTFOLIO] Loaded ${:.2} USD across {} devices in {}ms", 
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
            by_type: HashMap::new(), // TODO: Categorize by asset type
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