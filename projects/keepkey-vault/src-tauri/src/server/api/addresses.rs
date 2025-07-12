use axum::{
    extract::{State, Json},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::server::ServerState;
use crate::commands::DeviceRequest;
use crate::commands::DeviceResponse;

// Add error response structure (same as in system.rs)
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

impl ErrorResponse {
    pub fn new(error: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: code.into(),
        }
    }
}

// ============ Common Request/Response Types ============

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddressRequest {
    #[serde(alias = "addressNList")]
    pub address_n: Vec<u32>,
    #[serde(alias = "showDisplay")]
    pub show_display: Option<bool>,
    // Accept but ignore additional KeepKey SDK fields
    #[serde(default)]
    pub curve: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddressResponse {
    pub address: String,
}

// ============ UTXO Address ============

#[derive(Debug, Deserialize, ToSchema)]
pub struct UtxoAddressRequest {
    pub address_n: Vec<u32>,
    pub coin: String,
    pub script_type: Option<String>,
    pub show_display: Option<bool>,
}

#[utoipa::path(
    post,
    path = "/addresses/utxo",
    request_body = UtxoAddressRequest,
    responses(
        (status = 200, description = "Address generated successfully", body = AddressResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Address"
)]
pub async fn utxo_get_address(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<UtxoAddressRequest>,
) -> Result<Json<AddressResponse>, Response> {
    // Convert address_n to path string
    let path = format!("m/{}", request.address_n.iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("/"));
    
    // Get first available device
    let devices = keepkey_rust::features::list_connected_devices();
    let device = devices.first()
        .ok_or_else(|| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse::new("No KeepKey device connected", "DEVICE_NOT_FOUND"))
            ).into_response()
        })?;
    
    let device_id = device.unique_id.clone();
    let request_id = uuid::Uuid::new_v4().to_string();
    
    // Create device request using the same pattern as other endpoints
    let device_request = DeviceRequest::GetAddress {
        path: path.clone(),
        coin_name: request.coin,
        script_type: request.script_type,
        show_display: request.show_display,
    };
    
    // Process through the queue using the same pattern as other endpoints
    let address = process_address_through_queue(
        state,
        device_id,
        request_id,
        device_request,
        device.clone(),
    ).await
    .map_err(|e| {
        (
            e,
            Json(ErrorResponse::new("Failed to get address from device", "ADDRESS_ERROR"))
        ).into_response()
    })?;
    
    Ok(Json(AddressResponse { address }))
}

// ============ Binance Address ============

#[utoipa::path(
    post,
    path = "/addresses/bnb",
    request_body = AddressRequest,
    responses(
        (status = 200, description = "Address generated successfully", body = AddressResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Address"
)]
pub async fn binance_get_address(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<AddressRequest>,
) -> Result<Json<AddressResponse>, Response> {
    handle_address_request(
        state,
        request.address_n,
        request.show_display,
        |path, show_display| DeviceRequest::BinanceGetAddress { path, show_display }
    ).await
}

// ============ Cosmos Address ============

#[utoipa::path(
    post,
    path = "/addresses/cosmos",
    request_body = AddressRequest,
    responses(
        (status = 200, description = "Address generated successfully", body = AddressResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Address"
)]
pub async fn cosmos_get_address(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<AddressRequest>,
) -> Result<Json<AddressResponse>, Response> {
    handle_address_request(
        state,
        request.address_n,
        request.show_display,
        |path, show_display| DeviceRequest::CosmosGetAddress { 
            path, 
            hrp: "cosmos".to_string(),
            show_display 
        }
    ).await
}

// ============ Osmosis Address ============

#[utoipa::path(
    post,
    path = "/addresses/osmosis",
    request_body = AddressRequest,
    responses(
        (status = 200, description = "Address generated successfully", body = AddressResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Address"
)]
pub async fn osmosis_get_address(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<AddressRequest>,
) -> Result<Json<AddressResponse>, Response> {
    handle_address_request(
        state,
        request.address_n,
        request.show_display,
        |path, show_display| DeviceRequest::OsmosisGetAddress { path, show_display }
    ).await
}

// ============ Ethereum Address ============

#[utoipa::path(
    post,
    path = "/addresses/eth",
    request_body = AddressRequest,
    responses(
        (status = 200, description = "Address generated successfully", body = AddressResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Address"
)]
pub async fn ethereum_get_address(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<AddressRequest>,
) -> Result<Json<AddressResponse>, Response> {
    handle_address_request(
        state,
        request.address_n,
        request.show_display,
        |path, show_display| DeviceRequest::EthereumGetAddress { path, show_display }
    ).await
}

// ============ Tendermint Address ============

#[utoipa::path(
    post,
    path = "/addresses/tendermint",
    request_body = AddressRequest,
    responses(
        (status = 200, description = "Address generated successfully", body = AddressResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Address"
)]
pub async fn tendermint_get_address(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<AddressRequest>,
) -> Result<Json<AddressResponse>, Response> {
    handle_address_request(
        state,
        request.address_n,
        request.show_display,
        |path, show_display| DeviceRequest::TendermintGetAddress { path, show_display }
    ).await
}

