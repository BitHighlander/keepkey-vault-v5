//! Multi-chain support for KeepKey hardware wallets
//! 
//! This module provides comprehensive support for multiple blockchain networks,
//! including transaction building, signing, and address generation.

pub mod bitcoin;
pub mod ethereum;
pub mod cosmos;
pub mod ripple;
pub mod eos;
pub mod nano;
pub mod binance;
pub mod thorchain;
pub mod osmosis;

// Re-export common types and traits
pub use bitcoin::BitcoinSupport;
pub use ethereum::EthereumSupport;
pub use cosmos::CosmosSupport;

// Common chain traits
pub trait ChainSupport {
    type Address;
    type Transaction;
    type SignedTransaction;
    type Error;
    
    /// Generate an address for the given derivation path
    fn get_address(path: &[u32]) -> Result<Self::Address, Self::Error>;
    
    /// Sign a transaction
    fn sign_transaction(
        path: &[u32],
        transaction: Self::Transaction,
    ) -> Result<Self::SignedTransaction, Self::Error>;
} 