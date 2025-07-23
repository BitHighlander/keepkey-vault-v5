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
    pub async fn get_fee_rates(&self, network_id: &str) -> Result<FeeRates> {
        log::info!("🔍 Fetching fee rates for network: {}", network_id);
        
        let url = format!("{}/api/v1/{}/fees", self.base_url, network_id);
        
        let mut request = self.client.get(&url);
        
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }
        
        let response = request.send().await?;
        
        match response.status() {
            StatusCode::OK => {
                let fees: FeeRates = response.json().await?;
                log::info!("✅ Received fee rates for {}", network_id);
                Ok(fees)
            }
            StatusCode::SERVICE_UNAVAILABLE => {
                log::warn!("⚠️ Fee rates unavailable for {}, using defaults", network_id);
                Ok(FeeRates {
                    slow: 5,
                    fast: 20,
                    average: 10,
                })
            }
            status => {
                let error_text = response.text().await.unwrap_or_default();
                Err(anyhow!("Fee rates API error ({}): {}", status, error_text))
            }
        }
    }

    // REMOVED: build_portfolio function with hardcoded CAIP data
    // This function violated the "NEVER MOCK ANYTHING" rule by using 
    // placeholder CAIPs. Real CAIP derivation should be implemented
    // based on actual xpub analysis, not hardcoded values.

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