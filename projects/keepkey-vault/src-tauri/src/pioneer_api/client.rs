// Pioneer API client implementation
use super::types::*;
use anyhow::{Result, anyhow};
use reqwest::{Client, StatusCode};
use serde_json::json;
use std::time::Duration;

const DEFAULT_API_URL: &str = "https://pioneers.dev";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Pioneer API client for fetching portfolio data
pub struct PioneerClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl PioneerClient {
    /// Create a new Pioneer API client
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .build()?;
        
        Ok(Self {
            client,
            base_url: DEFAULT_API_URL.to_string(),
            api_key,
        })
    }
    
    /// Create a client with custom base URL (for testing)
    pub fn with_base_url(base_url: String, api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .build()?;
        
        Ok(Self {
            client,
            base_url,
            api_key,
        })
    }
    
    /// Get portfolio balances for a list of pubkey/CAIP pairs
    /// This matches the pioneer-sdk GetPortfolioBalances method
    pub async fn get_portfolio_balances(&self, requests: Vec<PortfolioRequest>) -> Result<Vec<PortfolioBalance>> {
        log::info!("🔍 Fetching portfolio balances for {} pubkeys", requests.len());
        
        let url = format!("{}/api/v1/portfolio/balances", self.base_url);
        
        let mut request = self.client
            .post(&url)
            .json(&json!({ "requests": requests }));
        
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }
        
        let response = request.send().await?;
        
        match response.status() {
            StatusCode::OK => {
                let data: Vec<PortfolioBalance> = response.json().await?;
                log::info!("✅ Received {} balances", data.len());
                Ok(data)
            }
            StatusCode::UNAUTHORIZED => {
                Err(anyhow!("Unauthorized: Invalid or missing API key"))
            }
            StatusCode::SERVICE_UNAVAILABLE => {
                log::warn!("⚠️ Pioneer API unavailable, returning empty balances");
                Ok(vec![])
            }
            status => {
                let error_text = response.text().await.unwrap_or_default();
                Err(anyhow!("API error ({}): {}", status, error_text))
            }
        }
    }
    
    /// Get staking positions for a Cosmos address
    pub async fn get_staking_positions(&self, network_id: &str, address: &str) -> Result<Vec<StakingPosition>> {
        log::info!("🔍 Fetching staking positions for {} on {}", address, network_id);
        
        let url = format!("{}/api/v1/{}/staking/{}", self.base_url, network_id, address);
        
        let mut request = self.client.get(&url);
        
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }
        
        let response = request.send().await?;
        
        match response.status() {
            StatusCode::OK => {
                let data: Vec<StakingPosition> = response.json().await?;
                log::info!("✅ Received {} staking positions", data.len());
                Ok(data)
            }
            StatusCode::NOT_FOUND => {
                log::info!("ℹ️ No staking positions found");
                Ok(vec![])
            }
            status => {
                let error_text = response.text().await.unwrap_or_default();
                Err(anyhow!("API error ({}): {}", status, error_text))
            }
        }
    }
    
    /// Get charts data (additional portfolio info including staking)
    /// This matches the pioneer-sdk getCharts method
    pub async fn get_charts(&self, pubkeys: Vec<PubkeyInfo>) -> Result<Vec<PortfolioBalance>> {
        log::info!("🔍 Fetching charts/staking data for {} pubkeys", pubkeys.len());
        
        let url = format!("{}/api/v1/portfolio/charts", self.base_url);
        
        let mut request = self.client
            .post(&url)
            .json(&json!({ "pubkeys": pubkeys }));
        
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }
        
        let response = request.send().await?;
        
        match response.status() {
            StatusCode::OK => {
                let data: Vec<PortfolioBalance> = response.json().await?;
                log::info!("✅ Received {} chart/staking entries", data.len());
                Ok(data)
            }
            StatusCode::SERVICE_UNAVAILABLE => {
                log::warn!("⚠️ Charts API unavailable, skipping staking data");
                Ok(vec![])
            }
            status => {
                let error_text = response.text().await.unwrap_or_default();
                Err(anyhow!("API error ({}): {}", status, error_text))
            }
        }
    }
    
    /// Get fee rates for a network
    pub async fn get_fee_rate(&self, network_id: &str) -> Result<FeeRateResponse> {
        log::info!("🔍 Fetching fee rates for {}", network_id);
        
        let url = format!("{}/api/v1/{}/fees", self.base_url, network_id);
        
        let mut request = self.client.get(&url);
        
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }
        
        let response = request.send().await?;
        
        match response.status() {
            StatusCode::OK => {
                let data: FeeRateResponse = response.json().await?;
                Ok(data)
            }
            _ => {
                // Fallback to default rates
                Ok(FeeRateResponse {
                    fastest: 50,
                    fast: 20,
                    average: 10,
                })
            }
        }
    }
    
    /// Build a complete portfolio (emulating pioneer-sdk sync behavior)
    pub async fn build_portfolio(&self, xpubs: Vec<&str>) -> Result<Dashboard> {
        log::info!("🔍 Building complete portfolio for {} xpubs", xpubs.len());
        
        // Create portfolio requests for all xpubs
        let mut requests = Vec::new();
        for xpub in &xpubs {
            // In real implementation, we'd derive the CAIP from the xpub
            // For now, using placeholder CAIPs
            requests.push(PortfolioRequest {
                caip: "bip122:000000000019d6689c085ae165831e93/slip44:0".to_string(),
                pubkey: xpub.to_string(),
            });
        }
        
        // Fetch all balances
        let balances = self.get_portfolio_balances(requests).await?;
        
        // Build dashboard
        let mut total_value_usd = 0.0;
        let mut network_totals: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
        let mut asset_totals: std::collections::HashMap<String, (f64, String)> = std::collections::HashMap::new();
        
        for balance in &balances {
            let value = balance.value_usd.parse::<f64>().unwrap_or(0.0);
            total_value_usd += value;
            
            // Aggregate by network
            *network_totals.entry(balance.network_id.clone()).or_insert(0.0) += value;
            
            // Aggregate by asset
            let asset_entry = asset_totals.entry(balance.ticker.clone()).or_insert((0.0, "0".to_string()));
            asset_entry.0 += value;
            
            // Parse and add balance
            if let Ok(bal) = balance.balance.parse::<f64>() {
                let current = asset_entry.1.parse::<f64>().unwrap_or(0.0);
                asset_entry.1 = (current + bal).to_string();
            }
        }
        
        // Build network summaries
        let mut networks = Vec::new();
        for (network_id, value_usd) in network_totals {
            let percentage = (value_usd / total_value_usd) * 100.0;
            networks.push(NetworkSummary {
                network_id: network_id.clone(),
                name: Self::get_network_name(&network_id),
                value_usd,
                percentage,
            });
        }
        
        // Build asset summaries
        let mut assets = Vec::new();
        for (ticker, (value_usd, balance)) in asset_totals {
            let percentage = (value_usd / total_value_usd) * 100.0;
            assets.push(AssetSummary {
                ticker: ticker.clone(),
                name: ticker.clone(), // In real impl, would look up full name
                balance,
                value_usd,
                percentage,
            });
        }
        
        // Sort by value
        networks.sort_by(|a, b| b.value_usd.partial_cmp(&a.value_usd).unwrap());
        assets.sort_by(|a, b| b.value_usd.partial_cmp(&a.value_usd).unwrap());
        
        Ok(Dashboard {
            total_value_usd,
            networks,
            assets,
        })
    }
    
    // Helper method to get network names
    fn get_network_name(network_id: &str) -> String {
        match network_id {
            "eip155:1" => "Ethereum".to_string(),
            "bip122:000000000019d6689c085ae165831e93" => "Bitcoin".to_string(),
            "cosmos:cosmoshub-4" => "Cosmos Hub".to_string(),
            "cosmos:osmosis-1" => "Osmosis".to_string(),
            _ => network_id.to_string(),
        }
    }
} 