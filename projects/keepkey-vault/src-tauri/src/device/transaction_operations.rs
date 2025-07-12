use keepkey_rust::device_queue::DeviceQueueHandle;
use crate::commands::{DeviceRequest, DeviceResponse};

pub async fn process_transaction_request(
    queue_handle: &DeviceQueueHandle,
    request: &DeviceRequest,
    request_id: &str,
    device_id: &str,
) -> Result<DeviceResponse, String> {
    let response = match request {
        // Bitcoin/UTXO signing
        DeviceRequest::SignTransaction { coin: _coin, inputs, outputs, version, lock_time } => {
            // Convert our internal types to keepkey-rust types
            let kk_inputs: Vec<keepkey_rust::messages::TxInputType> = inputs.iter().map(|input| {
                keepkey_rust::messages::TxInputType {
                    address_n: input.address_n_list.clone(),
                    prev_hash: hex::decode(&input.txid).unwrap_or_default(),
                    prev_index: input.vout,
                    amount: if let Ok(amount) = input.amount.parse::<u64>() {
                        Some(amount)
                    } else {
                        None
                    },
                    script_type: match input.script_type.as_str() {
                        "p2pkh" => Some(0),
                        "p2sh" => Some(1),
                        "p2wpkh" => Some(3),
                        "p2sh-p2wpkh" => Some(4),
                        _ => Some(0),
                    },
                    ..Default::default()
                }
            }).collect();
            
            let kk_outputs: Vec<keepkey_rust::messages::TxOutputType> = outputs.iter().map(|output| {
                keepkey_rust::messages::TxOutputType {
                    address: if output.is_change.unwrap_or(false) {
                        None
                    } else {
                        Some(output.address.clone())
                    },
                    address_n: if output.is_change.unwrap_or(false) {
                        output.address_n_list.clone().unwrap_or_default()
                    } else {
                        vec![]
                    },
                    amount: output.amount,
                    script_type: if output.is_change.unwrap_or(false) {
                        match output.script_type.as_deref() {
                            Some("p2pkh") => 0,
                            Some("p2sh") => 1,
                            Some("p2wpkh") => 3,
                            Some("p2sh-p2wpkh") => 4,
                            _ => 0,
                        }
                    } else {
                        0 // Default to p2pkh for regular outputs
                    },
                    ..Default::default()
                }
            }).collect();
            
            // For Bitcoin signing, we need to handle the transaction protocol manually
            // This is a placeholder - the actual signing happens in queue.rs
            DeviceResponse::SignedTransaction {
                request_id: request_id.to_string(),
                device_id: device_id.to_string(),
                signed_tx: String::new(),
                txid: None,
                success: false,
                error: Some("Bitcoin signing should be handled by queue.rs".to_string()),
            }
        },
        
        // Ethereum signing
        DeviceRequest::EthereumSignTransaction { 
            nonce, gas_price, gas_limit, to, value, data, 
            chain_id, max_fee_per_gas, max_priority_fee_per_gas, access_list 
        } => {
            let mut eth_tx = keepkey_rust::messages::EthereumSignTx::default();
            
            // Parse nonce
            if let Ok(nonce_bytes) = hex::decode(nonce.trim_start_matches("0x")) {
                eth_tx.nonce = Some(nonce_bytes);
            }
            
            // Gas price (for legacy transactions)
            if let Some(gp) = gas_price {
                if let Ok(gp_bytes) = hex::decode(gp.trim_start_matches("0x")) {
                    eth_tx.gas_price = Some(gp_bytes);
                }
            }
            
            // Gas limit
            if let Ok(gl_bytes) = hex::decode(gas_limit.trim_start_matches("0x")) {
                eth_tx.gas_limit = Some(gl_bytes);
            }
            
            // To address
            if !to.is_empty() {
                // Remove 0x prefix and convert to bytes
                let to_clean = to.trim_start_matches("0x");
                if let Ok(to_bytes) = hex::decode(to_clean) {
                    eth_tx.to = Some(to_bytes);
                }
            }
            
            // Value
            if let Ok(value_bytes) = hex::decode(value.trim_start_matches("0x")) {
                eth_tx.value = Some(value_bytes);
            }
            
            // Data
            if let Some(d) = data {
                if let Ok(data_bytes) = hex::decode(d.trim_start_matches("0x")) {
                    eth_tx.data_initial_chunk = Some(data_bytes);
                }
            }
            
            // Chain ID
            eth_tx.chain_id = Some(*chain_id);
            
            // EIP-1559 fields
            if let Some(max_fee) = max_fee_per_gas {
                if let Ok(max_fee_bytes) = hex::decode(max_fee.trim_start_matches("0x")) {
                    eth_tx.max_fee_per_gas = Some(max_fee_bytes);
                }
            }
            
            if let Some(max_priority) = max_priority_fee_per_gas {
                if let Ok(max_priority_bytes) = hex::decode(max_priority.trim_start_matches("0x")) {
                    eth_tx.max_priority_fee_per_gas = Some(max_priority_bytes);
                }
            }
            
            // Send the Ethereum sign transaction message
            match queue_handle.send_raw(eth_tx.into(), false).await {
                Ok(keepkey_rust::messages::Message::EthereumTxRequest(tx_req)) => {
                    // Handle the multi-step transaction signing protocol
                    // For now, return a placeholder response
                    DeviceResponse::EthereumSignedTransaction {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        serialized: String::new(),
                        v: 0,
                        r: String::new(),
                        s: String::new(),
                        success: false,
                        error: Some("Ethereum transaction signing protocol not fully implemented".to_string()),
                    }
                },
                Ok(keepkey_rust::messages::Message::Failure(failure)) => {
                    DeviceResponse::EthereumSignedTransaction {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        serialized: String::new(),
                        v: 0,
                        r: String::new(),
                        s: String::new(),
                        success: false,
                        error: Some(failure.message.unwrap_or_default()),
                    }
                },
                _ => DeviceResponse::EthereumSignedTransaction {
                    request_id: request_id.to_string(),
                    device_id: device_id.to_string(),
                    serialized: String::new(),
                    v: 0,
                    r: String::new(),
                    s: String::new(),
                    success: false,
                    error: Some("Unexpected response from device".to_string()),
                },
            }
        },
        
        // Ethereum message signing
        DeviceRequest::EthereumSignMessage { message, address } => {
            let eth_msg = keepkey_rust::messages::EthereumSignMessage {
                message: message.as_bytes().to_vec(),
                address_n: vec![], // TODO: We'd need to derive this from the address
            };
            
            match queue_handle.send_raw(eth_msg.into(), false).await {
                Ok(keepkey_rust::messages::Message::EthereumMessageSignature(sig)) => {
                    // The signature is already in the correct format
                    DeviceResponse::EthereumSignedMessage {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        signature: format!("0x{}", sig.signature.as_ref().map(|s| hex::encode(s)).unwrap_or_default()),
                        success: true,
                        error: None,
                    }
                },
                Ok(keepkey_rust::messages::Message::Failure(failure)) => {
                    DeviceResponse::EthereumSignedMessage {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        signature: String::new(),
                        success: false,
                        error: Some(failure.message.unwrap_or_default()),
                    }
                },
                _ => DeviceResponse::EthereumSignedMessage {
                    request_id: request_id.to_string(),
                    device_id: device_id.to_string(),
                    signature: String::new(),
                    success: false,
                    error: Some("Unexpected response from device".to_string()),
                },
            }
        },
        
        // Cosmos Amino signing
        DeviceRequest::CosmosSignAmino { sign_doc, signer_address } |
        DeviceRequest::ThorchainSignAmino { sign_doc, signer_address } |
        DeviceRequest::OsmosisSignAmino { sign_doc, signer_address } |
        DeviceRequest::MayachainSignAmino { sign_doc, signer_address } => {
            // For now, return a placeholder response
            // TODO: Implement actual Cosmos/Thorchain signing
            DeviceResponse::CosmosSignedAmino {
                request_id: request_id.to_string(),
                device_id: device_id.to_string(),
                signature: String::new(),
                serialized: String::new(),
                success: false,
                error: Some("Cosmos Amino signing not yet implemented".to_string()),
            }
        },
        
        // Binance signing
        DeviceRequest::BinanceSignTransaction { sign_doc, signer_address } => {
            // TODO: Implement Binance signing
            DeviceResponse::SignedTransaction {
                request_id: request_id.to_string(),
                device_id: device_id.to_string(),
                signed_tx: String::new(),
                txid: None,
                success: false,
                error: Some("Binance signing not yet implemented".to_string()),
            }
        },
        
        // XRP signing
        DeviceRequest::XrpSignTransaction { transaction } => {
            // TODO: Implement XRP signing
            DeviceResponse::SignedTransaction {
                request_id: request_id.to_string(),
                device_id: device_id.to_string(),
                signed_tx: String::new(),
                txid: None,
                success: false,
                error: Some("XRP signing not yet implemented".to_string()),
            }
        },
        
        _ => {
            log::error!("process_transaction_request: Unsupported transaction request type");
            DeviceResponse::Raw {
                request_id: request_id.to_string(),
                device_id: device_id.to_string(),
                response: serde_json::json!({
                    "error": "Unsupported transaction request type"
                }),
                success: false,
                error: Some("Unsupported transaction request type".to_string()),
            }
        }
    };
    
    Ok(response)
} 