use axum::{
    extract::State,
    http::StatusCode,
    Json,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{info, error, warn};
use utoipa::ToSchema;

use crate::server::ServerState;
use crate::server::context::{self};

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub device_id: String,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    pub is_keepkey: bool,
    pub keepkey_info: Option<KeepKeyInfo>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KeepKeyInfo {
    pub label: Option<String>,
    pub device_id: Option<String>,
    pub firmware_version: String,
    pub revision: Option<String>,
    pub bootloader_hash: Option<String>,
    pub bootloader_version: Option<String>,
    pub initialized: bool,
    pub bootloader_mode: bool,
}

// SDK compatible Features structure
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct Features {
    pub vendor: Option<String>,
    pub major_version: Option<u32>,
    pub minor_version: Option<u32>,
    pub patch_version: Option<u32>,
    pub bootloader_mode: Option<bool>,
    pub device_id: Option<String>,
    pub pin_protection: Option<bool>,
    pub passphrase_protection: Option<bool>,
    pub language: Option<String>,
    pub label: Option<String>,
    pub initialized: Option<bool>,
    pub revision: Option<String>,
    pub firmware_hash: Option<String>,
    pub bootloader_hash: Option<String>,
    pub imported: Option<bool>,
    pub pin_cached: Option<bool>,
    pub passphrase_cached: Option<bool>,
    pub model: Option<String>,
    pub firmware_variant: Option<String>,
    pub no_backup: Option<bool>,
}

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/api/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    ),
    tag = "system"
)]
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: "2.0.0".to_string(),
    })
}

/// Get device context
#[utoipa::path(
    get,
    path = "/api/context",
    responses(
        (status = 200, description = "Get current device context", body = ContextResponse)
    ),
    tag = "device"
)]
pub async fn api_get_context() -> Json<context::ContextResponse> {
    context::get_context().await
}

/// Set device context
#[utoipa::path(
    post,
    path = "/api/context",
    request_body = SetContextRequest,
    responses(
        (status = 204, description = "Set device context")
    ),
    tag = "device"
)]
pub async fn api_set_context(payload: Json<context::SetContextRequest>) -> StatusCode {
    context::set_context(payload).await
}

/// Clear device context
#[utoipa::path(
    delete,
    path = "/api/context",
    responses(
        (status = 204, description = "Clear device context")
    ),
    tag = "device"
)]
pub async fn api_clear_context() -> StatusCode {
    context::clear_context().await
}

/// List connected devices
#[utoipa::path(
    get,
    path = "/api/devices",
    responses(
        (status = 200, description = "List of connected KeepKey devices", body = Vec<DeviceInfo>),
        (status = 500, description = "Internal server error")
    ),
    tag = "device"
)]
pub async fn api_list_devices(State(state): State<Arc<ServerState>>) -> Result<Json<Vec<DeviceInfo>>, StatusCode> {
    // List connected devices (direct access for enumeration is OK)
    let devices = keepkey_rust::features::list_connected_devices();
    
    let mut device_infos = Vec::new();
    
    // Get device queue manager from state
    let queue_manager = &state.device_queue_manager;
    
    for device in devices {
        // For each device, try to get features through the queue
        let queue_handle = {
            let mut manager = queue_manager.lock().await;
            
            if let Some(handle) = manager.get(&device.unique_id) {
                handle.clone()
            } else {
                // Spawn a new device worker if not exists
                let handle = keepkey_rust::device_queue::DeviceQueueFactory::spawn_worker(
                    device.unique_id.clone(), 
                    device.clone()
                );
                manager.insert(device.unique_id.clone(), handle.clone());
                handle
            }
        };
        
        // Try to get features through the queue (non-blocking, with timeout)
        let keepkey_info = match tokio::time::timeout(
            std::time::Duration::from_millis(500),
            queue_handle.get_features()
        ).await {
            Ok(Ok(raw_features)) => {
                let features = crate::commands::convert_features_to_device_features(raw_features);
                Some(KeepKeyInfo {
                    label: features.label.clone(),
                    device_id: features.device_id.clone(),
                    firmware_version: features.version.clone(),
                    revision: features.firmware_hash.clone(),
                    bootloader_hash: features.bootloader_hash.clone(),
                    bootloader_version: None,
                    initialized: features.initialized,
                    bootloader_mode: features.bootloader_mode,
                })
            }
            Ok(Err(e)) => {
                warn!("Failed to get features for device {} through queue: {}", device.unique_id, e);
                None
            }
            Err(_) => {
                warn!("Timeout getting features for device {}", device.unique_id);
                None
            }
        };
        
        device_infos.push(DeviceInfo {
            device_id: device.unique_id,
            name: device.name,
            vendor_id: device.vid,
            product_id: device.pid,
            manufacturer: device.manufacturer,
            product: device.product,
            serial_number: device.serial_number,
            is_keepkey: device.is_keepkey,
            keepkey_info,
        });
    }
    
    info!("Found {} KeepKey device(s)", device_infos.len());
    Ok(Json(device_infos))
}

