use axum::{
    extract::{State, Json},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::server::ServerState;
use crate::commands::{DeviceRequest, DeviceResponse};

// ============ Ping ============

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PingRequest {
    pub message: Option<String>,
    pub button_protection: Option<bool>,
    pub passphrase_protection: Option<bool>,
    pub wipe_code_protection: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    pub message: String,
}

// Add error response structure
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

#[utoipa::path(
    post,
    path = "/system/ping",
    request_body = PingRequest,
    responses(
        (status = 200, description = "Ping successful", body = PingResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "System"
)]
pub async fn system_ping(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<PingRequest>,
) -> Result<Json<PingResponse>, Response> {
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
    let device_request = DeviceRequest::Ping {
        message: request.message.clone(),
        button_protection: request.button_protection,
    };
    
    // Process through the queue with cache support
    let response = process_system_request_with_cache(
        state,
        device_id,
        request_id,
        device_request,
        device.clone(),
    ).await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(format!("Device operation failed: {}", e), "DEVICE_ERROR"))
        ).into_response()
    })?;
    
    match response {
        DeviceResponse::PingResponse { message, .. } => Ok(Json(PingResponse { message })),
        _ => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Unexpected response from device", "INVALID_RESPONSE"))
        ).into_response()),
    }
}

// ============ Get Entropy ============

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetEntropyRequest {
    pub size: u32,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetEntropyResponse {
    pub entropy: String, // hex encoded
}

#[utoipa::path(
    post,
    path = "/system/info/get-entropy",
    request_body = GetEntropyRequest,
    responses(
        (status = 200, description = "Entropy retrieved", body = GetEntropyResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "System"
)]
pub async fn get_entropy(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<GetEntropyRequest>,
) -> Result<Json<GetEntropyResponse>, Response> {
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
    
    let device_request = DeviceRequest::GetEntropy { size: request.size };
    
    let response = process_system_request(
        state,
        device_id,
        request_id,
        device_request,
        device.clone(),
    ).await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(format!("Device operation failed: {}", e), "DEVICE_ERROR"))
        ).into_response()
    })?;
    
    match response {
        DeviceResponse::Entropy { entropy, .. } => Ok(Json(GetEntropyResponse { entropy })),
        _ => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Unexpected response from device", "INVALID_RESPONSE"))
        ).into_response()),
    }
}

// ============ Get Public Key ============

#[derive(Debug, Deserialize, ToSchema)]
pub struct GetPublicKeyRequest {
    pub address_n: Vec<u32>,
    pub ecdsa_curve_name: Option<String>,
    pub show_display: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetPublicKeyResponse {
    pub xpub: String,
    pub node: serde_json::Value,
}

#[utoipa::path(
    post,
    path = "/system/info/get-public-key",
    request_body = GetPublicKeyRequest,
    responses(
        (status = 200, description = "Public key retrieved", body = GetPublicKeyResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "System"
)]
pub async fn get_public_key(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<GetPublicKeyRequest>,
) -> Result<Json<GetPublicKeyResponse>, Response> {
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
    
    let path = format!("m/{}", request.address_n.iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("/"));
    
    // Simple approach like kkcli-v2: let the system operations layer handle defaults
    // Don't try to auto-detect script types - this causes failures
    let device_request = DeviceRequest::GetPublicKey {
        path,
        coin_name: None, // Will be defaulted to Bitcoin in system operations
        script_type: None, // Will be set to None in system operations (let firmware decide)
        ecdsa_curve_name: request.ecdsa_curve_name,
        show_display: request.show_display,
    };
    
    let response = process_system_request_with_cache(
        state,
        device_id,
        request_id,
        device_request,
        device.clone(),
    ).await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(format!("Device operation failed: {}", e), "DEVICE_ERROR"))
        ).into_response()
    })?;
    
    match response {
        DeviceResponse::PublicKey { xpub, node, .. } => {
            Ok(Json(GetPublicKeyResponse { xpub, node: node.unwrap_or(serde_json::Value::Null) }))
        },
        _ => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Unexpected response from device", "INVALID_RESPONSE"))
        ).into_response()),
    }
}

