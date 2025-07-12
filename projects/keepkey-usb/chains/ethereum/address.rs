//! Ethereum address generation

use ethereum_types::Address;
use anyhow::{Result, anyhow};
use crate::device_queue::DeviceQueueHandle;

/// Get an Ethereum address from the device
pub async fn get_ethereum_address(
    device_queue: &DeviceQueueHandle,
    path: &[u32],
    display: bool,
) -> Result<Address> {
    // Create the EthereumGetAddress message
    let msg = crate::messages::EthereumGetAddress {
        address_n: path.to_vec(),
        show_display: Some(display),
    };
    
    let response = device_queue
        .send_raw(crate::messages::Message::EthereumGetAddress(msg), false)
        .await?;
    
    match response {
        crate::messages::Message::EthereumAddress(addr) => {
            // Address is returned as bytes, not string
            let address_bytes = addr.address;
            
            if address_bytes.len() != 20 {
                return Err(anyhow!("Invalid address length: {}", address_bytes.len()));
            }
            
            Ok(Address::from_slice(&address_bytes))
        }
        _ => Err(anyhow!("Unexpected response type")),
    }
}

/// Get multiple Ethereum addresses in batch
pub async fn get_ethereum_addresses(
    device_queue: &DeviceQueueHandle,
    paths: &[Vec<u32>],
) -> Result<Vec<Address>> {
    let mut addresses = Vec::new();
    
    for path in paths {
        let address = get_ethereum_address(device_queue, path, false).await?;
        addresses.push(address);
    }
    
    Ok(addresses)
} 