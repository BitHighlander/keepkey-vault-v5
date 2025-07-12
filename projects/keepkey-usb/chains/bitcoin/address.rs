//! Bitcoin address generation

use bitcoin::{Address, Network, PublicKey};
use anyhow::{Result, anyhow};
use crate::device_queue::DeviceQueueHandle;
use super::ScriptType;
use std::str::FromStr;

/// Get a Bitcoin address from the device
pub async fn get_bitcoin_address(
    device_queue: &DeviceQueueHandle,
    path: &[u32],
    script_type: ScriptType,
    network: Network,
) -> Result<Address> {
    // Create the GetAddress message
    let msg = crate::messages::GetAddress {
        address_n: path.to_vec(),
        coin_name: Some(match network {
            Network::Bitcoin => "Bitcoin".to_string(),
            Network::Testnet => "Testnet".to_string(),
            Network::Signet => "Testnet".to_string(),
            Network::Regtest => "Testnet".to_string(),
            _ => return Err(anyhow!("Unsupported network")),
        }),
        show_display: Some(false),
        multisig: None,
        script_type: Some(script_type.to_proto_output()),
    };
    
    // Send request through device queue
    let response = device_queue
        .send_raw(crate::messages::Message::GetAddress(msg), false)
        .await?;
    
    // Extract address from response
    match response {
        crate::messages::Message::Address(addr) => {
            // Parse the address string and check network
            let parsed = Address::from_str(&addr.address)
                .map_err(|e| anyhow!("Failed to parse address: {}", e))?;
            parsed.require_network(network)
                .map_err(|e| anyhow!("Address network mismatch: {}", e))
        }
        _ => Err(anyhow!("Unexpected response type")),
    }
}

/// Get extended public key for a derivation path
pub async fn get_xpub(
    device_queue: &DeviceQueueHandle,
    path: &[u32],
    script_type: ScriptType,
    network: Network,
) -> Result<String> {
    // Create the GetPublicKey message
    let msg = crate::messages::GetPublicKey {
        address_n: path.to_vec(),
        ecdsa_curve_name: Some("secp256k1".to_string()),
        show_display: Some(false),
        coin_name: Some(match network {
            Network::Bitcoin => "Bitcoin".to_string(),
            Network::Testnet => "Testnet".to_string(),
            _ => return Err(anyhow!("Unsupported network")),
        }),
        script_type: Some(script_type.to_proto_output()),
    };
    
    let response = device_queue
        .send_raw(crate::messages::Message::GetPublicKey(msg), false)
        .await?;
    
    match response {
        crate::messages::Message::PublicKey(pubkey) => {
            pubkey.xpub.ok_or_else(|| anyhow!("No xpub in response"))
        }
        _ => Err(anyhow!("Unexpected response type")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_script_type_conversion() {
        assert_eq!(ScriptType::P2PKH.to_proto_output(), 0);
        assert_eq!(ScriptType::P2WPKH.to_proto_output(), 4);
    }
} 