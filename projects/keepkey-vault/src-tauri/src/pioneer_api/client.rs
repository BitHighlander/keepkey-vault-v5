// Pioneer API client - based on https://pioneers.dev/spec/swagger.json
use super::types::*;
use anyhow::{Result, anyhow};
use reqwest::{Client, StatusCode};
use serde_json::json;
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
    /// Based on swagger spec: POST /portfolio/balances
    pub async fn get_portfolio_balances(&self, pubkeys: Vec<PubkeyInfo>) -> Result<Vec<PortfolioBalance>> {
        if pubkeys.is_empty() {
            return Ok(vec![]);
        }
        
        log::info!("🔍 Fetching portfolio balances for {} pubkeys from Pioneer API", pubkeys.len());
        
        let url = format!("{}/portfolio/balances", PIONEER_API_URL);
        
        // Build request payload matching swagger spec
        let payload = json!({
            "pubkeys": pubkeys
        });
        
        let response = self.client
            .post(&url)
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
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