use axum::{
    extract::{State, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::server::ServerState;
use crate::commands::{DeviceRequest, DeviceResponse, BitcoinUtxoInput, BitcoinUtxoOutput};

// ============ UTXO Transaction Signing ============

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UtxoSignTransactionRequest {
    pub coin: String,
    pub inputs: Vec<BitcoinUtxoInput>,
    pub outputs: Vec<BitcoinUtxoOutput>,
    pub version: Option<u32>,
    pub lock_time: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UtxoSignTransactionResponse {
    pub serialized: String,
    pub txid: Option<String>,
}

#[utoipa::path(
    post,
    path = "/utxo/sign-transaction",
    request_body = UtxoSignTransactionRequest,
    responses(
        (status = 200, description = "Transaction signed successfully", body = UtxoSignTransactionResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transaction"
)]
pub async fn utxo_sign_transaction(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<UtxoSignTransactionRequest>,
) -> Result<Json<UtxoSignTransactionResponse>, StatusCode> {
    let devices = keepkey_rust::features::list_connected_devices();
    let device = devices.first()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let device_id = device.unique_id.clone();
    let request_id = uuid::Uuid::new_v4().to_string();
    
    let device_request = DeviceRequest::SignTransaction {
        coin: request.coin,
        inputs: request.inputs,
        outputs: request.outputs,
        version: request.version.unwrap_or(1),
        lock_time: request.lock_time.unwrap_or(0),
    };
    
    let response = process_transaction_request(
        state,
        device_id,
        request_id,
        device_request,
        device.clone(),
    ).await?;
    
    match response {
        DeviceResponse::SignedTransaction { signed_tx, txid, .. } => {
            Ok(Json(UtxoSignTransactionResponse { 
                serialized: signed_tx,
                txid,
            }))
        },
        _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// ============ Ethereum Transaction Signing ============

#[derive(Debug, Deserialize, ToSchema)]
pub struct EthSignTransactionRequest {
    pub address_n: Vec<u32>,
    pub nonce: String,
    pub gas_price: Option<String>,
    pub gas_limit: String,
    pub to: String,
    pub value: String,
    pub data: Option<String>,
    pub chain_id: u32,
    pub max_fee_per_gas: Option<String>,
    pub max_priority_fee_per_gas: Option<String>,
    pub access_list: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EthSignTransactionResponse {
    pub v: u32,
    pub r: String,
    pub s: String,
    pub serialized: String,
}

#[utoipa::path(
    post,
    path = "/eth/signTransaction",
    request_body = EthSignTransactionRequest,
    responses(
        (status = 200, description = "Transaction signed successfully", body = EthSignTransactionResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transaction"
)]
pub async fn eth_sign_transaction(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<EthSignTransactionRequest>,
) -> Result<Json<EthSignTransactionResponse>, StatusCode> {
    let devices = keepkey_rust::features::list_connected_devices();
    let device = devices.first()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let device_id = device.unique_id.clone();
    let request_id = uuid::Uuid::new_v4().to_string();
    
    let device_request = DeviceRequest::EthereumSignTransaction {
        nonce: request.nonce,
        gas_price: request.gas_price,
        gas_limit: request.gas_limit,
        to: request.to,
        value: request.value,
        data: request.data,
        chain_id: request.chain_id,
        max_fee_per_gas: request.max_fee_per_gas,
        max_priority_fee_per_gas: request.max_priority_fee_per_gas,
        access_list: request.access_list,
    };
    
    let response = process_transaction_request(
        state,
        device_id,
        request_id,
        device_request,
        device.clone(),
    ).await?;
    
    match response {
        DeviceResponse::EthereumSignedTransaction { v, r, s, serialized, .. } => {
            Ok(Json(EthSignTransactionResponse { v, r, s, serialized }))
        },
        _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// ============ Ethereum Message Signing ============

#[derive(Debug, Deserialize, ToSchema)]
pub struct EthSignMessageRequest {
    pub address_n: Vec<u32>,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EthSignMessageResponse {
    pub address: String,
    pub signature: String,
}

#[utoipa::path(
    post,
    path = "/eth/sign",
    request_body = EthSignMessageRequest,
    responses(
        (status = 200, description = "Message signed successfully", body = EthSignMessageResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transaction"
)]
pub async fn eth_sign_message(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<EthSignMessageRequest>,
) -> Result<Json<EthSignMessageResponse>, StatusCode> {
    let devices = keepkey_rust::features::list_connected_devices();
    let device = devices.first()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let device_id = device.unique_id.clone();
    let request_id = uuid::Uuid::new_v4().to_string();
    
    // First get the address for this derivation path
    
    // Get or create device queue handle
    let queue_handle = {
        let mut manager = state.device_queue_manager.lock().await;
        
        if let Some(handle) = manager.get(&device_id) {
            handle.clone()
        } else {
            let handle = keepkey_rust::device_queue::DeviceQueueFactory::spawn_worker(device_id.clone(), device.clone());
            manager.insert(device_id.clone(), handle.clone());
            handle
        }
    };
    
    // Get the address
    let msg = keepkey_rust::messages::EthereumGetAddress {
        address_n: request.address_n.clone(),
        show_display: Some(false),
    };
    
    let address = match queue_handle.send_raw(msg.into(), false).await {
        Ok(keepkey_rust::messages::Message::EthereumAddress(addr)) => {
            format!("0x{}", hex::encode(&addr.address))
        },
        _ => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    
    let device_request = DeviceRequest::EthereumSignMessage {
        message: request.message,
        address: address.clone(),
    };
    
    let response = process_transaction_request(
        state,
        device_id,
        request_id,
        device_request,
        device.clone(),
    ).await?;
    
    match response {
        DeviceResponse::EthereumSignedMessage { signature, .. } => {
            Ok(Json(EthSignMessageResponse { address, signature }))
        },
        _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// ============ Cosmos/Amino Signing ============

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CosmosSignAminoRequest {
    pub sign_doc: serde_json::Value,
    pub signer_address: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CosmosSignAminoResponse {
    pub signed: serde_json::Value,
    pub signature: String,
    pub serialized: String,
}

#[utoipa::path(
    post,
    path = "/cosmos/sign-amino",
    request_body = CosmosSignAminoRequest,
    responses(
        (status = 200, description = "Transaction signed successfully", body = CosmosSignAminoResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Transaction"
)]
pub async fn cosmos_sign_amino(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<CosmosSignAminoRequest>,
) -> Result<Json<CosmosSignAminoResponse>, StatusCode> {
    let devices = keepkey_rust::features::list_connected_devices();
    let device = devices.first()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let device_id = device.unique_id.clone();
    let request_id = uuid::Uuid::new_v4().to_string();
    
    let device_request = DeviceRequest::CosmosSignAmino {
        sign_doc: request.sign_doc,
        signer_address: request.signer_address,
    };
    
    let response = process_transaction_request(
        state,
        device_id,
        request_id,
        device_request,
        device.clone(),
    ).await?;
    
    match response {
        DeviceResponse::CosmosSignedAmino { signature, serialized, .. } => {
            Ok(Json(CosmosSignAminoResponse { 
                signed: serde_json::Value::Object(serde_json::Map::new()), // TODO: include actual signed doc
                signature,
                serialized,
            }))
        },
        _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// ============ Helper Function ============

async fn process_transaction_request(
    state: Arc<ServerState>,
    device_id: String,
    request_id: String,
    device_request: DeviceRequest,
    device: keepkey_rust::friendly_usb::FriendlyUsbDevice,
) -> Result<DeviceResponse, StatusCode> {
    // Get or create device queue handle
    let queue_handle = {
        let mut manager = state.device_queue_manager.lock().await;
        
        if let Some(handle) = manager.get(&device_id) {
            handle.clone()
        } else {
            let handle = keepkey_rust::device_queue::DeviceQueueFactory::spawn_worker(device_id.clone(), device.clone());
            manager.insert(device_id.clone(), handle.clone());
            handle
        }
    };
    
    // Process the request through the appropriate handler
    let response = match crate::device::transaction_operations::process_transaction_request(
        &queue_handle,
        &device_request,
        &request_id,
        &device_id,
    ).await {
        Ok(response) => response,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    
    Ok(response)
} 