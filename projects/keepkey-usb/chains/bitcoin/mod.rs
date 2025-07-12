//! Bitcoin and UTXO chain support for KeepKey
//! 
//! Provides comprehensive Bitcoin support including:
//! - Multiple address types (P2PKH, P2SH, P2WPKH, P2WSH)
//! - Transaction building and signing
//! - Message signing and verification

use bitcoin::{Address, Network, Transaction};
use anyhow::Result;

pub mod address;
pub mod transaction;
pub mod message;

pub use address::get_bitcoin_address;
pub use transaction::{sign_bitcoin_transaction, BitcoinTxInput, BitcoinTxOutput};
pub use message::{sign_message, verify_message};

/// Main Bitcoin support structure
pub struct BitcoinSupport;

impl BitcoinSupport {
    /// Get a Bitcoin address for the given path and script type
    pub async fn get_address(
        device_queue: &crate::device_queue::DeviceQueueHandle,
        path: &[u32],
        script_type: ScriptType,
        network: Network,
    ) -> Result<Address> {
        address::get_bitcoin_address(device_queue, path, script_type, network).await
    }
    
    /// Sign a Bitcoin transaction
    pub async fn sign_transaction(
        device_queue: &crate::device_queue::DeviceQueueHandle,
        inputs: Vec<BitcoinTxInput>,
        outputs: Vec<BitcoinTxOutput>,
        network: Network,
    ) -> Result<Transaction> {
        transaction::sign_bitcoin_transaction(device_queue, inputs, outputs, network).await
    }
    
    /// Sign a message with a Bitcoin address
    pub async fn sign_message(
        device_queue: &crate::device_queue::DeviceQueueHandle,
        path: &[u32],
        message: &str,
    ) -> Result<String> {
        message::sign_message(device_queue, path, message).await
    }
}

/// Bitcoin script types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptType {
    /// Pay to Public Key Hash (Legacy)
    P2PKH,
    /// Pay to Script Hash (Legacy)
    P2SH,
    /// Pay to Witness Public Key Hash (SegWit)
    P2WPKH,
    /// Pay to Witness Script Hash (SegWit)
    P2WSH,
    /// Pay to Taproot
    P2TR,
}

impl ScriptType {
    /// Convert to protobuf input script type
    pub fn to_proto_input(&self) -> i32 {
        match self {
            ScriptType::P2PKH => 0,  // SPENDADDRESS
            ScriptType::P2SH => 1,   // SPENDMULTISIG
            ScriptType::P2WPKH => 4, // SPENDWITNESS
            ScriptType::P2WSH => 4,  // SPENDWITNESS
            ScriptType::P2TR => 5,   // SPENDTAPROOT
        }
    }
    
    /// Convert to protobuf output script type
    pub fn to_proto_output(&self) -> i32 {
        match self {
            ScriptType::P2PKH => 0,  // PAYTOADDRESS
            ScriptType::P2SH => 1,   // PAYTOSCRIPTHASH
            ScriptType::P2WPKH => 4, // PAYTOWITNESS
            ScriptType::P2WSH => 4,  // PAYTOWITNESS
            ScriptType::P2TR => 5,   // PAYTOTAPROOT
        }
    }
} 