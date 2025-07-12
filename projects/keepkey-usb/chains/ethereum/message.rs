//! Ethereum message signing

use anyhow::{Result, anyhow};
use crate::device_queue::DeviceQueueHandle;

/// Sign a message using personal_sign
pub async fn sign_message(
    device_queue: &DeviceQueueHandle,
    path: &[u32],
    message: &[u8],
) -> Result<Vec<u8>> {
    let msg = crate::messages::EthereumSignMessage {
        address_n: path.to_vec(),
        message: message.to_vec(),
    };
    
    let response = device_queue
        .send_raw(crate::messages::Message::EthereumSignMessage(msg), false)
        .await?;
    
    match response {
        crate::messages::Message::EthereumMessageSignature(sig) => {
            // Return signature bytes
            sig.signature.ok_or_else(|| anyhow!("No signature in response"))
        }
        _ => Err(anyhow!("Unexpected response type")),
    }
}

/// Sign typed data (EIP-712)
pub async fn sign_typed_data(
    device_queue: &DeviceQueueHandle,
    path: &[u32],
    domain_hash: &[u8; 32],
    message_hash: &[u8; 32],
) -> Result<Vec<u8>> {
    // TODO: Implement EIP-712 typed data signing
    // This requires:
    // 1. EthereumSignTypedData message
    // 2. Proper domain and message hash computation
    // 3. Device support for EIP-712
    
    Err(anyhow!("EIP-712 typed data signing not yet implemented"))
}

/// Verify a signed message
pub async fn verify_message(
    address: &str,
    signature: &[u8],
    message: &[u8],
) -> Result<bool> {
    // TODO: Implement message verification
    // This can be done locally without device interaction
    
    Err(anyhow!("Message verification not yet implemented"))
} 