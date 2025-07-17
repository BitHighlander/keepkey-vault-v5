// Pioneer API types matching the TypeScript pioneer-sdk structures
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

/// Portfolio balance entry matching pioneer-sdk structure
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioBalance {
    pub caip: String,                    // e.g., "eip155:1/slip44:60"
    pub ticker: String,                  // e.g., "ETH"
    pub balance: String,                 // Balance as string for precision
    pub value_usd: String,               // USD value
    pub price_usd: Option<String>,       // Price per unit
    pub network_id: String,              // e.g., "eip155:1"
    pub address: Option<String>,         // Specific address if applicable
    
    #[serde(rename = "type")]
    pub balance_type: Option<String>,    // 'balance', 'staking', 'delegation', 'reward', 'unbonding'
    
    // Asset metadata
    pub name: Option<String>,            // Full asset name
    pub icon: Option<String>,            // Icon URL
    pub precision: Option<i32>,          // Decimal places
    pub contract: Option<String>,        // Contract address for tokens
    
    // Staking specific
    pub validator: Option<String>,       // Validator address
    pub unbonding_end: Option<i64>,      // Timestamp
    pub rewards_available: Option<String>,
}

/// Staking position (for Cosmos chains)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StakingPosition {
    pub validator: String,
    pub amount: String,
    pub rewards: String,
    pub unbonding_amount: Option<String>,
    pub unbonding_end: Option<i64>,
}

/// Request structure for GetPortfolioBalances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioRequest {
    pub caip: String,
    pub pubkey: String,
}

/// Response from GetPortfolioBalances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioResponse {
    pub balances: Vec<PortfolioBalance>,
    pub total_value_usd: String,
}

/// Dashboard structure matching pioneer-sdk
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub total_value_usd: f64,
    pub networks: Vec<NetworkSummary>,
    pub assets: Vec<AssetSummary>,
}

/// Network summary for dashboard
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSummary {
    pub network_id: String,
    pub name: String,
    pub value_usd: f64,
    pub percentage: f64,
}

/// Asset summary for dashboard
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetSummary {
    pub ticker: String,
    pub name: String,
    pub balance: String,
    pub value_usd: f64,
    pub percentage: f64,
}

/// Asset metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetInfo {
    pub caip: String,
    pub ticker: String,
    pub name: String,
    pub icon: Option<String>,
    pub network_id: String,
    pub contract: Option<String>,
    pub decimals: Option<i32>,
    pub coin_gecko_id: Option<String>,
    pub is_native: bool,
}

/// Network metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInfo {
    pub network_id: String,
    pub name: String,
    pub chain_id: Option<i32>,
    pub native_asset_caip: Option<String>,
    pub explorer_url: Option<String>,
    pub rpc_url: Option<String>,
    pub is_testnet: bool,
}

/// Pubkey info structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubkeyInfo {
    pub pubkey: String,
    pub networks: Vec<String>,
    pub path: Option<String>,
    pub address: Option<String>,
}

/// Fee rate response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeRateResponse {
    pub fastest: u32,
    pub fast: u32,
    pub average: u32,
}

/// Chart data for portfolio history
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartData {
    pub timestamp: i64,
    pub value_usd: f64,
}

/// Error response from Pioneer API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub message: String,
    pub error: Option<ErrorDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub name: String,
    #[serde(rename = "statusCode")]
    pub status_code: Option<u16>,
} 