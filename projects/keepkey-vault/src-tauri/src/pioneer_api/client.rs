// Pioneer API client - based on https://pioneers.dev/spec/swagger.json
use super::types::*;
use anyhow::{Result, anyhow};
use reqwest::{Client, StatusCode};
use serde_json;
use std::time::Duration;

const PIONEER_API_URL: &str = "https://pioneers.dev/api/v1";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Simple Pioneer API client following the real swagger spec
pub struct PioneerClient {
    client: Client,
    api_key: String,
}

impl PioneerClient {
    /// Create a new Pioneer API client with required API key
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let api_key = api_key.ok_or_else(|| anyhow!("Pioneer API key is required"))?;
        
        let client = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .user_agent("keepkey-vault/2.0.0")
            .build()?;
        
        Ok(Self {
            client,
            api_key,
        })
    }
    
    /// Get portfolio balances using the real Pioneer API
    /// Based on working example: GET /api/v1/portfolio
    pub async fn get_portfolio_balances(&self, pubkeys: Vec<PubkeyInfo>) -> Result<Vec<PortfolioBalance>> {
        if pubkeys.is_empty() {
            return Ok(vec![]);
        }
        
        log::info!("🔍 Fetching portfolio balances for {} pubkeys from Pioneer API", pubkeys.len());
        
        let url = format!("{}/portfolio", PIONEER_API_URL);
        
        // Convert PubkeyInfo to the format expected by Pioneer API
        let assets: Vec<serde_json::Value> = pubkeys.iter().map(|pubkey_info| {
            // Map blockchain names to proper CAIP identifiers (case-insensitive)
            let caip = if let Some(blockchain) = pubkey_info.networks.get(0) {
                // Handle both plain blockchain names and full CAIP strings
                if blockchain.contains("/slip44:") {
                    // Already in CAIP format, use as-is
                    blockchain.as_str()
                } else {
                    // Map plain blockchain names to CAIP
                    match blockchain.to_lowercase().as_str() {
                        "bitcoin" => "bip122:000000000019d6689c085ae165831e93/slip44:0",
                        "ethereum" => "eip155:1/slip44:60",
                        "cosmos" => "cosmos:cosmoshub-4/slip44:118", 
                        "thorchain" => "cosmos:thorchain-mainnet-v1/slip44:931",
                        "mayachain" => "cosmos:mayachain-mainnet-v1/slip44:931",
                        "osmosis" => "cosmos:osmosis-1/slip44:118",
                        "ripple" => "ripple:1/slip44:144",
                        "doge" | "dogecoin" => "bip122:1a91e3dace36e2be3bf030a65679fe821aa1d6ef92e7c9902eb318182c355691/slip44:3",
                        "litecoin" => "bip122:12a765e31ffd4059bada1e25190f6e98c99d9714d334efa41a195a7e7e04bfe2/slip44:2",
                        "bch" | "bitcoin-cash" | "bitcoincash" => "bip122:000000000019d6689c085ae165831e93/slip44:145",
                        "dash" => "bip122:feb5034fc5ef3d0c5a9c358c0b9730c9d5d0b6c1d9878ca2b258a78c0a4cea51/slip44:5",
                        _ => {
                            log::warn!("🔧 Unknown blockchain '{}', using default CAIP", blockchain);
                            "unknown:0/slip44:0"
                        }
                    }
                }
            } else {
                log::warn!("🔧 No blockchain specified for pubkey, using default CAIP");
                "unknown:0/slip44:0"
            };
            
            serde_json::json!({
                "caip": caip,
                "pubkey": pubkey_info.pubkey
            })
        }).collect();
        
        // Send the array directly, not wrapped in an object
        log::debug!("📡 Pioneer API request payload: {}", serde_json::to_string_pretty(&assets).unwrap_or_default());
        
        let response = self.client
            .post(&url)
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&assets)  // Send array directly
            .send()
            .await?;
        
        match response.status() {
            StatusCode::OK => {
                let balances: Vec<PortfolioBalance> = response.json().await?;
                log::info!("✅ Received {} portfolio balances from Pioneer API", balances.len());
                Ok(balances)
            }
            StatusCode::UNAUTHORIZED => {
                Err(anyhow!("Pioneer API: Unauthorized - check API key"))
            }
            StatusCode::TOO_MANY_REQUESTS => {
                log::warn!("⚠️ Pioneer API rate limited, returning empty balances");
                Ok(vec![])
            }
            StatusCode::SERVICE_UNAVAILABLE => {
                log::warn!("⚠️ Pioneer API unavailable, returning empty balances");
                Ok(vec![])
            }
            status => {
                let error_text = response.text().await.unwrap_or_default();
                log::error!("❌ Pioneer API error ({}): {}", status, error_text);
                Err(anyhow!("Pioneer API error ({}): {}", status, error_text))
            }
        }
    }
    
    /// Check API health
    /// Based on swagger spec: GET /health
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", PIONEER_API_URL);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        Ok(response.status().is_success())
    }
} 