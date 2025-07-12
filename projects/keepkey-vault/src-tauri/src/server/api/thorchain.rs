use axum::{
    extract::{Json, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::server::ServerState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ThorchainAddressRequest {
    pub address_n: Vec<u32>,
    pub show_display: Option<bool>,
    pub testnet: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThorchainAddressResponse {
    pub address: String,
}

#[utoipa::path(
    post,
    path = "/addresses/thorchain",
    request_body = ThorchainAddressRequest,
    responses(
        (status = 200, description = "Thorchain address generated successfully", body = ThorchainAddressResponse),
        (status = 500, description = "Failed to generate address")
    ),
    tag = "addresses"
)]
pub async fn thorchain_get_address(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<ThorchainAddressRequest>,
) -> Result<Json<ThorchainAddressResponse>, StatusCode> {
    // Get first connected device
    let devices = keepkey_rust::features::list_connected_devices();
    let device = devices.first()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let device_id = device.unique_id.clone();
    let _request_id = format!("thorchain_addr_{}_{}", 
        chrono::Utc::now().timestamp_millis(),
        uuid::Uuid::new_v4().to_string().chars().take(8).collect::<String>()
    );
    
    // Convert address_n to path string
    let path = format!("m/{}", request.address_n.iter()
        .map(|&n| {
            if n & 0x80000000 != 0 {
                format!("{}'", n & 0x7FFFFFFF)
            } else {
                n.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/"));
    
    // Create device request (removed - using direct message passing instead)
    
    // Get or create device queue handle
    let queue_handle = {
        let mut manager = state.device_queue_manager.lock().await;
        
        if let Some(handle) = manager.get(&device_id) {
            handle.clone()
        } else {
            // Spawn a new device worker
            let handle = keepkey_rust::device_queue::DeviceQueueFactory::spawn_worker(device_id.clone(), device.clone());
            manager.insert(device_id.clone(), handle.clone());
            handle
        }
    };
    
    // Send the Thorchain get address request
    let path_parts = crate::commands::parse_derivation_path(&path)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let msg = keepkey_rust::messages::ThorchainGetAddress {
        address_n: path_parts,
        testnet: Some(request.testnet.unwrap_or(false)),
        show_display: request.show_display,
    };
    
    let response = queue_handle
        .send_raw(msg.into(), false)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
    match response {
        keepkey_rust::messages::Message::ThorchainAddress(addr) => {
            Ok(Json(ThorchainAddressResponse {
                address: addr.address.unwrap_or_default(),
            }))
        }
        keepkey_rust::messages::Message::Failure(failure) => {
            eprintln!("Device returned error: {}", failure.message.unwrap_or_default());
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
        _ => {
            eprintln!("Unexpected response from device for Thorchain address request");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
} 