/// Get device features (SDK compatible format)
#[utoipa::path(
    post,
    path = "/system/info/get-features",
    responses(
        (status = 200, description = "Device features retrieved successfully", body = Features),
        (status = 400, description = "No device context set"),
        (status = 404, description = "Device not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "device"
)]
pub async fn api_get_features(State(state): State<Arc<ServerState>>) -> Result<Json<Features>, StatusCode> {
    // Get the current device context or default to first available device
    let devices = keepkey_rust::features::list_connected_devices();
    
    let device_id = match context::get_current_context_info() {
        Some((id, _)) => id,
        None => {
            // No context set, try to use first available device
            let first_device = devices
                .iter()
                .filter(|d| d.is_keepkey)
                .next()
                .ok_or_else(|| {
                    error!("No KeepKey devices connected");
                    StatusCode::NOT_FOUND
                })?;
            info!("No device context set, defaulting to first available device: {}", first_device.unique_id);
            first_device.unique_id.clone()
        }
    };
    
    // Find the device by ID
    let device = devices
        .iter()
        .find(|d| d.unique_id == device_id)
        .ok_or_else(|| {
            error!("Device {} not found", device_id);
            StatusCode::NOT_FOUND
        })?;
    
    // Get or create device queue handle
    let queue_manager = &state.device_queue_manager;
    let queue_handle = {
        let mut manager = queue_manager.lock().await;
        
        if let Some(handle) = manager.get(&device_id) {
            handle.clone()
        } else {
            // Spawn a new device worker
            let handle = keepkey_rust::device_queue::DeviceQueueFactory::spawn_worker(
                device_id.clone(), 
                device.clone()
            );
            manager.insert(device_id.clone(), handle.clone());
            handle
        }
    };
    
    // Get device features through the queue
    match queue_handle.get_features().await {
        Ok(raw_features) => {
            let device_features = crate::commands::convert_features_to_device_features(raw_features);
            
            // Parse version to extract major/minor/patch
            let version_parts: Vec<&str> = device_features.version.split('.').collect();
            let major_version = version_parts.get(0).and_then(|v| v.parse::<u32>().ok());
            let minor_version = version_parts.get(1).and_then(|v| v.parse::<u32>().ok());
            let patch_version = version_parts.get(2).and_then(|v| v.parse::<u32>().ok());
            
            // Convert to SDK format
            let features = Features {
                vendor: device_features.vendor.clone(),
                major_version,
                minor_version,
                patch_version,
                bootloader_mode: Some(device_features.bootloader_mode),
                device_id: device_features.device_id.clone(),
                pin_protection: Some(device_features.pin_protection),
                passphrase_protection: Some(device_features.passphrase_protection),
                language: device_features.language.clone(),
                label: device_features.label.clone(),
                initialized: Some(device_features.initialized),
                revision: device_features.firmware_hash.clone(),
                firmware_hash: device_features.firmware_hash.clone(),
                bootloader_hash: device_features.bootloader_hash.clone(),
                imported: device_features.imported,
                pin_cached: Some(device_features.pin_cached),
                passphrase_cached: Some(device_features.passphrase_cached),
                model: device_features.model.clone(),
                firmware_variant: device_features.firmware_variant.clone(),
                no_backup: Some(device_features.no_backup),
            };
            
            info!("✅ Retrieved device features for device {}", device_id);
            Ok(Json(features))
        }
        Err(e) => {
            error!("Failed to get device features through queue: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// MCP (Model Context Protocol) Types

#[derive(Debug, Deserialize)]
struct McpRequest {
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

#[derive(Debug, Serialize)]
struct McpResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
    id: Option<Value>,
}

#[derive(Debug, Serialize)]
struct McpError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// MCP endpoint handler
#[utoipa::path(
    post,
    path = "/mcp",
    request_body = Value,
    responses(
        (status = 200, description = "MCP response", body = Value)
    ),
    tag = "mcp"
)]
pub async fn mcp_handle(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<Value>,
) -> impl IntoResponse {
    info!("MCP request received: {:?}", request);
    
    // Parse the request as MCP JSON-RPC
    let mcp_request: McpRequest = match serde_json::from_value(request) {
        Ok(req) => req,
        Err(e) => {
            error!("Invalid MCP request: {}", e);
            return Json(json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32700,
                    "message": "Parse error"
                },
                "id": null
            }));
        }
    };
    
    // Handle different MCP methods
    let response = match mcp_request.method.as_str() {
        "ping" => {
            McpResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(json!({ "status": "pong" })),
                error: None,
                id: mcp_request.id,
            }
        }
        
        "resources/list" => {
            // List available resources
            McpResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(json!({
                    "resources": [
                        {
                            "uri": "device://current",
                            "name": "Current Device",
                            "description": "The currently selected KeepKey device"
                        },
                        {
                            "uri": "device://context",
                            "name": "Device Context",
                            "description": "Current device context including Bitcoin address"
                        }
                    ]
                })),
                error: None,
                id: mcp_request.id,
            }
        }
        
        "resources/read" => {
            // Read a specific resource
            if let Some(params) = mcp_request.params {
                if let Some(uri) = params.get("uri").and_then(|u| u.as_str()) {
                    match uri {
                        "device://current" => {
                            let context = context::get_current_context_info();
                            McpResponse {
                                jsonrpc: "2.0".to_string(),
                                result: Some(json!({
                                    "contents": {
                                        "text": if let Some((device_id, btc_address)) = context {
                                            format!("Device ID: {}\nBitcoin Address: {:?}", device_id, btc_address)
                                        } else {
                                            "No device currently selected".to_string()
                                        }
                                    }
                                })),
                                error: None,
                                id: mcp_request.id,
                            }
                        }
                        "device://context" => {
                            let context_response = context::get_context().await;
                            McpResponse {
                                jsonrpc: "2.0".to_string(),
                                result: Some(serde_json::to_value(&context_response.0).unwrap_or(json!({}))),
                                error: None,
                                id: mcp_request.id,
                            }
                        }
                        _ => {
                            McpResponse {
                                jsonrpc: "2.0".to_string(),
                                result: None,
                                error: Some(McpError {
                                    code: -32602,
                                    message: format!("Unknown resource URI: {}", uri),
                                    data: None,
                                }),
                                id: mcp_request.id,
                            }
                        }
                    }
                } else {
                    McpResponse {
                        jsonrpc: "2.0".to_string(),
                        result: None,
                        error: Some(McpError {
                            code: -32602,
                            message: "Missing 'uri' parameter".to_string(),
                            data: None,
                        }),
                        id: mcp_request.id,
                    }
                }
            } else {
                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: "Missing parameters".to_string(),
                        data: None,
                    }),
                    id: mcp_request.id,
                }
            }
        }
        
        "tools/list" => {
            // List available tools
            McpResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(json!({
                    "tools": [
                        {
                            "name": "get_device_status",
                            "description": "Get the current device status",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        },
                        {
                            "name": "get_device_features",
                            "description": "Get detailed features of the current device",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        },
                        {
                            "name": "list_devices",
                            "description": "List all connected KeepKey devices",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        },
                        {
                            "name": "get_bitcoin_address",
                            "description": "Get a Bitcoin address for the current device",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "path": {
                                        "type": "string",
                                        "description": "BIP32 derivation path (e.g., m/84'/0'/0'/0/0)"
                                    },
                                    "script_type": {
                                        "type": "string",
                                        "enum": ["p2pkh", "p2sh-p2wpkh", "p2wpkh"],
                                        "description": "Bitcoin address type"
                                    }
                                }
                            }
                        }
                    ]
                })),
                error: None,
                id: mcp_request.id,
            }
        }
        
        "tools/call" => {
            // Call a specific tool
            if let Some(params) = mcp_request.params {
                if let Some(name) = params.get("name").and_then(|n| n.as_str()) {
                    match name {
                        "get_device_status" => {
                            let (device_id, btc_address) = context::get_current_context_info()
                                .unwrap_or_else(|| ("No device".to_string(), None));
                            
                            McpResponse {
                                jsonrpc: "2.0".to_string(),
                                result: Some(json!({
                                    "content": [
                                        {
                                            "type": "text",
                                            "text": format!("Current device: {}\nBitcoin address: {:?}", device_id, btc_address)
                                        }
                                    ]
                                })),
                                error: None,
                                id: mcp_request.id,
                            }
                        }
                        "get_device_features" => {
                            match api_get_features(State(state.clone())).await {
                                Ok(Json(features)) => {
                                    McpResponse {
                                        jsonrpc: "2.0".to_string(),
                                        result: Some(json!({
                                            "content": [
                                                {
                                                    "type": "text",
                                                    "text": serde_json::to_string_pretty(&features).unwrap_or_else(|_| "Failed to serialize features".to_string())
                                                }
                                            ]
                                        })),
                                        error: None,
                                        id: mcp_request.id,
                                    }
                                }
                                Err(_) => {
                                    McpResponse {
                                        jsonrpc: "2.0".to_string(),
                                        result: None,
                                        error: Some(McpError {
                                            code: -32603,
                                            message: "Failed to get device features".to_string(),
                                            data: None,
                                        }),
                                        id: mcp_request.id,
                                    }
                                }
                            }
                        }
                        "list_devices" => {
                            match api_list_devices(State(state.clone())).await {
                                Ok(Json(devices)) => {
                                    McpResponse {
                                        jsonrpc: "2.0".to_string(),
                                        result: Some(json!({
                                            "content": [
                                                {
                                                    "type": "text",
                                                    "text": serde_json::to_string_pretty(&devices).unwrap_or_else(|_| "Failed to serialize devices".to_string())
                                                }
                                            ]
                                        })),
                                        error: None,
                                        id: mcp_request.id,
                                    }
                                }
                                Err(_) => {
                                    McpResponse {
                                        jsonrpc: "2.0".to_string(),
                                        result: None,
                                        error: Some(McpError {
                                            code: -32603,
                                            message: "Failed to list devices".to_string(),
                                            data: None,
                                        }),
                                        id: mcp_request.id,
                                    }
                                }
                            }
                        }
                        "get_bitcoin_address" => {
                            // TODO: Implement actual address generation
                            McpResponse {
                                jsonrpc: "2.0".to_string(),
                                result: Some(json!({
                                    "content": [
                                        {
                                            "type": "text",
                                            "text": "Bitcoin address generation not yet implemented"
                                        }
                                    ]
                                })),
                                error: None,
                                id: mcp_request.id,
                            }
                        }
                        _ => {
                            McpResponse {
                                jsonrpc: "2.0".to_string(),
                                result: None,
                                error: Some(McpError {
                                    code: -32602,
                                    message: format!("Unknown tool: {}", name),
                                    data: None,
                                }),
                                id: mcp_request.id,
                            }
                        }
                    }
                } else {
                    McpResponse {
                        jsonrpc: "2.0".to_string(),
                        result: None,
                        error: Some(McpError {
                            code: -32602,
                            message: "Missing 'name' parameter".to_string(),
                            data: None,
                        }),
                        id: mcp_request.id,
                    }
                }
            } else {
                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: "Missing parameters".to_string(),
                        data: None,
                    }),
                    id: mcp_request.id,
                }
            }
        }
        
        _ => {
            warn!("Unknown MCP method: {}", mcp_request.method);
            McpResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(McpError {
                    code: -32601,
                    message: format!("Method not found: {}", mcp_request.method),
                    data: None,
                }),
                id: mcp_request.id,
            }
        }
    };
    
    Json(serde_json::to_value(response).unwrap_or(json!({})))
} 