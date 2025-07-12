//! Cosmos ecosystem support for KeepKey
//! 
//! Provides support for Cosmos SDK based chains including:
//! - Cosmos Hub
//! - Osmosis
//! - Juno
//! - Other Cosmos SDK chains

use cosmrs::AccountId;
use anyhow::Result;

pub mod address;
pub mod transaction;
pub mod amino;

pub use address::get_cosmos_address;
pub use transaction::{sign_cosmos_transaction, CosmosTransaction};

/// Main Cosmos support structure
pub struct CosmosSupport;

impl CosmosSupport {
    /// Get a Cosmos address for the given path and prefix
    pub async fn get_address(
        device_queue: &crate::device_queue::DeviceQueueHandle,
        path: &[u32],
        hrp: &str,
    ) -> Result<AccountId> {
        address::get_cosmos_address(device_queue, path, hrp).await
    }
    
    /// Sign a Cosmos transaction
    pub async fn sign_transaction(
        device_queue: &crate::device_queue::DeviceQueueHandle,
        transaction: CosmosTransaction,
    ) -> Result<Vec<u8>> {
        transaction::sign_cosmos_transaction(device_queue, transaction).await
    }
}

/// Supported Cosmos message types
#[derive(Debug, Clone)]
pub enum CosmosMessageType {
    /// Send tokens
    Send {
        from_address: String,
        to_address: String,
        amount: Vec<Coin>,
    },
    /// Delegate to validator
    Delegate {
        delegator_address: String,
        validator_address: String,
        amount: Coin,
    },
    /// Undelegate from validator
    Undelegate {
        delegator_address: String,
        validator_address: String,
        amount: Coin,
    },
    /// IBC transfer
    IbcTransfer {
        sender: String,
        receiver: String,
        amount: Coin,
        source_channel: String,
        timeout_timestamp: u64,
    },
}

/// Cosmos coin representation
#[derive(Debug, Clone)]
pub struct Coin {
    pub denom: String,
    pub amount: String,
} 