// ============ Mayachain Address ============

#[utoipa::path(
    post,
    path = "/addresses/mayachain",
    request_body = AddressRequest,
    responses(
        (status = 200, description = "Address generated successfully", body = AddressResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Address"
)]
pub async fn mayachain_get_address(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<AddressRequest>,
) -> Result<Json<AddressResponse>, Response> {
    handle_address_request(
        state,
        request.address_n,
        request.show_display,
        |path, show_display| DeviceRequest::MayachainGetAddress { path, show_display }
    ).await
}

// ============ XRP Address ============

#[utoipa::path(
    post,
    path = "/addresses/xrp",
    request_body = AddressRequest,
    responses(
        (status = 200, description = "Address generated successfully", body = AddressResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Address"
)]
pub async fn xrp_get_address(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<AddressRequest>,
) -> Result<Json<AddressResponse>, Response> {
    handle_address_request(
        state,
        request.address_n,
        request.show_display,
        |path, show_display| DeviceRequest::XrpGetAddress { path, show_display }
    ).await
}

// ============ Thorchain Address ============

#[derive(Debug, Deserialize, ToSchema)]
pub struct ThorchainAddressRequest {
    #[serde(alias = "addressNList")]
    pub address_n: Vec<u32>,
    #[serde(alias = "showDisplay")]
    pub show_display: Option<bool>,
    pub testnet: Option<bool>,
}

#[utoipa::path(
    post,
    path = "/addresses/thorchain",
    request_body = ThorchainAddressRequest,
    responses(
        (status = 200, description = "Address generated successfully", body = AddressResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Address"
)]
pub async fn thorchain_get_address(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<ThorchainAddressRequest>,
) -> Result<Json<AddressResponse>, Response> {
    handle_address_request(
        state,
        request.address_n,
        request.show_display,
        |path, show_display| DeviceRequest::ThorchainGetAddress { 
            path, 
            testnet: request.testnet.unwrap_or(false),
            show_display 
        }
    ).await
}

// ============ Helper Function ============

async fn handle_address_request<F>(
    state: Arc<ServerState>,
    address_n: Vec<u32>,
    show_display: Option<bool>,
    create_request: F,
) -> Result<Json<AddressResponse>, Response>
where
    F: FnOnce(String, Option<bool>) -> DeviceRequest,
{
    // Convert address_n to path string
    let path = format!("m/{}", address_n.iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("/"));
    
    // Get first available device
    let devices = keepkey_rust::features::list_connected_devices();
    let device = devices.first()
        .ok_or_else(|| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse::new("No KeepKey device connected", "DEVICE_NOT_FOUND"))
            ).into_response()
        })?;
    
    let device_id = device.unique_id.clone();
    let request_id = uuid::Uuid::new_v4().to_string();
    
    // Create device request
    let device_request = create_request(path.clone(), show_display);
    
    // Process through the queue
    let address = process_address_through_queue(
        state,
        device_id,
        request_id,
        device_request,
        device.clone(),
    ).await
    .map_err(|e| {
        (
            e,
            Json(ErrorResponse::new("Failed to get address from device", "ADDRESS_ERROR"))
        ).into_response()
    })?;
    
    Ok(Json(AddressResponse { address }))
}

async fn process_address_through_queue(
    state: Arc<ServerState>,
    device_id: String,
    request_id: String,
    device_request: DeviceRequest,
    device: keepkey_rust::friendly_usb::FriendlyUsbDevice,
) -> Result<String, StatusCode> {
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
    
    // Get cache manager
    let cache = crate::commands::get_cache_manager(&state.cache_manager).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Process the request through the cache-aware handler
    let response = match crate::device::address_operations::process_address_request_with_cache(
        &cache,
        &queue_handle,
        &device_request,
        &request_id,
        &device_id,
    ).await {
        Ok(response) => response,
        Err(error) => {
            // Log the actual error for debugging
            eprintln!("❌ Address request failed for device {}: {}", device_id, error);
            log::error!("Address request failed for device {}: {}", device_id, error);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Extract address from response
    match response {
        DeviceResponse::Address { address, success: true, .. } => Ok(address),
        DeviceResponse::BinanceAddress { address, success: true, .. } => Ok(address),
        DeviceResponse::CosmosAddress { address, success: true, .. } => Ok(address),
        DeviceResponse::OsmosisAddress { address, success: true, .. } => Ok(address),
        DeviceResponse::EthereumAddress { address, success: true, .. } => Ok(address),
        DeviceResponse::TendermintAddress { address, success: true, .. } => Ok(address),
        DeviceResponse::ThorchainAddress { address, success: true, .. } => Ok(address),
        DeviceResponse::MayachainAddress { address, success: true, .. } => Ok(address),
        DeviceResponse::XrpAddress { address, success: true, .. } => Ok(address),
        // Also handle failed responses with specific error logging
        DeviceResponse::EthereumAddress { error: Some(err), .. } => {
            eprintln!("❌ Ethereum address generation failed: {}", err);
            log::error!("Ethereum address generation failed: {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        },
        _ => {
            eprintln!("❌ Unexpected device response type or failed request");
            log::error!("Unexpected device response type or failed request");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
} 