//! Bitcoin message signing and verification

use anyhow::{Result, anyhow};
use crate::device_queue::DeviceQueueHandle;

/// Sign a message with a Bitcoin address
pub async fn sign_message(
    device_queue: &DeviceQueueHandle,
    path: &[u32],
    message: &str,
) -> Result<String> {
    // Create SignMessage request
    let msg = crate::messages::SignMessage {
        address_n: path.to_vec(),
        message: message.as_bytes().to_vec(),
        coin_name: Some("Bitcoin".to_string()),
        script_type: None,
    };
    
    let response = device_queue
        .send_raw(crate::messages::Message::SignMessage(msg), false)
        .await?;
    
    match response {
        crate::messages::Message::MessageSignature(sig) => {
            // Convert signature to base64
            use base64::Engine;
            let signature = sig.signature.ok_or_else(|| anyhow!("No signature in response"))?;
            Ok(base64::engine::general_purpose::STANDARD.encode(&signature))
        }
        _ => Err(anyhow!("Unexpected response type")),
    }
}

/// Verify a signed message
pub async fn verify_message(
    device_queue: &DeviceQueueHandle,
    address: &str,
    signature: &str,
    message: &str,
) -> Result<bool> {
    // Decode base64 signature
    use base64::Engine;
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature)
        .map_err(|e| anyhow!("Invalid signature encoding: {}", e))?;
    
    // Create VerifyMessage request
    let msg = crate::messages::VerifyMessage {
        address: Some(address.to_string()),
        signature: Some(sig_bytes),
        message: Some(message.as_bytes().to_vec()),
        coin_name: Some("Bitcoin".to_string()),
    };
    
    let response = device_queue
        .send_raw(crate::messages::Message::VerifyMessage(msg), false)
        .await?;
    
    match response {
        crate::messages::Message::Success(_) => Ok(true),
        crate::messages::Message::Failure(f) => {
            // Check if it's a verification failure or other error
            if f.message.as_deref() == Some("Invalid signature") {
                Ok(false)
            } else {
                Err(anyhow!("Verification failed: {:?}", f.message))
            }
        }
        _ => Err(anyhow!("Unexpected response type")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_signature_encoding() {
        use base64::Engine;
        let sig_bytes = vec![1, 2, 3, 4, 5];
        let encoded = base64::engine::general_purpose::STANDARD.encode(&sig_bytes);
        let decoded = base64::engine::general_purpose::STANDARD.decode(&encoded).unwrap();
        assert_eq!(sig_bytes, decoded);
    }
} 