// Pioneer API client module for fetching portfolio data
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

pub mod types;
pub mod client;

pub use types::*;
pub use client::PioneerClient;

#[cfg(test)]
mod tests;

// Re-export main types
pub use types::{
    PortfolioBalance,
    PortfolioRequest,
    PortfolioResponse,
    StakingPosition,
    AssetInfo,
    NetworkInfo,
};

/// Create a new Pioneer API client
pub fn create_client(api_key: Option<String>) -> Result<PioneerClient> {
    PioneerClient::new(api_key)
} 