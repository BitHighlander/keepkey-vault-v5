use std::sync::Mutex;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use tracing::info;

/// Global context for the currently selected device
static DEVICE_CONTEXT: Lazy<Mutex<Option<DeviceContext>>> = Lazy::new(|| Mutex::new(None));

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceContext {
    /// The device ID of the currently selected device
    pub device_id: String,
    /// The Bitcoin address - helps track wallet state
    pub btc_address: Option<String>,
    /// Optional label for the device
    pub label: Option<String>,
    /// Timestamp when this context was set
    pub set_at: u64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextResponse {
    /// The current device context, if any
    pub context: Option<DeviceContext>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetContextRequest {
    /// The device ID to set as current context
    pub device_id: String,
    /// The Bitcoin address (optional)
    pub btc_address: Option<String>,
    /// Optional label for the device
    pub label: Option<String>,
}

/// Get the current device context
pub async fn get_context() -> axum::Json<ContextResponse> {
    let context = DEVICE_CONTEXT.lock().unwrap().clone();
    axum::Json(ContextResponse { context })
}

/// Set the current device context
pub async fn set_context(payload: axum::Json<SetContextRequest>) -> axum::http::StatusCode {
    let mut context = DEVICE_CONTEXT.lock().unwrap();
    *context = Some(DeviceContext {
        device_id: payload.device_id.clone(),
        btc_address: payload.btc_address.clone(),
        label: payload.label.clone(),
        set_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    });
    
    info!("Device context set: device_id={}, btc_address={:?}, label={:?}", 
          payload.device_id, payload.btc_address, payload.label);
    
    axum::http::StatusCode::NO_CONTENT
}

/// Clear the current device context
pub async fn clear_context() -> axum::http::StatusCode {
    let mut context = DEVICE_CONTEXT.lock().unwrap();
    *context = None;
    axum::http::StatusCode::NO_CONTENT
}

/// Get the current device ID from context
pub fn get_current_device_id() -> Option<String> {
    DEVICE_CONTEXT.lock().unwrap().as_ref().map(|c| c.device_id.clone())
}

/// Get the current Bitcoin address from context
pub fn get_current_btc_address() -> Option<String> {
    DEVICE_CONTEXT.lock().unwrap().as_ref().and_then(|c| c.btc_address.clone())
}

/// Get both device ID and Bitcoin address from context
pub fn get_current_context_info() -> Option<(String, Option<String>)> {
    DEVICE_CONTEXT.lock().unwrap().as_ref().map(|c| (c.device_id.clone(), c.btc_address.clone()))
} 