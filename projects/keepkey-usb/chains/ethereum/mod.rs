//! Ethereum and EVM chain support for KeepKey
//! 
//! Provides comprehensive Ethereum support including:
//! - Address generation
//! - Transaction signing (Legacy, EIP-155, EIP-1559)
//! - Message signing (personal_sign, eth_sign, EIP-712)
//! - Smart contract interaction

use ethereum_types::{Address, H256, U256};
use anyhow::Result;

pub mod address;
pub mod transaction;
pub mod message;

pub use address::get_ethereum_address;
pub use transaction::{sign_ethereum_transaction, EthereumTransaction};
pub use message::{sign_message, sign_typed_data};

/// Main Ethereum support structure
pub struct EthereumSupport;

impl EthereumSupport {
    /// Get an Ethereum address for the given path
    pub async fn get_address(
        device_queue: &crate::device_queue::DeviceQueueHandle,
        path: &[u32],
        display: bool,
    ) -> Result<Address> {
        address::get_ethereum_address(device_queue, path, display).await
    }
    
    /// Sign an Ethereum transaction
    pub async fn sign_transaction(
        device_queue: &crate::device_queue::DeviceQueueHandle,
        transaction: EthereumTransaction,
    ) -> Result<Vec<u8>> {
        transaction::sign_ethereum_transaction(device_queue, transaction).await
    }
    
    /// Sign a message using personal_sign
    pub async fn sign_message(
        device_queue: &crate::device_queue::DeviceQueueHandle,
        path: &[u32],
        message: &[u8],
    ) -> Result<Vec<u8>> {
        message::sign_message(device_queue, path, message).await
    }
}

/// Supported Ethereum transaction types
#[derive(Debug, Clone)]
pub enum TransactionType {
    /// Legacy transaction (pre-EIP-155)
    Legacy,
    /// EIP-155 transaction
    Eip155 { chain_id: u64 },
    /// EIP-1559 transaction
    Eip1559 { chain_id: u64 },
} 