// ============ Apply Settings ============

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplySettingsRequest {
    pub language: Option<String>,
    pub label: Option<String>,
    pub use_passphrase: Option<bool>,
    pub homescreen: Option<String>,
    pub auto_lock_delay_ms: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplySettingsResponse {
    pub success: bool,
}

#[utoipa::path(
    post,
    path = "/system/settings/apply",
    request_body = ApplySettingsRequest,
    responses(
        (status = 200, description = "Settings applied", body = ApplySettingsResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "System"
)]
pub async fn apply_settings(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<ApplySettingsRequest>,
) -> Result<Json<ApplySettingsResponse>, Response> {
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
    
    let device_request = DeviceRequest::ApplySettings {
        label: request.label,
        language: request.language,
        use_passphrase: request.use_passphrase,
        auto_lock_delay_ms: request.auto_lock_delay_ms,
        u2f_counter: None,
    };
    
    let response = process_system_request(
        state,
        device_id,
        request_id,
        device_request,
        device.clone(),
    ).await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(format!("Device operation failed: {}", e), "DEVICE_ERROR"))
        ).into_response()
    })?;
    
    match response {
        DeviceResponse::Success { .. } => Ok(Json(ApplySettingsResponse { success: true })),
        _ => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Unexpected response from device", "INVALID_RESPONSE"))
        ).into_response()),
    }
}

// ============ Clear Session ============

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClearSessionResponse {
    pub success: bool,
}

#[utoipa::path(
    post,
    path = "/system/clear-session",
    responses(
        (status = 200, description = "Session cleared", body = ClearSessionResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "System"
)]
pub async fn clear_session(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<ClearSessionResponse>, Response> {
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
    
    let device_request = DeviceRequest::ClearSession;
    
    let response = process_system_request(
        state,
        device_id,
        request_id,
        device_request,
        device.clone(),
    ).await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(format!("Device operation failed: {}", e), "DEVICE_ERROR"))
        ).into_response()
    })?;
    
    match response {
        DeviceResponse::Success { .. } => Ok(Json(ClearSessionResponse { success: true })),
        _ => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Unexpected response from device", "INVALID_RESPONSE"))
        ).into_response()),
    }
}

// ============ Wipe Device ============

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WipeDeviceResponse {
    pub success: bool,
}

#[utoipa::path(
    post,
    path = "/system/wipe-device",
    responses(
        (status = 200, description = "Device wiped", body = WipeDeviceResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "System"
)]
pub async fn wipe_device(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<WipeDeviceResponse>, Response> {
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
    
    let device_request = DeviceRequest::WipeDevice;
    
    let response = process_system_request(
        state,
        device_id,
        request_id,
        device_request,
        device.clone(),
    ).await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(format!("Device operation failed: {}", e), "DEVICE_ERROR"))
        ).into_response()
    })?;
    
    match response {
        DeviceResponse::Success { .. } => Ok(Json(WipeDeviceResponse { success: true })),
        _ => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Unexpected response from device", "INVALID_RESPONSE"))
        ).into_response()),
    }
}

// ============ Helper Function ============

async fn process_system_request_with_cache(
    state: Arc<ServerState>,
    device_id: String,
    request_id: String,
    device_request: DeviceRequest,
    device: keepkey_rust::friendly_usb::FriendlyUsbDevice,
) -> Result<DeviceResponse, String> {
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
        .map_err(|e| format!("Failed to get cache manager: {}", e))?;
    
    // Process the request through the cache-aware handler
    let response = match crate::device::system_operations::process_system_request_with_cache(
        &cache,
        &queue_handle,
        &device_request,
        &request_id,
        &device_id,
    ).await {
        Ok(response) => response,
        Err(e) => return Err(e),
    };
    
    Ok(response)
}

async fn process_system_request(
    state: Arc<ServerState>,
    device_id: String,
    request_id: String,
    device_request: DeviceRequest,
    device: keepkey_rust::friendly_usb::FriendlyUsbDevice,
) -> Result<DeviceResponse, String> {
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
    
    // Process the request through the appropriate handler (without cache)
    let response = match crate::device::system_operations::process_system_request(
        &queue_handle,
        &device_request,
        &request_id,
        &device_id,
    ).await {
        Ok(response) => response,
        Err(e) => return Err(e),
    };
    
    Ok(response)
} 