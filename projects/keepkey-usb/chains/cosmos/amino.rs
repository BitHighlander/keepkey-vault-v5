//! Amino encoding for Cosmos transactions

use anyhow::{Result, anyhow};

/// Encode a Cosmos message in Amino format
pub fn encode_amino_message(message: &super::CosmosMessageType) -> Result<Vec<u8>> {
    // TODO: Implement Amino encoding for each message type
    // This is needed for legacy Cosmos transaction signing
    
    Err(anyhow!("Amino encoding not yet implemented"))
}
