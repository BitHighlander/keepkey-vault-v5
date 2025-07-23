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
use crate::cache::CacheManager;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedPortfolioResponse {
    // Summary across all devices
    pub summary: PortfolioSummary,
    
    // Individual device portfolios
    pub devices: HashMap<String, DevicePortfolio>,
    
    // Combined portfolio (all devices merged)
    pub combined: CombinedPortfolio,
    
    // Performance metrics
    pub performance: PerformanceMetrics,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioSummary {
    pub total_usd_value: f64,
    pub device_count: usize,
    pub last_updated: i64,
    pub cache_status: CacheStatus,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CacheStatus {
    #[serde(rename = "fresh")]
    Fresh,
    #[serde(rename = "stale")]
    Stale,
    #[serde(rename = "updating")]
    Updating,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePortfolio {
    pub device_id: String,
    pub label: String,
    pub total_usd_value: f64,
    pub last_seen: i64,
    pub assets: Vec<AssetBalance>,
    pub chains: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetBalance {
    pub symbol: String,
    pub name: String,
    pub chain: String,
    pub balance: String,
    pub usd_value: f64,
    pub price: f64,
    pub icon: String,
    pub percentage: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombinedPortfolio {
    pub assets: Vec<AssetBalance>,
    pub total_usd_value: f64,
    pub by_chain: HashMap<String, f64>,
    pub by_category: HashMap<String, f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceMetrics {
    pub load_time_ms: u64,
    pub cache_hit: bool,
    pub data_age: i64,
}

/// Get unified portfolio for all devices
pub async fn get_unified_portfolio(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<UnifiedPortfolioResponse>, StatusCode> {
    let start_time = std::time::Instant::now();
    
    // Get cache manager
    let cache_manager = match state.cache_manager.get() {
        Some(cm) => cm,
        None => {
            error!("Cache manager not initialized");
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    
    // Get all connected devices
    let devices = keepkey_rust::features::list_connected_devices();
    let keepkey_devices: Vec<_> = devices.iter().filter(|d| d.is_keepkey).collect();
    
    if keepkey_devices.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }
    
    // Collect portfolio data for all devices
    let mut device_portfolios = HashMap::new();
    let mut all_assets: Vec<AssetBalance> = Vec::new();
    let mut total_usd_value = 0.0;
    let mut latest_update = 0i64;
    
    for device in &keepkey_devices {
        // Get cached portfolio for this device
        match get_device_portfolio_data(cache_manager, &device.unique_id).await {
            Ok(portfolio) => {
                total_usd_value += portfolio.total_usd_value;
                
                // Track latest update time
                if portfolio.last_seen > latest_update {
                    latest_update = portfolio.last_seen;
                }
                
                // Collect all assets
                for asset in &portfolio.assets {
                    all_assets.push(asset.clone());
                }
                
                device_portfolios.insert(device.unique_id.clone(), portfolio);
            }
            Err(e) => {
                error!("Failed to get portfolio for device {}: {}", device.unique_id, e);
                // Continue with other devices
            }
        }
    }
    
    // Create combined portfolio
    let combined = create_combined_portfolio(&all_assets, total_usd_value);
    
    // Determine cache status
    let now = chrono::Utc::now().timestamp();
    let data_age = now - latest_update;
    let cache_status = if data_age < 300 { // 5 minutes
        CacheStatus::Fresh
    } else if data_age < 3600 { // 1 hour
        CacheStatus::Stale
    } else {
        CacheStatus::Updating
    };
    
    // Create response
    let response = UnifiedPortfolioResponse {
        summary: PortfolioSummary {
            total_usd_value,
            device_count: device_portfolios.len(),
            last_updated: latest_update,
            cache_status,
        },
        devices: device_portfolios,
        combined,
        performance: PerformanceMetrics {
            load_time_ms: start_time.elapsed().as_millis() as u64,
            cache_hit: true, // We're always using cache
            data_age,
        },
    };
    
    info!("✅ Unified portfolio loaded in {}ms", start_time.elapsed().as_millis());
    Ok(Json(response))
}

/// Get portfolio data for a specific device from cache
async fn get_device_portfolio_data(
    cache_manager: &Arc<CacheManager>,
    device_id: &str,
) -> Result<DevicePortfolio, anyhow::Error> {
    // Get cached balances
    let balances = cache_manager.get_device_portfolio(device_id).await?;
    
    // Get device label (from features if available)
    let label = get_device_label(cache_manager, device_id).await
        .unwrap_or_else(|| format!("KeepKey {}", &device_id[..8]));
    
    // Calculate total USD value
    let total_usd_value: f64 = balances
        .iter()
        .map(|b| b.value_usd.parse::<f64>().unwrap_or(0.0))
        .sum();
    
    // Get unique chains
    let mut chains: Vec<String> = balances
        .iter()
        .map(|b| b.network_id.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    chains.sort();
    
    // Convert to AssetBalance format
    let mut assets: Vec<AssetBalance> = balances
        .into_iter()
        .filter_map(|balance| {
            let usd_value = balance.value_usd.parse::<f64>().ok()?;
            let price = balance.price_usd
                .as_ref()
                .and_then(|p| p.parse::<f64>().ok())
                .unwrap_or(0.0);
            
            // Skip zero balances
            if usd_value < 0.01 {
                return None;
            }
            
            Some(AssetBalance {
                symbol: balance.ticker.clone(),
                name: balance.name.unwrap_or_else(|| balance.ticker.clone()),
                chain: network_id_to_chain_name(&balance.network_id),
                balance: balance.balance,
                usd_value,
                price,
                icon: balance.icon.unwrap_or_default(),
                percentage: 0.0, // Will be calculated later
                contract: balance.contract,
            })
        })
        .collect();
    
    // Calculate percentages
    for asset in &mut assets {
        asset.percentage = (asset.usd_value / total_usd_value) * 100.0;
    }
    
    // Sort by USD value descending
    assets.sort_by(|a, b| b.usd_value.partial_cmp(&a.usd_value).unwrap());
    
    // Get last seen timestamp from cache metadata
    let last_seen = cache_manager
        .get_cache_metadata(device_id)
        .await
        .and_then(|m| m.last_frontload)
        .unwrap_or_else(|| chrono::Utc::now().timestamp());
    
    Ok(DevicePortfolio {
        device_id: device_id.to_string(),
        label,
        total_usd_value,
        last_seen,
        assets,
        chains,
    })
}

/// Get device label from cached features
async fn get_device_label(cache_manager: &Arc<CacheManager>, device_id: &str) -> Option<String> {
    // TODO: Implement fetching device label from cached features
    // For now, return None to use default
    None
}

/// Create combined portfolio from all device assets
fn create_combined_portfolio(all_assets: &[AssetBalance], total_usd_value: f64) -> CombinedPortfolio {
    let mut combined_assets: HashMap<String, AssetBalance> = HashMap::new();
    let mut by_chain: HashMap<String, f64> = HashMap::new();
    let mut by_category: HashMap<String, f64> = HashMap::new();
    
    // Merge assets across devices
    for asset in all_assets {
        let key = format!("{}-{}", asset.symbol, asset.chain);
        
        match combined_assets.get_mut(&key) {
            Some(existing) => {
                // Add to existing asset
                let existing_balance = existing.balance.parse::<f64>().unwrap_or(0.0);
                let new_balance = asset.balance.parse::<f64>().unwrap_or(0.0);
                existing.balance = (existing_balance + new_balance).to_string();
                existing.usd_value += asset.usd_value;
            }
            None => {
                // New asset
                combined_assets.insert(key, asset.clone());
            }
        }
        
        // Aggregate by chain
        *by_chain.entry(asset.chain.clone()).or_insert(0.0) += asset.usd_value;
        
        // Aggregate by category (for now, use chain as category)
        // TODO: Implement proper categorization (DeFi, Stablecoins, Native, etc.)
        let category = get_asset_category(asset);
        *by_category.entry(category).or_insert(0.0) += asset.usd_value;
    }
    
    // Convert to sorted vector and calculate percentages
    let mut assets: Vec<AssetBalance> = combined_assets.into_values().collect();
    for asset in &mut assets {
        asset.percentage = (asset.usd_value / total_usd_value) * 100.0;
    }
    assets.sort_by(|a, b| b.usd_value.partial_cmp(&a.usd_value).unwrap());
    
    CombinedPortfolio {
        assets,
        total_usd_value,
        by_chain,
        by_category,
    }
}

/// Convert network ID to human-readable chain name
fn network_id_to_chain_name(network_id: &str) -> String {
    match network_id {
        "eip155:1" => "Ethereum",
        "eip155:137" => "Polygon",
        "eip155:43114" => "Avalanche",
        "eip155:56" => "BSC",
        "eip155:42161" => "Arbitrum",
        "eip155:10" => "Optimism",
        "bip122:000000000019d6689c085ae165831e93" => "Bitcoin",
        "bip122:12a765e31ffd4059bada1e25190f6e98" => "Litecoin",
        "bip122:1a91e3dace36e2be3bf030a65679fe82" => "Dogecoin",
        "cosmos:cosmoshub-4" => "Cosmos",
        "cosmos:osmosis-1" => "Osmosis",
        "thorchain:thorchain-mainnet-v1" => "THORChain",
        "mayachain:mayachain-mainnet-v1" => "Maya",
        _ => network_id,
    }
    .to_string()
}

/// Get asset category for pie chart grouping
fn get_asset_category(asset: &AssetBalance) -> String {
    // Stablecoins
    if ["USDT", "USDC", "DAI", "BUSD", "UST", "FRAX", "TUSD", "USDP"]
        .contains(&asset.symbol.as_str())
    {
        return "Stablecoins".to_string();
    }
    
    // Native tokens
    if ["BTC", "ETH", "BNB", "MATIC", "AVAX", "FTM", "ATOM", "OSMO", "RUNE", "MAYA"]
        .contains(&asset.symbol.as_str())
    {
        return "Native Tokens".to_string();
    }
    
    // DeFi tokens
    if ["UNI", "AAVE", "COMP", "MKR", "SNX", "YFI", "SUSHI", "CRV", "1INCH"]
        .contains(&asset.symbol.as_str())
    {
        return "DeFi".to_string();
    }
    
    // Default to chain category
    asset.chain.clone()
}