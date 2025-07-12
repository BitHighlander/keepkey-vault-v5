use tauri::{AppHandle, Emitter, State};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use keepkey_rust::{
    device_queue::{DeviceQueueFactory, DeviceQueueHandle},
    features::DeviceFeatures,
};
use uuid;
use hex;
use std::io::Cursor;
use once_cell;
// Removed unused imports that were moved to device/updates.rs
use crate::logging::{log_device_request, log_device_response, log_raw_device_message};
use lazy_static;
use std::path::PathBuf;
use std::fs;
use serde_json::Value;
use log;
use crate::device::updates::{update_device_bootloader, update_device_firmware};

// Add timeout constant
const DEVICE_OPERATION_TIMEOUT_SECS: u64 = 30; // Increased from 5 to 30 seconds

// Add device cleanup tracking
lazy_static::lazy_static! {
    static ref DEVICE_CLEANUP_TRACKER: Arc<tokio::sync::Mutex<std::collections::HashSet<String>>> = Arc::new(tokio::sync::Mutex::new(std::collections::HashSet::new()));
}

pub type DeviceQueueManager = Arc<tokio::sync::Mutex<std::collections::HashMap<String, DeviceQueueHandle>>>;

// Change the response storage to use request_id as key instead of device_id
type LastResponsesMap = Arc<tokio::sync::Mutex<std::collections::HashMap<String, DeviceResponse>>>;

// Add frontend readiness state and queued events
lazy_static::lazy_static! {
    static ref FRONTEND_READY_STATE: Arc<tokio::sync::RwLock<FrontendReadyState>> = Arc::new(tokio::sync::RwLock::new(FrontendReadyState::default()));
}

#[derive(Debug, Clone)]
struct FrontendReadyState {
    is_ready: bool,
    queued_events: Vec<QueuedEvent>,
}

impl Default for FrontendReadyState {
    fn default() -> Self {
        Self {
            is_ready: false,
            queued_events: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueuedEvent {
    event_name: String,
    payload: serde_json::Value,
    timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BitcoinUtxoInput {
    pub address_n_list: Vec<u32>,     // Derivation path [2147483692, 2147483648, ...]
    pub script_type: String,          // "p2pkh", "p2sh", "p2wpkh"
    pub amount: String,               // Amount in satoshis as string
    pub vout: u32,                    // Output index
    pub txid: String,                 // Transaction ID
    pub prev_tx_hex: Option<String>,  // Raw previous transaction hex
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BitcoinUtxoOutput {
    pub address: String,              // Destination address
    pub amount: u64,                  // Amount in satoshis
    pub address_type: String,         // "spend" or "change"
    pub is_change: Option<bool>,      // Optional change flag
    pub address_n_list: Option<Vec<u32>>, // Derivation path for change outputs
    pub script_type: Option<String>,  // Script type for change outputs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceRequest {
    // ============ Address Generation ============
    GetXpub {
        path: String,
    },
    GetAddress {
        path: String,
        coin_name: String,
        script_type: Option<String>,
        show_display: Option<bool>,
    },
    // Thorchain operations
    ThorchainGetAddress {
        path: String,
        testnet: bool,
        show_display: Option<bool>,
    },
    // Cosmos operations
    CosmosGetAddress {
        path: String,
        hrp: String,
        show_display: Option<bool>,
    },
    // Ethereum operations  
    EthereumGetAddress {
        path: String,
        show_display: Option<bool>,
    },
    // Binance Chain
    BinanceGetAddress {
        path: String,
        show_display: Option<bool>,
    },
    // Osmosis
    OsmosisGetAddress {
        path: String,
        show_display: Option<bool>,
    },
    // Tendermint
    TendermintGetAddress {
        path: String,
        show_display: Option<bool>,
    },
    // Mayachain
    MayachainGetAddress {
        path: String,
        show_display: Option<bool>,
    },
    // XRP
    XrpGetAddress {
        path: String,
        show_display: Option<bool>,
    },
    
    // ============ Transaction Signing ============
    SignTransaction {
        coin: String,
        inputs: Vec<BitcoinUtxoInput>,
        outputs: Vec<BitcoinUtxoOutput>,
        version: u32,
        lock_time: u32,
    },
    // Ethereum signing
    EthereumSignTransaction {
        nonce: String,
        gas_price: Option<String>,
        gas_limit: String,
        to: String,
        value: String,
        data: Option<String>,
        chain_id: u32,
        max_fee_per_gas: Option<String>,
        max_priority_fee_per_gas: Option<String>,
        access_list: Option<Vec<serde_json::Value>>,
    },
    EthereumSignMessage {
        message: String,
        address: String,
    },
    EthereumSignTypedData {
        typed_data: serde_json::Value,
        address: String,
    },
    // Cosmos/Thorchain/Osmosis Amino signing
    CosmosSignAmino {
        sign_doc: serde_json::Value,
        signer_address: String,
    },
    ThorchainSignAmino {
        sign_doc: serde_json::Value,
        signer_address: String,
    },
    OsmosisSignAmino {
        sign_doc: serde_json::Value,
        signer_address: String,
    },
    MayachainSignAmino {
        sign_doc: serde_json::Value,
        signer_address: String,
    },
    // Binance signing
    BinanceSignTransaction {
        sign_doc: serde_json::Value,
        signer_address: String,
    },
    // XRP signing
    XrpSignTransaction {
        transaction: serde_json::Value,
    },
    
    // ============ System Operations ============
    GetFeatures,
    Ping {
        message: Option<String>,
        button_protection: Option<bool>,
    },
    GetEntropy {
        size: u32,
    },
    GetPublicKey {
        path: String,
        coin_name: Option<String>,
        script_type: Option<String>,
        ecdsa_curve_name: Option<String>,
        show_display: Option<bool>,
    },
    ListCoins,
    
    // ============ Device Management ============
    ApplySettings {
        label: Option<String>,
        language: Option<String>,
        use_passphrase: Option<bool>,
        auto_lock_delay_ms: Option<u32>,
        u2f_counter: Option<u32>,
    },
    ApplyPolicies {
        policies: Vec<serde_json::Value>,
    },
    ChangePin {
        remove: Option<bool>,
    },
    ClearSession,
    WipeDevice,
    
    // ============ Device Initialization ============
    ResetDevice {
        display_random: Option<bool>,
        strength: Option<u32>,
        passphrase_protection: Option<bool>,
        pin_protection: Option<bool>,
        language: Option<String>,
        label: Option<String>,
        no_backup: Option<bool>,
        auto_lock_delay_ms: Option<u32>,
        u2f_counter: Option<u32>,
    },
    RecoverDevice {
        word_count: u32,
        passphrase_protection: Option<bool>,
        pin_protection: Option<bool>,
        language: Option<String>,
        label: Option<String>,
        dry_run: Option<bool>,
        auto_lock_delay_ms: Option<u32>,
        u2f_counter: Option<u32>,
    },
    LoadDevice {
        mnemonic: Option<String>,
        xprv: Option<String>,
        pin: Option<String>,
        passphrase_protection: Option<bool>,
        language: Option<String>,
        label: Option<String>,
        skip_checksum: Option<bool>,
        u2f_counter: Option<u32>,
    },
    
    // ============ Advanced Operations ============
    SignIdentity {
        identity: serde_json::Value,
        challenge_hidden: String,
        challenge_visual: Option<String>,
        ecdsa_curve_name: Option<String>,
    },
    CipherKeyValue {
        address_n: Vec<u32>,
        key: String,
        value: String,
        encrypt: bool,
        ask_on_encrypt: Option<bool>,
        ask_on_decrypt: Option<bool>,
        iv: Option<String>,
    },
    FirmwareUpdate {
        payload: Vec<u8>,
    },
    
    // Generic raw message sending
    SendRaw {
        message_type: String,
        message_data: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRequestWrapper {
    pub device_id: String,
    pub request_id: String,
    pub request: DeviceRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceResponse {
    // ============ Address Responses ============
    Xpub {
        request_id: String,
        device_id: String,
        path: String,
        xpub: String,
        script_type: Option<String>,
        success: bool,
        error: Option<String>,
    },
    Address {
        request_id: String,
        device_id: String,
        path: String,
        address: String,
        success: bool,
        error: Option<String>,
    },
    ThorchainAddress {
        request_id: String,
        device_id: String,
        path: String,
        address: String,
        success: bool,
        error: Option<String>,
    },
    CosmosAddress {
        request_id: String,
        device_id: String,
        path: String,
        address: String,
        success: bool,
        error: Option<String>,
    },
    EthereumAddress {
        request_id: String,
        device_id: String,
        path: String,
        address: String,
        success: bool,
        error: Option<String>,
    },
    BinanceAddress {
        request_id: String,
        device_id: String,
        path: String,
        address: String,
        success: bool,
        error: Option<String>,
    },
    OsmosisAddress {
        request_id: String,
        device_id: String,
        path: String,
        address: String,
        success: bool,
        error: Option<String>,
    },
    TendermintAddress {
        request_id: String,
        device_id: String,
        path: String,
        address: String,
        success: bool,
        error: Option<String>,
    },
    MayachainAddress {
        request_id: String,
        device_id: String,
        path: String,
        address: String,
        success: bool,
        error: Option<String>,
    },
    XrpAddress {
        request_id: String,
        device_id: String,
        path: String,
        address: String,
        success: bool,
        error: Option<String>,
    },
    
    // ============ Transaction Signing Responses ============
    SignedTransaction {
        request_id: String,
        device_id: String,
        signed_tx: String,         // Signed transaction hex
        txid: Option<String>,      // Transaction ID if available
        success: bool,
        error: Option<String>,
    },
    EthereumSignedTransaction {
        request_id: String,
        device_id: String,
        serialized: String,
        v: u32,
        r: String,
        s: String,
        success: bool,
        error: Option<String>,
    },
    EthereumSignedMessage {
        request_id: String,
        device_id: String,
        signature: String,
        success: bool,
        error: Option<String>,
    },
    CosmosSignedAmino {
        request_id: String,
        device_id: String,
        signature: String,
        serialized: String,
        success: bool,
        error: Option<String>,
    },
    
    // ============ System Responses ============
    Features {
        request_id: String,
        device_id: String,
        features: DeviceFeatures,
        success: bool,
        error: Option<String>,
    },
    PingResponse {
        request_id: String,
        device_id: String,
        message: String,
        success: bool,
        error: Option<String>,
    },
    Entropy {
        request_id: String,
        device_id: String,
        entropy: String,
        success: bool,
        error: Option<String>,
    },
    PublicKey {
        request_id: String,
        device_id: String,
        xpub: String,
        node: Option<serde_json::Value>,
        success: bool,
        error: Option<String>,
    },
    CoinList {
        request_id: String,
        device_id: String,
        coins: Vec<serde_json::Value>,
        success: bool,
        error: Option<String>,
    },
    
    // ============ Device Management Responses ============
    Success {
        request_id: String,
        device_id: String,
        message: Option<String>,
        success: bool,
        error: Option<String>,
    },
    
    // Generic raw response
    Raw {
        request_id: String,
        device_id: String,
        response: serde_json::Value,
        success: bool,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatus {
    pub device_id: Option<String>,
    pub total_queued: usize,
    pub active_operations: usize,
    pub status: String,
    pub last_response: Option<DeviceResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub device_id: String,
    pub connected: bool,
    pub features: Option<DeviceFeatures>,
    pub needs_bootloader_update: bool,
    pub needs_firmware_update: bool,
    pub needs_initialization: bool,
    pub needs_pin_unlock: bool,
    pub bootloader_check: Option<BootloaderCheck>,
    pub firmware_check: Option<FirmwareCheck>,
    pub initialization_check: Option<InitializationCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootloaderCheck {
    pub current_version: String,
    pub latest_version: String,
    pub needs_update: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareCheck {
    pub current_version: String,
    pub latest_version: String,
    pub needs_update: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializationCheck {
    pub initialized: bool,
    pub has_backup: bool,
    pub imported: bool,
    pub needs_setup: bool,
}

/// Unified device queue command - all device operations go through this
#[tauri::command]
pub async fn reset_device_queue(
    device_id: String,
    queue_manager: State<'_, DeviceQueueManager>,
) -> Result<(), String> {
    println!("♻️  Resetting device queue for {}", device_id);
    let mut manager = queue_manager.lock().await;
    if let Some(handle) = manager.remove(&device_id) {
        let _ = handle.shutdown().await;
    }
    Ok(())
}

// The add_to_device_queue implementation has been moved to device/queue.rs for modularity and maintainability.
// Please update imports if you use this function directly.

/// Get the current status of a specific device queue
#[tauri::command]
pub async fn get_queue_status(
    device_id: Option<String>,
    queue_manager: State<'_, DeviceQueueManager>,
    last_responses: State<'_, Arc<tokio::sync::Mutex<std::collections::HashMap<String, DeviceResponse>>>>,
) -> Result<QueueStatus, String> {
    let manager = queue_manager.lock().await;
    let responses = last_responses.lock().await;
    
    println!("[QUEUE_STATUS_CALL] get_queue_status called for device: {:?}", device_id);
    if let Some(device_id) = device_id {
        // Find the most recent response for this device_id from all stored responses
        let mut last_response = None;
        let mut newest_timestamp = 0u64;
        
        for (request_id, response) in responses.iter() {
            // Check if this response belongs to the requested device
            if match response {
                DeviceResponse::Xpub { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::Address { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::ThorchainAddress { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::CosmosAddress { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::EthereumAddress { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::BinanceAddress { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::OsmosisAddress { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::TendermintAddress { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::MayachainAddress { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::XrpAddress { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::Features { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::SignedTransaction { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::EthereumSignedTransaction { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::EthereumSignedMessage { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::CosmosSignedAmino { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::PingResponse { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::Entropy { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::PublicKey { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::CoinList { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::Success { device_id: resp_device_id, .. } => resp_device_id == &device_id,
                DeviceResponse::Raw { device_id: resp_device_id, .. } => resp_device_id == &device_id,
            } {
                // For now, use a simple heuristic: the response with the most recent request_id (timestamp-based)
                // Extract timestamp from request_id format like "sign_tx_1750283052093_4af7u3ebr"
                if let Some(timestamp_str) = request_id.split('_').nth(2) {
                    if let Ok(timestamp) = timestamp_str.parse::<u64>() {
                        if timestamp > newest_timestamp {
                            newest_timestamp = timestamp;
                            last_response = Some(response.clone());
                            if let DeviceResponse::SignedTransaction { request_id, .. } = response {
                                println!("[QUEUE_STATUS_SIGNED_TX] Returning SignedTransaction for request_id: {} in get_queue_status for device_id: {}", request_id, device_id);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(QueueStatus {
            device_id: Some(device_id.clone()),
            total_queued: 0, // Would need to track this per device
            active_operations: if manager.contains_key(&device_id) { 1 } else { 0 },
            status: if manager.contains_key(&device_id) { "active".to_string() } else { "idle".to_string() },
            last_response,
        })
    } else {
        // Return general status
        Ok(QueueStatus {
            device_id: None,
            total_queued: 0,
            active_operations: manager.len(),
            status: if manager.is_empty() { "idle".to_string() } else { "active".to_string() },
            last_response: None,
        })
    }
}

/// Get connected devices (frontend expects this name)
#[tauri::command]
pub async fn get_connected_devices() -> Result<Vec<serde_json::Value>, String> {
    let devices = keepkey_rust::features::list_connected_devices();
    
    // Convert to the structure the frontend expects
    let json_devices = devices.into_iter()
        .filter(|device| device.is_keepkey)
        .map(|device| {
            serde_json::json!({
                "device": {
                    "unique_id": device.unique_id,
                    "name": device.name,
                    "vid": device.vid,
                    "pid": device.pid,
                    "manufacturer": device.manufacturer,
                    "product": device.product,
                    "serial_number": device.serial_number,
                    "is_keepkey": device.is_keepkey,
                },
                "features": null, // Features fetched separately via queue
            })
        })
        .collect();
    
    Ok(json_devices)
}

/// Get blocking actions (enhanced version)
#[tauri::command]
pub async fn get_blocking_actions() -> Result<Vec<serde_json::Value>, String> {
    // For now, return empty array since vault v2 uses DeviceUpdateManager with its own logic
    // TODO: Implement proper blocking actions registry like vault v1
    Ok(vec![])
}

/// Helper function to parse derivation path string to Vec<u32>
pub fn parse_derivation_path(path: &str) -> Result<Vec<u32>, String> {
    let path = path.trim_start_matches("m/");
    let parts: Result<Vec<u32>, String> = path
        .split('/')
        .map(|part| {
            // Check if this specific part is hardened (ends with ')
            let is_hardened = part.ends_with('\'');
            let part = part.trim_end_matches('\'');
            let mut value = part.parse::<u32>()
                .map_err(|_| format!("Invalid path component: {}", part))?;
            
            // Apply hardened derivation if this part ends with '
            if is_hardened {
                value |= 0x80000000;
            }
            
            Ok(value)
        })
        .collect();
    
    parts.map_err(|e: String| format!("Failed to parse derivation path '{}': {}", path, e))
}

/// Test command to demonstrate the unified device queue interface
#[tauri::command]
pub async fn test_device_queue() -> Result<String, String> {
    println!("🧪 Testing unified device queue interface...");

    // Example of how frontend would use the unified interface
    let test_request = DeviceRequestWrapper {
        device_id: "test-device-001".to_string(),
        request_id: uuid::Uuid::new_v4().to_string(),
        request: DeviceRequest::GetFeatures,
    };

    println!("📝 Created test request: {:?}", test_request);

    // In real usage, this would be sent to add_to_device_queue
    Ok(format!("✅ Unified device queue test completed. Request ID: {}", test_request.request_id))
}

/// Get device status including update needs and initialization status
#[tauri::command]
pub async fn get_device_status(
    device_id: String,
    queue_manager: State<'_, DeviceQueueManager>,
) -> Result<Option<DeviceStatus>, String> {
    println!("Getting device status for: {}", device_id);
    
    let request_id = uuid::Uuid::new_v4().to_string();
    
    // Log the request
    let request_data = serde_json::json!({
        "device_id": device_id,
        "operation": "get_device_status"
    });
    
    if let Err(e) = log_device_request(&device_id, &request_id, "GetDeviceStatus", &request_data).await {
        eprintln!("Failed to log get device status request: {}", e);
    }
    
    // Get connected devices to find the one we want
    let devices = keepkey_rust::features::list_connected_devices();
    let device_info = devices
        .iter()
        .find(|d| d.unique_id == device_id);
    
    if let Some(device_info) = device_info {
        // Get or create device queue handle
        let queue_handle = {
            let mut manager = queue_manager.lock().await;
            
            if let Some(handle) = manager.get(&device_id) {
                handle.clone()
            } else {
                // Spawn a new device worker
                let handle = DeviceQueueFactory::spawn_worker(device_id.clone(), device_info.clone());
                manager.insert(device_id.clone(), handle.clone());
                handle
            }
        };
        
        // Fetch device features through the queue
        let features = match tokio::time::timeout(
            std::time::Duration::from_secs(DEVICE_OPERATION_TIMEOUT_SECS),
            queue_handle.get_features()
        ).await {
            Ok(Ok(raw_features)) => {
                // Convert from raw Features message to DeviceFeatures
                Some(convert_features_to_device_features(raw_features))
            }
            Ok(Err(e)) => {
                println!("Failed to get features for device {}: {}", device_id, e);
                
                // Log failed feature retrieval
                let device_response_data = serde_json::json!({
                    "error": format!("Failed to get features: {}", e),
                    "operation": "get_features_for_device"
                });
                
                if let Err(log_err) = log_device_response(&device_id, &request_id, false, &device_response_data, Some(&format!("Failed to get features: {}", e))).await {
                    eprintln!("Failed to log device features error response: {}", log_err);
                }
                
                None
            }
            Err(_) => {
                println!("Timeout getting features for device {}", device_id);
                
                // Log timeout
                let device_response_data = serde_json::json!({
                    "error": "Timeout getting features",
                    "operation": "get_features_for_device"
                });
                
                if let Err(e) = log_device_response(&device_id, &request_id, false, &device_response_data, Some("Timeout getting features")).await {
                    eprintln!("Failed to log device features timeout response: {}", e);
                }
                
                None
            }
        };
        
        // Evaluate device status
        let status = evaluate_device_status(device_id.clone(), features.as_ref());
        
        // Log the response
        let response_data = serde_json::json!({
            "status": status,
            "operation": "get_device_status"
        });
        
        if let Err(e) = log_device_response(&device_id, &request_id, true, &response_data, None).await {
            eprintln!("Failed to log get device status response: {}", e);
        }
        
        Ok(Some(status))
    } else {
        println!("Device {} not found", device_id);
        
        // Log the not found response
        let response_data = serde_json::json!({
            "error": "Device not found",
            "operation": "get_device_status"
        });
        
        if let Err(e) = log_device_response(&device_id, &request_id, false, &response_data, Some("Device not found")).await {
            eprintln!("Failed to log get device status error response: {}", e);
        }
        
        Ok(None)
    }
}

/// Get device info by ID (features only)
#[tauri::command]
pub async fn get_device_info_by_id(
    device_id: String,
    queue_manager: State<'_, DeviceQueueManager>,
    app: AppHandle,
) -> Result<Option<DeviceFeatures>, String> {
    println!("Getting device info for: {}", device_id);
    
    let request_id = uuid::Uuid::new_v4().to_string();
    
    // Log the request
    let request_data = serde_json::json!({
        "device_id": device_id,
        "operation": "get_device_info_by_id"
    });
    
    if let Err(e) = log_device_request(&device_id, &request_id, "GetDeviceInfo", &request_data).await {
        eprintln!("Failed to log get device info request: {}", e);
    }
    
    // Get or create device queue handle using centralized function
    let queue_handle = match get_or_create_device_queue(&device_id, &queue_manager).await {
        Ok(handle) => handle,
        Err(error) => {
            // Log the error response
            let response_data = serde_json::json!({
                "error": error,
                "operation": "get_device_info_by_id"
            });
            
            if let Err(e) = log_device_response(&device_id, &request_id, false, &response_data, Some(&error)).await {
                eprintln!("Failed to log get device info error response: {}", e);
            }
            
            return Err(error);
        }
    };
    
    // Fetch device features through the queue
    match tokio::time::timeout(
        std::time::Duration::from_secs(DEVICE_OPERATION_TIMEOUT_SECS),
        queue_handle.get_features()
    ).await {
        Ok(Ok(raw_features)) => {
            // Convert from raw Features message to DeviceFeatures
            let device_features = convert_features_to_device_features(raw_features);
            
            // Emit event for frontend listeners (KeepKeyDeviceList etc.)
            let event_payload = serde_json::json!({
                "deviceId": device_id,
                "features": device_features,
                "status": "ready"
            });
            let _ = app.emit("device:features-updated", event_payload);

            // Log the successful response
            let response_data = serde_json::json!({
                "features": device_features,
                "operation": "get_device_info_by_id"
            });
            
            if let Err(e) = log_device_response(&device_id, &request_id, true, &response_data, None).await {
                eprintln!("Failed to log get device info response: {}", e);
            }
            
            Ok(Some(device_features))
        }
        Ok(Err(e)) => {
            let error_msg = e.to_string();
            
            // Check for device access errors (already claimed)
            if error_msg.contains("Device Already In Use") || 
               error_msg.contains("already claimed") ||
               error_msg.contains("Device Access Failed") ||
               error_msg.contains("🔒") {
                
                println!("❌ Device {} is already in use by another application: {}", device_id, e);
                
                // Return the detailed error message from our HID transport
                let user_friendly_error = if error_msg.contains("🔒") {
                    error_msg
                } else {
                    format!(
                        "🔒 KeepKey Device Already In Use\n\n\
                        Your KeepKey device is currently being used by another application.\n\n\
                        Common causes:\n\
                        • KeepKey Desktop app is running\n\
                        • KeepKey Bridge is running\n\
                        • Another wallet application is connected\n\
                        • Previous connection wasn't properly closed\n\n\
                        Solutions:\n\
                        1. Close KeepKey Desktop app completely\n\
                        2. Close any other wallet applications\n\
                        3. Unplug and reconnect your KeepKey device\n\
                        4. Try again\n\n\
                        Technical details: {}", e
                    )
                };
                
                // Emit a special event for device access errors
                let error_event_payload = serde_json::json!({
                    "deviceId": device_id,
                    "error": user_friendly_error,
                    "errorType": "DEVICE_CLAIMED",
                    "status": "error"
                });
                let _ = app.emit("device:access-error", error_event_payload);
                
                // Log the error response
                let response_data = serde_json::json!({
                    "error": user_friendly_error,
                    "errorType": "DEVICE_CLAIMED",
                    "operation": "get_device_info_by_id"
                });
                
                if let Err(log_err) = log_device_response(&device_id, &request_id, false, &response_data, Some(&user_friendly_error)).await {
                    eprintln!("Failed to log get device info error response: {}", log_err);
                }
                
                return Err(user_friendly_error);
            }
            
            // For other errors, use default handling
            println!("Failed to get features for device {}: {}", device_id, e);
            let error = format!("Failed to get device features: {}", e);
            
            // Log the error response
            let response_data = serde_json::json!({
                "error": error,
                "operation": "get_device_info_by_id"
            });
            
            if let Err(log_err) = log_device_response(&device_id, &request_id, false, &response_data, Some(&error)).await {
                eprintln!("Failed to log get device info error response: {}", log_err);
            }
            
            Err(error)
        }
        Err(_) => {
            println!("Timeout getting features for device {}", device_id);
            
            // Log timeout
            let device_response_data = serde_json::json!({
                "error": "Timeout getting features",
                "operation": "get_features_for_device"
            });
            
            if let Err(e) = log_device_response(&device_id, &request_id, false, &device_response_data, Some("Timeout getting features")).await {
                eprintln!("Failed to log device features timeout response: {}", e);
            }
            
            Err("Timeout getting features".to_string())
        }
    }
}

/// Wipe device (factory reset)
#[tauri::command]
pub async fn wipe_device(
    device_id: String,
    queue_manager: State<'_, DeviceQueueManager>,
) -> Result<(), String> {
    println!("Wiping device: {}", device_id);
    
    let request_id = uuid::Uuid::new_v4().to_string();
    
    // Log the request
    let request_data = serde_json::json!({
        "device_id": device_id,
        "operation": "wipe_device"
    });
    
    if let Err(e) = log_device_request(&device_id, &request_id, "WipeDevice", &request_data).await {
        eprintln!("Failed to log wipe device request: {}", e);
    }
    
    // Get or create device queue handle
    let queue_handle = {
        let mut manager = queue_manager.lock().await;
        
        if let Some(handle) = manager.get(&device_id) {
            handle.clone()
        } else {
            // Find the device by ID
            let devices = keepkey_rust::features::list_connected_devices();
            let device_info = devices
                .iter()
                .find(|d| d.unique_id == device_id);
                
            match device_info {
                Some(device_info) => {
                    // Spawn a new device worker
                    let handle = DeviceQueueFactory::spawn_worker(device_id.clone(), device_info.clone());
                    manager.insert(device_id.clone(), handle.clone());
                    handle
                }
                None => {
                    let error = format!("Device {} not found", device_id);
                    
                    // Log the error response
                    let response_data = serde_json::json!({
                        "error": error,
                        "operation": "wipe_device"
                    });
                    
                    if let Err(e) = log_device_response(&device_id, &request_id, false, &response_data, Some(&error)).await {
                        eprintln!("Failed to log wipe device error response: {}", e);
                    }
                    
                    return Err(error);
                }
            }
        }
    };
    
    // Create WipeDevice message
    let wipe_message = keepkey_rust::messages::Message::WipeDevice(
        keepkey_rust::messages::WipeDevice {}
    );
    
    // Log the raw message being sent
    let message_data = serde_json::json!({
        "message_type": "WipeDevice",
        "message": {}
    });
    
    if let Err(e) = log_raw_device_message(&device_id, "SEND", "WipeDevice", &message_data).await {
        eprintln!("Failed to log wipe device raw message: {}", e);
    }
    
    // Send wipe device command through queue
    match queue_handle.send_raw(wipe_message, true).await {
        Ok(response) => {
            // Log the raw response
            let response_message_data = serde_json::json!({
                "response": format!("{:?}", response)
            });
            
            if let Err(e) = log_raw_device_message(&device_id, "RECEIVE", "WipeDeviceResponse", &response_message_data).await {
                eprintln!("Failed to log wipe device raw response: {}", e);
            }
            
            match response {
                keepkey_rust::messages::Message::Success(_) => {
                    println!("✅ Device {} wiped successfully", device_id);
                    
                    // Log the successful response
                    let response_data = serde_json::json!({
                        "success": true,
                        "operation": "wipe_device"
                    });
                    
                    if let Err(e) = log_device_response(&device_id, &request_id, true, &response_data, None).await {
                        eprintln!("Failed to log wipe device response: {}", e);
                    }
                    
                    Ok(())
                }
                keepkey_rust::messages::Message::Failure(failure) => {
                    let error = format!("Device rejected wipe request: {}", failure.message.unwrap_or_default());
                    println!("❌ Failed to wipe device {}: {}", device_id, error);
                    
                    // Log the error response
                    let response_data = serde_json::json!({
                        "error": error,
                        "operation": "wipe_device"
                    });
                    
                    if let Err(e) = log_device_response(&device_id, &request_id, false, &response_data, Some(&error)).await {
                        eprintln!("Failed to log wipe device error response: {}", e);
                    }
                    
                    Err(error)
                }
                _ => {
                    let error = "Unexpected response from device".to_string();
                    println!("❌ Failed to wipe device {}: {}", device_id, error);
                    
                    // Log the error response
                    let response_data = serde_json::json!({
                        "error": error,
                        "operation": "wipe_device"
                    });
                    
                    if let Err(e) = log_device_response(&device_id, &request_id, false, &response_data, Some(&error)).await {
                        eprintln!("Failed to log wipe device error response: {}", e);
                    }
                    
                    Err(error)
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to wipe device {}: {}", device_id, e);
            let error = format!("Failed to wipe device: {}", e);
            
            // Log the error response
            let response_data = serde_json::json!({
                "error": error,
                "operation": "wipe_device"
            });
            
            if let Err(log_err) = log_device_response(&device_id, &request_id, false, &response_data, Some(&error)).await {
                eprintln!("Failed to log wipe device error response: {}", log_err);
            }
            
            Err(error)
        }
    }
}

/// Set device label
#[tauri::command]
pub async fn set_device_label(
    device_id: String,
    label: String,
    queue_manager: State<'_, DeviceQueueManager>,
) -> Result<(), String> {
    println!("Setting device label for {}: '{}'", device_id, label);
    
    let request_id = uuid::Uuid::new_v4().to_string();
    
    // Log the request
    let request_data = serde_json::json!({
        "device_id": device_id,
        "label": label,
        "operation": "set_device_label"
    });
    
    if let Err(e) = log_device_request(&device_id, &request_id, "SetDeviceLabel", &request_data).await {
        eprintln!("Failed to log set device label request: {}", e);
    }
    
    // Validate label (max 32 chars for KeepKey)
    if label.len() > 32 {
        let error = "Label must be 32 characters or less".to_string();
        
        // Log the validation error
        let response_data = serde_json::json!({
            "error": error,
            "operation": "set_device_label"
        });
        
        if let Err(e) = log_device_response(&device_id, &request_id, false, &response_data, Some(&error)).await {
            eprintln!("Failed to log set device label validation error: {}", e);
        }
        
        return Err(error);
    }
    
    if !label.chars().all(|c| c.is_ascii() && !c.is_control()) {
        let error = "Label must contain only ASCII printable characters".to_string();
        
        // Log the validation error
        let response_data = serde_json::json!({
            "error": error,
            "operation": "set_device_label"
        });
        
        if let Err(e) = log_device_response(&device_id, &request_id, false, &response_data, Some(&error)).await {
            eprintln!("Failed to log set device label validation error: {}", e);
        }
        
        return Err(error);
    }
    
    // Get or create device queue handle
    let queue_handle = {
        let mut manager = queue_manager.lock().await;
        
        if let Some(handle) = manager.get(&device_id) {
            handle.clone()
        } else {
            // Find the device by ID
            let devices = keepkey_rust::features::list_connected_devices();
            let device_info = devices
                .iter()
                .find(|d| d.unique_id == device_id);
                
            match device_info {
                Some(device_info) => {
                    // Spawn a new device worker
                    let handle = DeviceQueueFactory::spawn_worker(device_id.clone(), device_info.clone());
                    manager.insert(device_id.clone(), handle.clone());
                    handle
                }
                None => {
                    let error = format!("Device {} not found", device_id);
                    
                    // Log the error response
                    let response_data = serde_json::json!({
                        "error": error,
                        "operation": "set_device_label"
                    });
                    
                    if let Err(e) = log_device_response(&device_id, &request_id, false, &response_data, Some(&error)).await {
                        eprintln!("Failed to log set device label error response: {}", e);
                    }
                    
                    return Err(error);
                }
            }
        }
    };
    
    // Create ApplySettings message with the label
    let apply_settings = keepkey_rust::messages::Message::ApplySettings(
        keepkey_rust::messages::ApplySettings {
            language: None,
            label: Some(label.clone()),
            use_passphrase: None,
            auto_lock_delay_ms: None,
            u2f_counter: None,
        }
    );
    
    // Log the raw message being sent
    let message_data = serde_json::json!({
        "message_type": "ApplySettings",
        "message": {
            "label": label
        }
    });
    
    if let Err(e) = log_raw_device_message(&device_id, "SEND", "ApplySettings", &message_data).await {
        eprintln!("Failed to log apply settings raw message: {}", e);
    }
    
    // Send label update through queue
    match queue_handle.send_raw(apply_settings, true).await {
        Ok(response) => {
            // Log the raw response
            let response_message_data = serde_json::json!({
                "response": format!("{:?}", response)
            });
            
            if let Err(e) = log_raw_device_message(&device_id, "RECEIVE", "ApplySettingsResponse", &response_message_data).await {
                eprintln!("Failed to log apply settings raw response: {}", e);
            }
            
            match response {
                keepkey_rust::messages::Message::Success(_) => {
                    println!("✅ Device label set successfully for {}: '{}'", device_id, label);
                    
                    // Log the successful response
                    let response_data = serde_json::json!({
                        "success": true,
                        "label": label,
                        "operation": "set_device_label"
                    });
                    
                    if let Err(e) = log_device_response(&device_id, &request_id, true, &response_data, None).await {
                        eprintln!("Failed to log set device label response: {}", e);
                    }
                    
                    Ok(())
                }
                keepkey_rust::messages::Message::Failure(failure) => {
                    let error = format!("Device rejected label change: {}", failure.message.unwrap_or_default());
                    println!("❌ Failed to set device label for {}: {}", device_id, error);
                    
                    // Log the error response
                    let response_data = serde_json::json!({
                        "error": error,
                        "operation": "set_device_label"
                    });
                    
                    if let Err(e) = log_device_response(&device_id, &request_id, false, &response_data, Some(&error)).await {
                        eprintln!("Failed to log set device label error response: {}", e);
                    }
                    
                    Err(error)
                }
                _ => {
                    let error = "Unexpected response from device".to_string();
                    println!("❌ Failed to set device label for {}: {}", device_id, error);
                    
                    // Log the error response
                    let response_data = serde_json::json!({
                        "error": error,
                        "operation": "set_device_label"
                    });
                    
                    if let Err(e) = log_device_response(&device_id, &request_id, false, &response_data, Some(&error)).await {
                        eprintln!("Failed to log set device label error response: {}", e);
                    }
                    
                    Err(error)
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to set device label for {}: {}", device_id, e);
            let error = format!("Failed to set device label: {}", e);
            
            // Log the error response
            let response_data = serde_json::json!({
                "error": error,
                "operation": "set_device_label"
            });
            
            if let Err(log_err) = log_device_response(&device_id, &request_id, false, &response_data, Some(&error)).await {
                eprintln!("Failed to log set device label error response: {}", log_err);
            }
            
            Err(error)
        }
    }
}

/// Enhanced get_connected_devices that fetches features through the queue
#[tauri::command]
pub async fn get_connected_devices_with_features(
    queue_manager: State<'_, DeviceQueueManager>,
) -> Result<Vec<serde_json::Value>, String> {
    let devices = keepkey_rust::features::list_connected_devices();
    
    let request_id = uuid::Uuid::new_v4().to_string();
    
    // Log the request
    let request_data = serde_json::json!({
        "operation": "get_connected_devices_with_features",
        "device_count": devices.len()
    });
    
    if let Err(e) = log_device_request("all", &request_id, "GetConnectedDevicesWithFeatures", &request_data).await {
        eprintln!("Failed to log get connected devices request: {}", e);
    }
    
    // Process devices in parallel to fetch features
    let mut tasks = Vec::new();
    
    for device in devices.into_iter().filter(|device| device.is_keepkey) {
        let device_id = device.unique_id.clone();
        let queue_manager = queue_manager.inner().clone();
        
        let task = tokio::spawn(async move {
            // Log individual device feature request
            let device_request_id = uuid::Uuid::new_v4().to_string();
            let device_request_data = serde_json::json!({
                "device_id": device_id,
                "operation": "get_features_for_device"
            });
            
            if let Err(e) = log_device_request(&device_id, &device_request_id, "GetFeaturesForDevice", &device_request_data).await {
                eprintln!("Failed to log device features request: {}", e);
            }
            
            // Get or create device queue handle
            let queue_handle = {
                let mut manager = queue_manager.lock().await;
                
                if let Some(handle) = manager.get(&device_id) {
                    handle.clone()
                } else {
                    // Spawn a new device worker
                    let handle = DeviceQueueFactory::spawn_worker(device_id.clone(), device.clone());
                    manager.insert(device_id.clone(), handle.clone());
                    handle
                }
            };
            
            // Try to fetch features through the queue
            let features = match tokio::time::timeout(
                std::time::Duration::from_secs(DEVICE_OPERATION_TIMEOUT_SECS),
                queue_handle.get_features()
            ).await {
                Ok(Ok(raw_features)) => {
                    // Convert from raw Features message to DeviceFeatures
                    let device_features = convert_features_to_device_features(raw_features);
                    
                    // Log successful feature retrieval
                    let device_response_data = serde_json::json!({
                        "features": device_features,
                        "operation": "get_features_for_device"
                    });
                    
                    if let Err(e) = log_device_response(&device_id, &device_request_id, true, &device_response_data, None).await {
                        eprintln!("Failed to log device features response: {}", e);
                    }
                    
                    Some(device_features)
                }
                Ok(Err(e)) => {
                    println!("Failed to get features for device {}: {}", device_id, e);
                    
                    // Log failed feature retrieval
                    let device_response_data = serde_json::json!({
                        "error": format!("Failed to get features: {}", e),
                        "operation": "get_features_for_device"
                    });
                    
                    if let Err(log_err) = log_device_response(&device_id, &device_request_id, false, &device_response_data, Some(&format!("Failed to get features: {}", e))).await {
                        eprintln!("Failed to log device features error response: {}", log_err);
                    }
                    
                    None
                }
                Err(_) => {
                    println!("Timeout getting features for device {}", device_id);
                    
                    // Log timeout
                    let device_response_data = serde_json::json!({
                        "error": "Timeout getting features",
                        "operation": "get_features_for_device"
                    });
                    
                    if let Err(e) = log_device_response(&device_id, &device_request_id, false, &device_response_data, Some("Timeout getting features")).await {
                        eprintln!("Failed to log device features timeout response: {}", e);
                    }
                    
                    None
                }
            };
            
            serde_json::json!({
                "device": {
                    "unique_id": device.unique_id,
                    "name": device.name,
                    "vid": device.vid,
                    "pid": device.pid,
                    "manufacturer": device.manufacturer,
                    "product": device.product,
                    "serial_number": device.serial_number,
                    "is_keepkey": device.is_keepkey,
                },
                "features": features,
            })
        });
        
        tasks.push(task);
    }
    
    // Wait for all tasks to complete
    let mut results = Vec::new();
    for task in tasks {
        match task.await {
            Ok(result) => results.push(result),
            Err(e) => {
                println!("Task failed: {}", e);
                // Continue with other devices
            }
        }
    }
    
    // Log the overall response
    let response_data = serde_json::json!({
        "devices": results,
        "operation": "get_connected_devices_with_features"
    });
    
    if let Err(e) = log_device_response("all", &request_id, true, &response_data, None).await {
        eprintln!("Failed to log get connected devices response: {}", e);
    }
    
    Ok(results)
}

/// Evaluate device status to determine what actions are needed
pub fn evaluate_device_status(device_id: String, features: Option<&DeviceFeatures>) -> DeviceStatus {
    let mut status = DeviceStatus {
        device_id: device_id.clone(),
        connected: true,
        features: features.cloned(),
        needs_bootloader_update: false,
        needs_firmware_update: false,
        needs_initialization: false,
        needs_pin_unlock: false,
        bootloader_check: None,
        firmware_check: None,
        initialization_check: None,
    };
    
    if let Some(features) = features {
        let latest_bootloader_version = "2.1.4".to_string(); // Fixed: 2.1.4 is the actual latest official bootloader
        
        // CRITICAL FIX: Check bootloader version regardless of current mode
        // For OOB devices, we can infer bootloader version from firmware version
        let current_bootloader_version = if features.bootloader_mode {
            // Device is in bootloader mode - use the firmware version as bootloader version
            if features.version.starts_with("1.") {
                features.version.clone() // OOB bootloader versions like 1.0.3
            } else {
                // Modern bootloader in bootloader mode - use version directly
                features.version.clone()
            }
        } else {
            // Device is in normal firmware mode - check if it's an OOB device
            if features.version.starts_with("1.0.") {
                // OOB device: firmware version 1.0.3 = bootloader version 1.0.3
                features.version.clone()
            } else if let Some(ref bl_version) = features.bootloader_version {
                // Use explicit bootloader version if available
                bl_version.clone()
            } else {
                // For modern firmware without explicit bootloader version, assume it's recent enough
                "2.1.4".to_string() // Assume recent bootloader if not specified
            }
        };
        
        // Check if bootloader needs update using proper semantic version comparison
        let needs_bootloader_update = if features.bootloader_mode {
            // For devices in bootloader mode, only update bootloader if it's truly old (like 1.x)
            // Modern bootloaders (2.1.4) don't need bootloader updates - they need firmware updates
            current_bootloader_version.starts_with("1.")
        } else if current_bootloader_version == "Unknown bootloader" {
            false // Can't determine, assume no update needed
        } else {
            match semver::Version::parse(&current_bootloader_version) {
                Ok(current_ver) => {
                    if let Ok(latest_ver) = semver::Version::parse(&latest_bootloader_version) {
                        current_ver < latest_ver
                    } else {
                        // Fallback to string comparison if parsing fails
                        current_bootloader_version != latest_bootloader_version && 
                        current_bootloader_version != "2.1.4"
                    }
                }
                Err(_) => {
                    // For versions like "1.0.3", definitely needs update
                    !current_bootloader_version.starts_with("2.1.")
                }
            }
        };
        
        println!("🔧 Bootloader check: {} -> needs update: {} (bootloader_mode: {})", 
                current_bootloader_version, needs_bootloader_update, features.bootloader_mode);
        
        // Set bootloader status regardless of current mode
        if current_bootloader_version != "Unknown bootloader" {
            status.bootloader_check = Some(BootloaderCheck {
                current_version: current_bootloader_version,
                latest_version: latest_bootloader_version,
                needs_update: needs_bootloader_update,
            });
            status.needs_bootloader_update = needs_bootloader_update;
        }
        
        // Check firmware version 
        let current_version = features.version.clone();
        let latest_version = "7.10.0".to_string(); // Latest firmware version
        
        let needs_firmware_update = if features.bootloader_mode {
            // CRITICAL: Devices in bootloader mode need firmware updates to get out of bootloader mode
            // Only exception is if they have an old bootloader (1.x) that needs bootloader update first
            !current_version.starts_with("1.")
        } else {
            // Normal mode - check if firmware needs update
            if current_version.starts_with("1.0.") {
                // OOB device - firmware update only after bootloader update
                false // Bootloader has higher priority
            } else {
                !current_version.starts_with("7.10.")
            }
        };
        
        status.firmware_check = Some(FirmwareCheck {
            current_version: current_version.clone(),
            latest_version: latest_version.clone(),
            needs_update: needs_firmware_update,
        });
        status.needs_firmware_update = needs_firmware_update;
        
        println!("🔧 Firmware check: {} vs {} -> needs update: {} (bootloader_mode: {})", 
                current_version, latest_version, needs_firmware_update, features.bootloader_mode);
        
        // Check initialization status
        if !features.bootloader_mode {
            // Normal mode - can check initialization status directly
            let initialized = features.initialized;
            let has_backup = !features.no_backup;
            let imported = features.imported.unwrap_or(false);
            
            // Check if device is unlocked (PIN cached or no PIN protection)
            let has_pin_protection = features.pin_protection;
            let pin_cached = features.pin_cached;
            let is_pin_locked = initialized && has_pin_protection && !pin_cached;
            
            // Device needs setup if it has never been initialized OR if the user never created a recovery backup
            let needs_setup = !initialized || !has_backup;
            
            status.initialization_check = Some(InitializationCheck {
                initialized,
                has_backup,
                imported,
                needs_setup,
            });
            
            // IMPORTANT: Only mark as needing initialization if it truly needs setup
            // A device that is initialized but locked with PIN should NOT be marked as needing initialization
            status.needs_initialization = needs_setup;
            status.needs_pin_unlock = is_pin_locked;
            
            println!("🔧 Initialization check: initialized={}, needs_setup={}, has_pin_protection={}, pin_cached={}", 
                    initialized, needs_setup, has_pin_protection, pin_cached);
            
            if is_pin_locked {
                println!("🔒 Device is initialized but locked with PIN - needs unlock (NOT initialization)");
            }
        } else {
            // Device is in bootloader mode - initialization status unknown
            // For OOB bootloaders (version 1.x), assume they'll need initialization after bootloader update
            let is_oob_bootloader = features.version.starts_with("1.");
            
            status.initialization_check = Some(InitializationCheck {
                initialized: false, // Unknown, but assume not for OOB
                has_backup: false,  // Unknown
                imported: false,    // Unknown
                needs_setup: is_oob_bootloader, // OOB devices will likely need setup after update
            });
            
            // Don't set needs_initialization=true during bootloader mode
            // The bootloader update takes priority, then we'll re-evaluate after device reboots
            status.needs_initialization = false;
            status.needs_pin_unlock = false; // PIN unlock not applicable in bootloader mode
        }
    } else {
        // No features available - device not communicating
        status.connected = false;
        status.needs_pin_unlock = false; // Can't determine PIN status if device not communicating
    }
    
    status
}

/// Convert raw Features message to DeviceFeatures
pub fn convert_features_to_device_features(raw_features: keepkey_rust::messages::Features) -> DeviceFeatures {
    DeviceFeatures {
        label: raw_features.label,
        vendor: raw_features.vendor,
        model: raw_features.model,
        firmware_variant: raw_features.firmware_variant,
        device_id: raw_features.device_id,
        language: raw_features.language,
        bootloader_mode: raw_features.bootloader_mode.unwrap_or(false),
        version: format!(
            "{}.{}.{}",
            raw_features.major_version.unwrap_or(0),
            raw_features.minor_version.unwrap_or(0),
            raw_features.patch_version.unwrap_or(0)
        ),
        firmware_hash: raw_features.firmware_hash.map(hex::encode),
        bootloader_hash: raw_features.bootloader_hash.clone().map(hex::encode),
        bootloader_version: None, // TODO: Implement proper hash-to-version mapping if needed
        initialized: raw_features.initialized.unwrap_or(false),
        imported: raw_features.imported,
        no_backup: raw_features.no_backup.unwrap_or(false),
        pin_protection: raw_features.pin_protection.unwrap_or(false),
        pin_cached: raw_features.pin_cached.unwrap_or(false),
        passphrase_protection: raw_features.passphrase_protection.unwrap_or(false),
        passphrase_cached: raw_features.passphrase_cached.unwrap_or(false),
        wipe_code_protection: raw_features.wipe_code_protection.unwrap_or(false),
        auto_lock_delay_ms: raw_features.auto_lock_delay_ms.map(|ms| ms as u64),
        policies: raw_features
            .policies
            .into_iter()
            .filter(|p| p.enabled())
            .map(|p| p.policy_name().to_string())
            .collect(),
    }
}

/// Get the path to today's device communication log file
#[tauri::command]
pub async fn get_device_log_path() -> Result<String, String> {
    let logger = crate::logging::get_device_logger();
    let log_path = logger.get_todays_log_path();
    
    Ok(log_path.to_string_lossy().to_string())
}

/// Get recent device communication log entries (last N entries)
#[tauri::command]
pub async fn get_recent_device_logs(limit: Option<usize>) -> Result<Vec<serde_json::Value>, String> {
    let logger = crate::logging::get_device_logger();
    let log_path = logger.get_todays_log_path();
    let limit = limit.unwrap_or(50); // Default to last 50 entries
    
    if !log_path.exists() {
        return Ok(vec![]);
    }
    
    // Read the log file and parse JSON lines
    let content = std::fs::read_to_string(&log_path)
        .map_err(|e| format!("Failed to read log file: {}", e))?;
    
    let mut entries: Vec<serde_json::Value> = content
        .lines()
        .filter_map(|line| {
            if line.trim().is_empty() {
                return None;
            }
            match serde_json::from_str(line) {
                Ok(json) => Some(json),
                Err(e) => {
                    eprintln!("Failed to parse log line: {} - Error: {}", line, e);
                    None
                }
            }
        })
        .collect();
    
    // Return the last N entries
    if entries.len() > limit {
        let skip_count = entries.len() - limit;
        entries = entries.into_iter().skip(skip_count).collect();
    }
    
    Ok(entries)
}

/// Clear old device communication logs (manually trigger cleanup)
#[tauri::command]
pub async fn cleanup_device_logs() -> Result<String, String> {
    let logger = crate::logging::get_device_logger();
    logger.cleanup_old_logs().await?;
    Ok("Old device logs cleaned up successfully".to_string())
}

/// Parse transaction from hex string
/// Returns (metadata, inputs, outputs) where metadata is (version, input_count, output_count, lock_time)
pub fn parse_transaction_from_hex(hex_data: &str) -> Result<((u32, u32, u32, u32), Vec<keepkey_rust::messages::TxInputType>, Vec<keepkey_rust::messages::TxOutputBinType>), String> {
    let tx_bytes = hex::decode(hex_data).map_err(|e| format!("Invalid hex: {}", e))?;
    let mut cursor = Cursor::new(tx_bytes);
    
    // Parse version (4 bytes, little-endian)
    let version = read_u32_le(&mut cursor)?;
    
    // Parse input count (varint)
    let input_count = read_varint(&mut cursor)?;
    
    // Parse inputs
    let mut inputs = Vec::new();
    for _ in 0..input_count {
        // Previous output hash (32 bytes, needs to be reversed)
        let mut prev_hash = vec![0u8; 32];
        read_exact(&mut cursor, &mut prev_hash)?;
        prev_hash.reverse(); // Bitcoin uses little-endian for display but big-endian for hashing
        
        // Previous output index (4 bytes, little-endian)
        let prev_index = read_u32_le(&mut cursor)?;
        
        // Script length (varint)
        let script_len = read_varint(&mut cursor)? as usize;
        
        // Script
        let mut script_sig = vec![0u8; script_len];
        read_exact(&mut cursor, &mut script_sig)?;
        
        // Sequence (4 bytes, little-endian)
        let sequence = read_u32_le(&mut cursor)?;
        
        inputs.push(keepkey_rust::messages::TxInputType {
            address_n: vec![], // Will be filled by caller
            prev_hash,
            prev_index,
            script_sig: if script_sig.is_empty() { None } else { Some(script_sig) },
            sequence: Some(sequence),
            script_type: None, // Will be filled by caller
            multisig: None,
            amount: None, // Will be filled by caller
            decred_tree: None,
            decred_script_version: None,
        });
    }
    
    // Parse output count (varint)
    let output_count = read_varint(&mut cursor)?;
    
    // Parse outputs
    let mut outputs = Vec::new();
    for _ in 0..output_count {
        // Value (8 bytes, little-endian)
        let amount = read_u64_le(&mut cursor)?;
        
        // Script length (varint)
        let script_len = read_varint(&mut cursor)? as usize;
        
        // Script
        let mut script_pubkey = vec![0u8; script_len];
        read_exact(&mut cursor, &mut script_pubkey)?;
        
        outputs.push(keepkey_rust::messages::TxOutputBinType {
            amount,
            script_pubkey,
            decred_script_version: None,
        });
    }
    
    // Parse lock time (4 bytes, little-endian)
    let lock_time = read_u32_le(&mut cursor)?;
    
    Ok(((version, input_count as u32, output_count as u32, lock_time), inputs, outputs))
}

fn read_u32_le(cursor: &mut Cursor<Vec<u8>>) -> Result<u32, String> {
    let mut buf = [0u8; 4];
    read_exact(cursor, &mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64_le(cursor: &mut Cursor<Vec<u8>>) -> Result<u64, String> {
    let mut buf = [0u8; 8];
    read_exact(cursor, &mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_varint(cursor: &mut Cursor<Vec<u8>>) -> Result<u64, String> {
    let mut buf = [0u8; 1];
    read_exact(cursor, &mut buf)?;
    let first_byte = buf[0];
    
    match first_byte {
        0..=252 => Ok(first_byte as u64),
        253 => {
            let mut buf = [0u8; 2];
            read_exact(cursor, &mut buf)?;
            Ok(u16::from_le_bytes(buf) as u64)
        }
        254 => {
            let mut buf = [0u8; 4];
            read_exact(cursor, &mut buf)?;
            Ok(u32::from_le_bytes(buf) as u64)
        }
        255 => {
            let mut buf = [0u8; 8];
            read_exact(cursor, &mut buf)?;
            Ok(u64::from_le_bytes(buf))
        }
    }
}

fn read_exact(cursor: &mut Cursor<Vec<u8>>, buf: &mut [u8]) -> Result<(), String> {
    use std::io::Read;
    cursor.read_exact(buf).map_err(|e| format!("Failed to read data: {}", e))
}

/// Test status event emission
#[tauri::command]
pub async fn test_status_emission(app: tauri::AppHandle) -> Result<String, String> {
    println!("📡 Test command: emitting test status...");
    let test_payload = serde_json::json!({
        "status": "Test message from backend"
    });
    println!("📡 Test payload: {}", test_payload);
    
    if let Err(e) = app.emit("status:update", test_payload) {
        println!("❌ Failed to emit test status: {}", e);
        Err(format!("Failed to emit test status: {}", e))
    } else {
        println!("✅ Successfully emitted test status");
        Ok("Test status emitted successfully".to_string())
    }
}

/// Signal that the frontend is ready to receive events
#[tauri::command]
pub async fn frontend_ready(app: AppHandle) -> Result<(), String> {
    println!("🎯 Frontend ready signal received - enabling event emission");
    
    let mut state = FRONTEND_READY_STATE.write().await;
    state.is_ready = true;
    
    // Flush any queued events
    if !state.queued_events.is_empty() {
        println!("📦 Flushing {} queued events to frontend", state.queued_events.len());
        
        for event in state.queued_events.drain(..) {
            println!("📡 Sending queued event: {} (queued at: {})", event.event_name, event.timestamp);
            if let Err(e) = app.emit(&event.event_name, &event.payload) {
                println!("❌ Failed to emit queued event {}: {}", event.event_name, e);
            }
        }
        
        println!("✅ All queued events have been sent to frontend");
    } else {
        println!("✅ No queued events to flush");
    }
    
    Ok(())
}

/// Helper function to emit events (either immediately or queue them)
pub async fn emit_or_queue_event(app: &AppHandle, event_name: &str, payload: serde_json::Value) -> Result<(), String> {
    let state = FRONTEND_READY_STATE.read().await;
    
    if state.is_ready {
        // Frontend is ready, emit immediately
        app.emit(event_name, &payload)
            .map_err(|e| format!("Failed to emit event {}: {}", event_name, e))?;
        println!("📡 Emitted event: {}", event_name);
    } else {
        // Frontend not ready, queue the event
        drop(state); // Release read lock
        let mut state = FRONTEND_READY_STATE.write().await;
        
        let queued_event = QueuedEvent {
            event_name: event_name.to_string(),
            payload,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        
        state.queued_events.push(queued_event);
        println!("📋 Queued event: {} (total queued: {})", event_name, state.queued_events.len());
    }
    
    Ok(())
}

/// Get the config directory path
fn get_config_dir() -> Result<PathBuf, String> {
    let home_dir = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "Could not find home directory")?;
    
    let config_dir = PathBuf::from(home_dir).join(".keepkey");
    
    // Create directory if it doesn't exist
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    
    Ok(config_dir)
}

/// Get the config file path
fn get_config_file_path() -> Result<PathBuf, String> {
    let config_dir = get_config_dir()?;
    Ok(config_dir.join("keepkey.json"))
}

/// Load configuration from file
fn load_config() -> Result<serde_json::Value, String> {
    let config_path = get_config_file_path()?;
    
    if !config_path.exists() {
        // Return default config if file doesn't exist
        return Ok(serde_json::json!({
            "language": "en",
            "isOnboarded": false,
            "theme": "dark",
            "notifications": true
        }));
    }
    
    let config_str = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    
    serde_json::from_str(&config_str)
        .map_err(|e| format!("Failed to parse config file: {}", e))
}

/// Save configuration to file
fn save_config(config: &serde_json::Value) -> Result<(), String> {
    let config_path = get_config_file_path()?;
    
    let config_str = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    fs::write(&config_path, config_str)
        .map_err(|e| format!("Failed to write config file: {}", e))?;
    
    Ok(())
}

/// Check if this is the first time install
#[tauri::command]
pub async fn is_first_time_install() -> Result<bool, String> {
    let config = load_config()?;
    let is_onboarded = config.get("isOnboarded")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    
    Ok(!is_onboarded)
}

/// Check if user is onboarded
#[tauri::command]
pub async fn is_onboarded() -> Result<bool, String> {
    let config = load_config()?;
    let is_onboarded = config.get("isOnboarded")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    
    Ok(is_onboarded)
}

/// Mark onboarding as completed
#[tauri::command]
pub async fn set_onboarding_completed() -> Result<(), String> {
    let mut config = load_config()?;
    
    if let Some(obj) = config.as_object_mut() {
        obj.insert("isOnboarded".to_string(), serde_json::Value::Bool(true));
    }
    
    save_config(&config)?;
    println!("Onboarding marked as completed");
    Ok(())
}

/// Get a preference value
#[tauri::command]
pub async fn get_preference(key: String) -> Result<Option<String>, String> {
    let config = load_config()?;
    
    let value = config.get(&key)
        .and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Bool(b) => Some(b.to_string()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        });
    
    Ok(value)
}

/// Set a preference value
#[tauri::command]
pub async fn set_preference(key: String, value: String) -> Result<(), String> {
    let mut config = load_config()?;
    
    if let Some(obj) = config.as_object_mut() {
        // Try to parse as different types
        let parsed_value = if value == "true" || value == "false" {
            serde_json::Value::Bool(value == "true")
        } else if let Ok(num) = value.parse::<i64>() {
            serde_json::Value::Number(serde_json::Number::from(num))
        } else {
            serde_json::Value::String(value)
        };
        
        obj.insert(key, parsed_value);
    }
    
    save_config(&config)?;
    Ok(())
}

/// Debug onboarding state
#[tauri::command]
pub async fn debug_onboarding_state() -> Result<String, String> {
    let config = load_config()?;
    Ok(format!("Config: {}", serde_json::to_string_pretty(&config).unwrap_or_else(|_| "Unable to serialize".to_string())))
}

/// Restart the application
#[tauri::command]
pub async fn restart_app(app: tauri::AppHandle) -> Result<(), String> {
    log::info!("Restarting application...");
    app.restart();
    // Note: app.restart() never returns, it exits the current process
}

/// Get API enable status
#[tauri::command]
pub async fn get_api_enabled() -> Result<bool, String> {
    log::debug!("Getting API enabled status");
    let config = load_config()?;
    let enabled = config.get("api_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true); // Default to true (enabled) if not set
    log::debug!("API enabled status: {}", enabled);
    Ok(enabled)
}

/// Set API enable status
#[tauri::command]
pub async fn set_api_enabled(enabled: bool) -> Result<(), String> {
    log::info!("Setting API enabled status: {}", enabled);
    let mut config = load_config()?;
    
    if let Some(obj) = config.as_object_mut() {
        obj.insert("api_enabled".to_string(), serde_json::Value::Bool(enabled));
    }
    
    save_config(&config)?;
    log::info!("API enabled status saved: {}", enabled);
    Ok(())
}

/// Get API status (running or not)
#[tauri::command]
pub async fn get_api_status() -> Result<serde_json::Value, String> {
    log::debug!("Getting API status");
    let config = load_config()?;
    let enabled = config.get("api_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    
    // Check if server is actually running by trying to connect to it
    let is_running = if enabled {
        // Simple check - try to connect to the port
        match std::net::TcpStream::connect_timeout(
            &"127.0.0.1:1646".parse().unwrap(),
            std::time::Duration::from_millis(100)
        ) {
            Ok(_) => true,
            Err(_) => false,
        }
    } else {
        false
    };
    
    let status = serde_json::json!({
        "enabled": enabled,
        "running": is_running,
        "port": 1646,
        "endpoints": {
            "rest_docs": "http://127.0.0.1:1646/docs",
            "mcp": "http://127.0.0.1:1646/mcp"
        }
    });
    
    log::debug!("API status: {}", status);
    Ok(status)
}

// Bootloader and firmware update functions have been moved to device/updates.rs for better organization

// PIN Creation Flow Types and Commands

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PinCreationSession {
    pub device_id: String,
    pub session_id: String,
    pub current_step: PinStep,
    pub is_active: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum PinStep {
    AwaitingFirst,   // Waiting for first PIN entry
    AwaitingSecond,  // Waiting for PIN confirmation
    AwaitingUnlock,  // Waiting for PIN unlock entry
    Completed,       // PIN creation/unlock done
    Failed,          // PIN creation/unlock failed
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PinMatrixResult {
    pub success: bool,
    pub next_step: Option<String>,
    pub session_id: String,
    pub error: Option<String>,
}

lazy_static::lazy_static! {
    static ref PIN_SESSIONS: Arc<std::sync::Mutex<std::collections::HashMap<String, PinCreationSession>>> =
        Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    static ref DEVICE_PIN_FLOWS: Arc<std::sync::Mutex<std::collections::HashSet<String>>> =
        Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
}

/// Start PIN creation process by initiating ResetDevice with PIN protection
#[tauri::command]
pub async fn initialize_device_pin(
    device_id: String, 
    label: Option<String>,
    queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<PinCreationSession, String> {
    log::info!("Starting PIN creation for device: {} with label: {:?}", device_id, label);
    
    // Check if device is already in PIN flow
    if is_device_in_pin_flow(&device_id) {
        return Err("Device is already in PIN creation flow".to_string());
    }
    
    // Mark device as in PIN flow BEFORE starting any operations
    mark_device_in_pin_flow(&device_id)?;
    
    // Generate unique session ID
    let session_id = format!("pin_session_{}_{}", device_id, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    
    // Create PIN session
    let session = PinCreationSession {
        device_id: device_id.clone(),
        session_id: session_id.clone(),
        current_step: PinStep::AwaitingFirst,
        is_active: true,
    };
    
    // Store session
    {
        let mut sessions = PIN_SESSIONS.lock().map_err(|_| "Failed to lock PIN sessions".to_string())?;
        sessions.insert(session_id.clone(), session.clone());
    }
    
    // Get or create device queue handle
    let queue_handle = {
        let mut manager = queue_manager.lock().await;
        
        if let Some(handle) = manager.get(&device_id) {
            handle.clone()
        } else {
            // Find the device by ID
            let devices = keepkey_rust::features::list_connected_devices();
            let device_info = devices
                .iter()
                .find(|d| d.unique_id == device_id)
                .ok_or_else(|| {
                    // Clean up session on device not found
                    let mut sessions = PIN_SESSIONS.lock().unwrap_or_else(|_| panic!("Failed to lock PIN sessions"));
                    sessions.remove(&session_id);
                    format!("Device {} not found", device_id)
                })?;
            
            // Spawn a new device worker
            let handle = keepkey_rust::device_queue::DeviceQueueFactory::spawn_worker(device_id.clone(), device_info.clone());
            manager.insert(device_id.clone(), handle.clone());
            handle
        }
    };
    
    // Create ResetDevice message with PIN protection enabled
    let reset_device = keepkey_rust::messages::ResetDevice {
        display_random: Some(false),  // Don't show confusing entropy screen to users
        strength: Some(128),  // 128 bits = 12 words (Bitcoin-only)
        passphrase_protection: Some(false),
        pin_protection: Some(true),  // This triggers PIN creation flow
        language: Some("english".to_string()),
        label: label.map(|l| l.to_string()),
        no_backup: Some(false),
        auto_lock_delay_ms: None,
        u2f_counter: None,
    };
    
    // Send ResetDevice message to actual device - THIS SHOULD TRIGGER PIN MATRIX ON DEVICE SCREEN
    match queue_handle.send_raw(keepkey_rust::messages::Message::ResetDevice(reset_device), false).await {
        Ok(response) => {
            log::info!("✅ ResetDevice sent successfully, device responded with: {:?}", response);
            
            // Handle the response - should be PinMatrixRequest
            match response {
                keepkey_rust::messages::Message::PinMatrixRequest(pmr) => {
                    log::info!("🎯 Device requesting PIN matrix input, type: {:?}", pmr.r#type);
                    // Device is ready for PIN input - return session to frontend
                    Ok(session)
                }
                keepkey_rust::messages::Message::Success(_) => {
                    log::info!("Device reset completed without PIN request");
                    // Mark as completed
                    if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                        if let Some(session) = sessions.get_mut(&session_id) {
                            session.current_step = PinStep::Completed;
                            session.is_active = false;
                        }
                    }
                    Ok(session)
                }
                other => {
                    log::warn!("Unexpected response from ResetDevice: {:?}", other);
                    // Return session anyway - device might be ready for PIN
                    Ok(session)
                }
            }
        }
        Err(e) => {
            log::error!("Failed to send ResetDevice message: {}", e);
            // Remove PIN session on failure
            let mut sessions = PIN_SESSIONS.lock().map_err(|_| "Failed to lock PIN sessions".to_string())?;
            sessions.remove(&session_id);
            // Unmark device from PIN flow on failure
            let _ = unmark_device_in_pin_flow(&device_id);
            Err(format!("Failed to start PIN creation: {}", e))
        }
    }
}

/// Send PIN matrix response (positions clicked by user)
#[tauri::command]
pub async fn send_pin_matrix_response(
    session_id: String,
    positions: Vec<u8>,  // Positions 1-9 that user clicked
    queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<PinMatrixResult, String> {
    log::info!("Sending PIN matrix response for session: {} with {} positions", session_id, positions.len());
    
    // Validate positions
    if positions.is_empty() || positions.len() > 9 {
        log::error!("Invalid PIN length: {} positions", positions.len());
        return Err("PIN must be between 1 and 9 digits".to_string());
    }
    
    for &pos in &positions {
        if pos < 1 || pos > 9 {
            log::error!("Invalid PIN position: {}", pos);
            return Err("Invalid PIN position: positions must be 1-9".to_string());
        }
    }
    
    log::info!("✅ PIN positions validated: {:?}", positions);
    
    // Get session data (release lock before async call)
    let (device_id, current_step) = {
        let mut sessions = PIN_SESSIONS.lock().map_err(|_| "Failed to lock PIN sessions".to_string())?;
        let session = sessions.get_mut(&session_id)
            .ok_or_else(|| format!("PIN session not found: {}", session_id))?;
        
        if !session.is_active {
            return Err("PIN session is not active".to_string());
        }
        
        (session.device_id.clone(), session.current_step.clone())
    };
    
    // Get device queue handle
    let queue_handle = {
        let manager = queue_manager.lock().await;
        manager.get(&device_id)
            .ok_or_else(|| format!("Device queue not found for device: {}", device_id))?
            .clone()
    };
    
    // Convert positions to PIN string for device protocol (positions as characters)
    let pin_string: String = positions.iter()
        .map(|&pos| (b'0' + pos) as char)
        .collect();
    
    log::info!("🔢 Converted {} positions {:?} to PIN string: '{}'", positions.len(), positions, pin_string);
    
    // Additional validation - ensure PIN string is not empty
    if pin_string.is_empty() {
        log::error!("❌ PIN string is empty after conversion!");
        return Err("PIN string conversion failed - empty result".to_string());
    }
    
    // Create PinMatrixAck message
    let pin_matrix_ack = keepkey_rust::messages::PinMatrixAck {
        pin: pin_string.clone(),
    };
    
    // Send message to device
    match queue_handle.send_raw(keepkey_rust::messages::Message::PinMatrixAck(pin_matrix_ack), false).await {
        Ok(response) => {
            log::info!("✅ PinMatrixAck sent successfully: {:?}", response);
            
            // Analyze response to determine next step
            match current_step {
                PinStep::AwaitingUnlock => {
                    // PIN unlock - check if device is now unlocked
                    match response {
                        keepkey_rust::messages::Message::Features(features) => {
                            let pin_cached = features.pin_cached.unwrap_or(false);
                            if pin_cached {
                                log::info!("✅ PIN unlock successful, device is now unlocked");
                                
                                // Update session state to completed
                                if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                    if let Some(session) = sessions.get_mut(&session_id) {
                                        session.current_step = PinStep::Completed;
                                        session.is_active = false;
                                    }
                                }
                                // Unmark device from PIN flow - PIN unlock completed
                                let _ = unmark_device_in_pin_flow(&device_id);
                                
                                Ok(PinMatrixResult {
                                    success: true,
                                    next_step: Some("unlocked".to_string()),
                                    session_id: session_id.clone(),
                                    error: None,
                                })
                            } else {
                                log::error!("❌ PIN unlock failed - device still locked");
                                
                                // Update session state to failed
                                if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                    if let Some(session) = sessions.get_mut(&session_id) {
                                        session.current_step = PinStep::Failed;
                                        session.is_active = false;
                                    }
                                }
                                // Unmark device from PIN flow on failure
                                let _ = unmark_device_in_pin_flow(&device_id);
                                
                                Err("PIN unlock failed - incorrect PIN".to_string())
                            }
                        }
                        keepkey_rust::messages::Message::Failure(f) => {
                            log::error!("❌ PIN unlock failed: {}", f.message.as_deref().unwrap_or("Unknown error"));
                            
                            // Update session state to failed  
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Failed;
                                    session.is_active = false;
                                }
                            }
                            // Unmark device from PIN flow on failure
                            let _ = unmark_device_in_pin_flow(&device_id);
                            
                            Err(format!("PIN unlock failed: {}", f.message.as_deref().unwrap_or("Unknown error")))
                        }
                        _ => {
                            log::error!("❌ Unexpected response to PIN unlock: {:?}", response);
                            
                            // Update session state to failed
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Failed;
                                    session.is_active = false;
                                }
                            }
                            // Unmark device from PIN flow on failure
                            let _ = unmark_device_in_pin_flow(&device_id);
                            
                            Err("Unexpected response from device during PIN unlock".to_string())
                        }
                    }
                }
                PinStep::AwaitingFirst => {
                    // First PIN entry - check what device wants next
                    match response {
                        keepkey_rust::messages::Message::PinMatrixRequest(pmr) => {
                            match pmr.r#type {
                                Some(3) => {  // NewSecond = 3 (PIN confirmation)
                                    log::info!("✅ First PIN accepted, device requesting confirmation");
                                    // Update session state
                                    if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                        if let Some(session) = sessions.get_mut(&session_id) {
                                            session.current_step = PinStep::AwaitingSecond;
                                        }
                                    }
                                    
                                    Ok(PinMatrixResult {
                                        success: true,
                                        next_step: Some("confirm".to_string()),
                                        session_id: session_id.clone(),
                                        error: None,
                                    })
                                }
                                _ => {
                                    log::warn!("Unexpected PIN matrix request type: {:?}", pmr.r#type);
                                    // Update session state
                                    if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                        if let Some(session) = sessions.get_mut(&session_id) {
                                            session.current_step = PinStep::AwaitingSecond;
                                        }
                                    }
                                    Ok(PinMatrixResult {
                                        success: true,
                                        next_step: Some("confirm".to_string()),
                                        session_id: session_id.clone(),
                                        error: None,
                                    })
                                }
                            }
                        }
                        keepkey_rust::messages::Message::EntropyRequest(_) | 
                        keepkey_rust::messages::Message::Success(_) => {
                            log::info!("✅ PIN creation completed in single step");
                            // Update session state
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Completed;
                                    session.is_active = false;
                                }
                            }
                            // Unmark device from PIN flow - PIN creation completed
                            let _ = unmark_device_in_pin_flow(&device_id);
                            
                            Ok(PinMatrixResult {
                                success: true,
                                next_step: Some("complete".to_string()),
                                session_id: session_id.clone(),
                                error: None,
                            })
                        }
                        keepkey_rust::messages::Message::Failure(f) => {
                            // Update session state
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Failed;
                                    session.is_active = false;
                                }
                            }
                            // Unmark device from PIN flow on failure
                            let _ = unmark_device_in_pin_flow(&device_id);
                            Err(format!("PIN creation failed: {}", f.message.unwrap_or_default()))
                        }
                        _ => {
                            log::warn!("Unexpected response to first PIN: {:?}", response);
                            // Update session state
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::AwaitingSecond;
                                }
                            }
                            Ok(PinMatrixResult {
                                success: true,
                                next_step: Some("confirm".to_string()),
                                session_id: session_id.clone(),
                                error: None,
                            })
                        }
                    }
                }
                PinStep::AwaitingSecond => {
                    // PIN confirmation - expect completion or error
                    match response {
                        keepkey_rust::messages::Message::EntropyRequest(_) => {
                            log::info!("✅ PIN confirmation accepted, device requesting entropy (handled automatically)");
                            // Update session state to completed
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Completed;
                                    session.is_active = false;
                                }
                            }
                            // Unmark device from PIN flow - PIN creation completed
                            let _ = unmark_device_in_pin_flow(&device_id);
                            
                            Ok(PinMatrixResult {
                                success: true,
                                next_step: Some("complete".to_string()),
                                session_id: session_id.clone(),
                                error: None,
                            })
                        }
                        keepkey_rust::messages::Message::Success(_) => {
                            log::info!("✅ PIN confirmation accepted, device initialization completed");
                            // Update session state
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Completed;
                                    session.is_active = false;
                                }
                            }
                            // Unmark device from PIN flow - PIN creation completed
                            let _ = unmark_device_in_pin_flow(&device_id);
                            
                            Ok(PinMatrixResult {
                                success: true,
                                next_step: Some("complete".to_string()),
                                session_id: session_id.clone(),
                                error: None,
                            })
                        }
                        keepkey_rust::messages::Message::Failure(f) => {
                            // Update session state
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Failed;
                                    session.is_active = false;
                                }
                            }
                            // Unmark device from PIN flow on failure
                            let _ = unmark_device_in_pin_flow(&device_id);
                            Err(format!("PIN confirmation failed: {}", f.message.unwrap_or_default()))
                        }
                        _ => {
                            log::warn!("Unexpected response during PIN confirmation: {:?}", response);
                            // Update session state
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Completed;
                                    session.is_active = false;
                                }
                            }
                            // Unmark device from PIN flow - assuming completion
                            let _ = unmark_device_in_pin_flow(&device_id);
                            
                            Ok(PinMatrixResult {
                                success: true,
                                next_step: Some("complete".to_string()),
                                session_id: session_id.clone(),
                                error: None,
                            })
                        }
                    }
                }
                PinStep::Completed => {
                    Err("PIN session already completed".to_string())
                }
                PinStep::Failed => {
                    Err("PIN session failed".to_string())
                }
            }
        }
        Err(e) => {
            log::error!("Failed to send PIN matrix response: {}", e);
            // Update session state
            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                if let Some(session) = sessions.get_mut(&session_id) {
                    session.current_step = PinStep::Failed;
                    session.is_active = false;
                }
            }
            // Unmark device from PIN flow on communication error
            let _ = unmark_device_in_pin_flow(&device_id);
            Err(format!("Failed to send PIN to device: {}", e))
        }
    }
}

/// Start PIN unlock process for a device that is locked
#[tauri::command]
pub async fn start_pin_unlock(
    device_id: String,
    _queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<PinCreationSession, String> {
    log::info!("Starting PIN unlock for device: {}", device_id);
    
    // Check if device is already in PIN flow
    if is_device_in_pin_flow(&device_id) {
        return Err("Device is already in PIN flow".to_string());
    }
    
    // Mark device as in PIN flow BEFORE starting any operations
    mark_device_in_pin_flow(&device_id)?;
    
    // Generate unique session ID
    let session_id = format!("pin_unlock_{}_{}", device_id, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    
    // Create PIN unlock session
    let session = PinCreationSession {
        device_id: device_id.clone(),
        session_id: session_id.clone(),
        current_step: PinStep::AwaitingUnlock,
        is_active: true,
    };
    
    // Store session
    {
        let mut sessions = PIN_SESSIONS.lock().map_err(|_| "Failed to lock PIN sessions".to_string())?;
        sessions.insert(session_id.clone(), session.clone());
    }
    
    log::info!("PIN unlock session created: {}", session_id);
    Ok(session)
}

/// Send PIN unlock response (for unlocking an already initialized device)
#[tauri::command]
pub async fn send_pin_unlock_response(
    session_id: String,
    positions: Vec<u8>,  // Positions 1-9 that user clicked
    queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<PinMatrixResult, String> {
    log::info!("Sending PIN unlock response for session: {} with {} positions", session_id, positions.len());
    
    // Validate positions
    if positions.is_empty() || positions.len() > 9 {
        return Err("PIN must be between 1 and 9 digits".to_string());
    }
    
    for &pos in &positions {
        if pos < 1 || pos > 9 {
            return Err("Invalid PIN position: positions must be 1-9".to_string());
        }
    }
    
    // Get session data (release lock before async call)
    let device_id = {
        let mut sessions = PIN_SESSIONS.lock().map_err(|_| "Failed to lock PIN sessions".to_string())?;
        let session = sessions.get_mut(&session_id)
            .ok_or_else(|| format!("PIN session not found: {}", session_id))?;
        
        if !session.is_active {
            return Err("PIN session is not active".to_string());
        }
        
        if session.current_step != PinStep::AwaitingUnlock {
            return Err("PIN session is not awaiting unlock".to_string());
        }
        
        session.device_id.clone()
    };
    
    // Convert positions to PIN string for device protocol (positions as characters)
    let pin_string: String = positions.iter()
        .map(|&pos| (b'0' + pos) as char)
        .collect();
    
    log::info!("Converted positions to PIN string for device communication: {}", pin_string);
    
    // Get or create device queue handle  
    let queue_handle = {
        let mut manager = queue_manager.lock().await;
        match manager.get(&device_id) {
            Some(handle) => handle.clone(),
            None => {
                // Find the device by ID
                let devices = keepkey_rust::features::list_connected_devices();
                let device_info = devices
                    .iter()
                    .find(|d| d.unique_id == device_id)
                    .ok_or_else(|| format!("Device {} not found", device_id))?;
                
                // Spawn a new device worker using the factory
                let handle = keepkey_rust::device_queue::DeviceQueueFactory::spawn_worker(
                    device_id.clone(),
                    device_info.clone()
                );
                manager.insert(device_id.clone(), handle.clone());
                handle
            }
        }
    };
    
    // Try a simple GetFeatures with PIN to unlock device
    let get_features = keepkey_rust::messages::Message::GetFeatures(
        keepkey_rust::messages::GetFeatures {}
    );
    
    match queue_handle.send_raw(get_features, false).await {
        Ok(response) => {
            match response {
                keepkey_rust::messages::Message::PinMatrixRequest(_) => {
                    // Device wants PIN, send our PIN response
                    let pin_matrix_ack = keepkey_rust::messages::Message::PinMatrixAck(
                        keepkey_rust::messages::PinMatrixAck {
                            pin: pin_string.clone(),
                        }
                    );
                    
                    match queue_handle.send_raw(pin_matrix_ack, false).await {
                        Ok(features_response) => {
                            match features_response {
                                keepkey_rust::messages::Message::Features(features) => {
                                    // Check if PIN was accepted (device should now be unlocked)
                                    let pin_cached = features.pin_cached.unwrap_or(false);
                                    if pin_cached {
                                        log::info!("✅ PIN unlock successful, device is now unlocked");
                                        
                                        // Update session state to completed
                                        if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                            if let Some(session) = sessions.get_mut(&session_id) {
                                                session.current_step = PinStep::Completed;
                                                session.is_active = false;
                                            }
                                        }
                                        // Unmark device from PIN flow - PIN unlock completed
                                        let _ = unmark_device_in_pin_flow(&device_id);
                                        
                                        Ok(PinMatrixResult {
                                            success: true,
                                            next_step: Some("unlocked".to_string()),
                                            session_id: session_id.clone(),
                                            error: None,
                                        })
                                    } else {
                                        log::error!("❌ PIN unlock failed - device still locked");
                                        
                                        // Update session state to failed
                                        if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                            if let Some(session) = sessions.get_mut(&session_id) {
                                                session.current_step = PinStep::Failed;
                                                session.is_active = false;
                                            }
                                        }
                                        // Unmark device from PIN flow on failure
                                        let _ = unmark_device_in_pin_flow(&device_id);
                                        
                                        Err("PIN unlock failed - incorrect PIN".to_string())
                                    }
                                }
                                keepkey_rust::messages::Message::Failure(f) => {
                                    log::error!("❌ PIN unlock failed: {}", f.message.as_deref().unwrap_or("Unknown error"));
                                    
                                    // Update session state to failed
                                    if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                        if let Some(session) = sessions.get_mut(&session_id) {
                                            session.current_step = PinStep::Failed;
                                            session.is_active = false;
                                        }
                                    }
                                    // Unmark device from PIN flow on failure
                                    let _ = unmark_device_in_pin_flow(&device_id);
                                    
                                    Err(format!("PIN unlock failed: {}", f.message.as_deref().unwrap_or("Unknown error")))
                                }
                                _ => {
                                    log::error!("❌ Unexpected response to PIN unlock: {:?}", features_response);
                                    
                                    // Update session state to failed
                                    if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                        if let Some(session) = sessions.get_mut(&session_id) {
                                            session.current_step = PinStep::Failed;
                                            session.is_active = false;
                                        }
                                    }
                                    // Unmark device from PIN flow on failure
                                    let _ = unmark_device_in_pin_flow(&device_id);
                                    
                                    Err("Unexpected response from device during PIN unlock".to_string())
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to send PIN to device: {}", e);
                            
                            // Update session state to failed
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Failed;
                                    session.is_active = false;
                                }
                            }
                            // Unmark device from PIN flow on failure
                            let _ = unmark_device_in_pin_flow(&device_id);
                            
                            Err(format!("Failed to send PIN to device: {}", e))
                        }
                    }
                }
                keepkey_rust::messages::Message::Features(features) => {
                    // Device responded with features without asking for PIN - already unlocked
                    let pin_cached = features.pin_cached.unwrap_or(false);
                    if pin_cached {
                        log::info!("✅ Device is already unlocked");
                        
                        // Update session state to completed
                        if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                            if let Some(session) = sessions.get_mut(&session_id) {
                                session.current_step = PinStep::Completed;
                                session.is_active = false;
                            }
                        }
                        // Unmark device from PIN flow - already unlocked
                        let _ = unmark_device_in_pin_flow(&device_id);
                        
                        Ok(PinMatrixResult {
                            success: true,
                            next_step: Some("already_unlocked".to_string()),
                            session_id: session_id.clone(),
                            error: None,
                        })
                    } else {
                        log::error!("❌ Device claims no PIN protection but is not unlocked");
                        
                        // Update session state to failed
                        if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                            if let Some(session) = sessions.get_mut(&session_id) {
                                session.current_step = PinStep::Failed;
                                session.is_active = false;
                            }
                        }
                        // Unmark device from PIN flow on failure  
                        let _ = unmark_device_in_pin_flow(&device_id);
                        
                        Err("Device state inconsistent - no PIN protection but not unlocked".to_string())
                    }
                }
                _ => {
                    log::error!("❌ Unexpected initial response from device: {:?}", response);
                    
                    // Update session state to failed
                    if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                        if let Some(session) = sessions.get_mut(&session_id) {
                            session.current_step = PinStep::Failed;
                            session.is_active = false;
                        }
                    }
                    // Unmark device from PIN flow on failure
                    let _ = unmark_device_in_pin_flow(&device_id);
                    
                    Err("Unexpected response from device during PIN unlock initialization".to_string())
                }
            }
        }
        Err(e) => {
            log::error!("Failed to communicate with device for PIN unlock: {}", e);
            
            // Update session state to failed
            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                if let Some(session) = sessions.get_mut(&session_id) {
                    session.current_step = PinStep::Failed;  
                    session.is_active = false;
                }
            }
            // Unmark device from PIN flow on failure
            let _ = unmark_device_in_pin_flow(&device_id);
            
            Err(format!("Failed to communicate with device: {}", e))
        }
    }
}

/// Get PIN creation session status
#[tauri::command]
pub async fn get_pin_session_status(session_id: String) -> Result<Option<PinCreationSession>, String> {
    let sessions = PIN_SESSIONS.lock().map_err(|_| "Failed to lock PIN sessions".to_string())?;
    Ok(sessions.get(&session_id).cloned())
}

/// Cancel PIN creation session
#[tauri::command]
pub async fn cancel_pin_creation(session_id: String) -> Result<bool, String> {
    log::info!("Cancelling PIN creation session: {}", session_id);
    
    let mut sessions = PIN_SESSIONS.lock().map_err(|_| "Failed to lock PIN sessions".to_string())?;
    if let Some(session) = sessions.get_mut(&session_id) {
        let device_id = session.device_id.clone();
        session.is_active = false;
        session.current_step = PinStep::Failed;
        
        // Unmark device from PIN flow when cancelled
        let _ = unmark_device_in_pin_flow(&device_id);
        
        log::info!("PIN creation session cancelled for device: {}", device_id);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Initialize/reset device to create new wallet
#[tauri::command]
pub async fn initialize_device_wallet(device_id: String, label: String) -> Result<(), String> {
    log::info!("Initializing wallet on device: {} with label: '{}'", device_id, label);
    
    // TODO: Implement actual device reset/initialization via device queue
    // This should:
    // 1. Reset the device to factory state
    // 2. Generate new seed
    // 3. Set the device label
    // 4. Initialize the device
    
    // Simulate device communication delay for reset operation
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    
    log::info!("Device wallet initialized successfully");
    Ok(())
}

/// Complete wallet creation (mark as initialized)
#[tauri::command]
pub async fn complete_wallet_creation(device_id: String) -> Result<(), String> {
    log::info!("Completing wallet creation for device: {}", device_id);
    
    // TODO: Implement final wallet setup steps
    // This should:
    // 1. Finalize device configuration
    // 2. Update device registry
    // 3. Mark device as ready for use
    
    // Simulate final setup delay
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    
    log::info!("Wallet creation completed successfully");
    Ok(())
}

// ========== Device Flow State Management ==========

/// Mark device as being in PIN flow to prevent duplicate operations
pub fn mark_device_in_pin_flow(device_id: &str) -> Result<(), String> {
    let mut flows = DEVICE_PIN_FLOWS.lock().map_err(|_| "Failed to lock device PIN flows".to_string())?;
    flows.insert(device_id.to_string());
    log::info!("Device {} marked as in PIN flow", device_id);
    Ok(())
}

/// Check if device is currently in PIN flow
pub fn is_device_in_pin_flow(device_id: &str) -> bool {
    if let Ok(flows) = DEVICE_PIN_FLOWS.lock() {
        flows.contains(device_id)
    } else {
        false
    }
}

/// Remove device from PIN flow state
pub fn unmark_device_in_pin_flow(device_id: &str) -> Result<(), String> {
    let mut flows = DEVICE_PIN_FLOWS.lock().map_err(|_| "Failed to lock device PIN flows".to_string())?;
    flows.remove(device_id);
    log::info!("Device {} removed from PIN flow", device_id);
    Ok(())
}

// ========== Recovery Commands (Direct Implementation) ==========

use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecoverySession {
    pub session_id: String,
    pub device_id: String,
    pub word_count: u32,
    pub current_word: u32,
    pub current_character: u32,
    pub is_active: bool,
    pub passphrase_protection: bool,
    pub label: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RecoveryAction {
    Space,     // Move to next word
    Done,      // Complete recovery  
    Delete,    // Backspace
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecoveryProgress {
    pub word_pos: u32,
    pub character_pos: u32,
    pub auto_completed: bool,
    pub is_complete: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecoveryStatus {
    pub session: RecoverySession,
    pub is_waiting_for_input: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SeedVerificationSession {
    pub session_id: String,
    pub device_id: String,
    pub word_count: u32,
    pub current_word: u32,
    pub current_character: u32,
    pub is_active: bool,
    pub pin_verified: bool,
}

// Global recovery sessions
lazy_static::lazy_static! {
    static ref RECOVERY_SESSIONS: Mutex<HashMap<String, RecoverySession>> = 
        Mutex::new(HashMap::new());
    static ref VERIFICATION_SESSIONS: Mutex<HashMap<String, SeedVerificationSession>> = 
        Mutex::new(HashMap::new());
    static ref RECOVERY_DEVICE_FLOWS: Mutex<std::collections::HashSet<String>> =
        Mutex::new(std::collections::HashSet::new());
    static ref RECOVERY_DEVICE_ALIASES: Mutex<HashMap<String, String>> = 
        Mutex::new(HashMap::new());
}

/// Start device recovery process
#[tauri::command]
pub async fn start_device_recovery(
    device_id: String,
    word_count: u32,
    passphrase_protection: bool,
    label: String,
    queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<RecoverySession, String> {
    log::info!("Starting device recovery for device: {} with {} words", device_id, word_count);
    
    // Check if device is already in recovery flow to prevent double initialization
    if is_device_in_recovery_flow(&device_id) {
        log::warn!("Device {} is already in recovery flow, returning existing session", device_id);
        // Try to find existing session
        let sessions = RECOVERY_SESSIONS.lock()
            .map_err(|_| "Failed to lock recovery sessions".to_string())?;
        
        if let Some(existing_session) = sessions.values().find(|s| s.device_id == device_id && s.is_active) {
            return Ok(existing_session.clone());
        } else {
            log::warn!("Device marked as in recovery but no active session found, cleaning up");
            drop(sessions);
            let _ = unmark_device_in_recovery_flow(&device_id);
        }
    }
    
    // Validate word count
    if ![12, 18, 24].contains(&word_count) {
        return Err("Invalid word count. Must be 12, 18, or 24".to_string());
    }
    
    // Generate session ID
    let session_id = format!("recovery_{}_{}", 
        device_id, 
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    
    // Create recovery session
    let session = RecoverySession {
        session_id: session_id.clone(),
        device_id: device_id.clone(),
        word_count,
        current_word: 0,
        current_character: 0,
        is_active: true,
        passphrase_protection,
        label: label.clone(),
    };
    
    // Store session
    {
        let mut sessions = RECOVERY_SESSIONS.lock()
            .map_err(|_| "Failed to lock recovery sessions".to_string())?;
        sessions.insert(session_id.clone(), session.clone());
    }
    
    // Mark device as being in recovery flow
    mark_device_in_recovery_flow(&device_id)?;
    
    // Get or create device queue handle
    let queue_handle = {
        let mut manager = queue_manager.lock().await;
        
        if let Some(handle) = manager.get(&device_id) {
            handle.clone()
        } else {
            // Find the device by ID
            let devices = keepkey_rust::features::list_connected_devices();
            let device_info = devices
                .iter()
                .find(|d| d.unique_id == device_id)
                .ok_or_else(|| {
                    // Clean up session on device not found
                    let mut sessions = RECOVERY_SESSIONS.lock().unwrap_or_else(|_| panic!("Failed to lock recovery sessions"));
                    sessions.remove(&session_id);
                    format!("Device {} not found", device_id)
                })?;
            
            // Spawn a new device worker
            let handle = keepkey_rust::device_queue::DeviceQueueFactory::spawn_worker(device_id.clone(), device_info.clone());
            manager.insert(device_id.clone(), handle.clone());
            handle
        }
    };
    
    // Create RecoveryDevice message - minimal essential parameters only
    let recovery_device = keepkey_rust::messages::RecoveryDevice {
        word_count: Some(word_count),
        passphrase_protection: Some(passphrase_protection),
        pin_protection: Some(true),  // Always use PIN
        language: Some("english".to_string()),
        label: Some(label),
        enforce_wordlist: None,      // Don't set - might not be supported
        use_character_cipher: Some(true),  // Re-enable - device expects substitution cipher
        auto_lock_delay_ms: None,    // Don't set - might not be supported
        u2f_counter: None,           // Don't set - not needed for recovery
        dry_run: Some(false),        // Essential - distinguishes from verification
    };
    
    // Send RecoveryDevice message through device queue
    match queue_handle.send_raw(keepkey_rust::messages::Message::RecoveryDevice(recovery_device), false).await {
        Ok(response) => {
            log::info!("RecoveryDevice sent, response: {:?}", response);
            
            match response {
                keepkey_rust::messages::Message::PinMatrixRequest(_) => {
                    // Expected - device wants PIN setup
                    log::info!("Device requesting PIN setup for recovery");
                    Ok(session)
                }
                keepkey_rust::messages::Message::CharacterRequest(req) => {
                    // Device might skip PIN if already set
                    log::info!("Device ready for character input: word {}, char {}", 
                        req.word_pos, req.character_pos);
                    // Update session state
                    if let Ok(mut sessions) = RECOVERY_SESSIONS.lock() {
                        if let Some(s) = sessions.get_mut(&session_id) {
                            s.current_word = req.word_pos;
                            s.current_character = req.character_pos;
                        }
                    }
                    Ok(session)
                }
                keepkey_rust::messages::Message::ButtonRequest(_) => {
                    // Device needs user confirmation
                    log::info!("Device requesting button press for recovery");
                    Ok(session)
                }
                keepkey_rust::messages::Message::Failure(f) => {
                    // Clean up session only on actual device failure
                    if let Ok(mut sessions) = RECOVERY_SESSIONS.lock() {
                        sessions.remove(&session_id);
                    }
                    let _ = unmark_device_in_recovery_flow(&device_id);
                    Err(format!("Device rejected recovery: {}", f.message.unwrap_or_default()))
                }
                _ => {
                    log::warn!("Unexpected response to RecoveryDevice: {:?}", response);
                    Ok(session)
                }
            }
        }
        Err(e) => {
            // Don't immediately clean up - this might be a transport error that can be retried
            log::error!("Failed to send RecoveryDevice, but keeping session active for potential retry: {}", e);
            Err(format!("Failed to start recovery: {}", e))
        }
    }
}

/// Send recovery character input
#[tauri::command]
pub async fn send_recovery_character(
    session_id: String,
    character: Option<String>,
    action: Option<RecoveryAction>,
    queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<RecoveryProgress, String> {
    log::info!("Sending recovery character for session: {} - char: {:?}, action: {:?}", 
        session_id, character, action);
    
    // Get session
    let (device_id, current_word, current_char) = {
        let sessions = RECOVERY_SESSIONS.lock()
            .map_err(|_| "Failed to lock recovery sessions".to_string())?;
        
        let session = sessions.get(&session_id)
            .ok_or_else(|| "Recovery session not found".to_string())?;
        
        if !session.is_active {
            return Err("Recovery session is not active".to_string());
        }
        
        (session.device_id.clone(), session.current_word, session.current_character)
    };
    
    // Resolve canonical device ID in case the device reconnected with a different ID
    let canonical_device_id = get_canonical_device_id(&device_id);
    log::info!("Using canonical device ID: {} (original: {})", canonical_device_id, device_id);
    
    // Get device queue handle
    let queue_handle = {
        let manager = queue_manager.lock().await;
        
        // Try canonical ID first, then original ID
        manager.get(&canonical_device_id)
            .or_else(|| manager.get(&device_id))
            .ok_or_else(|| format!("Device queue not found for device: {} (canonical: {})", device_id, canonical_device_id))?
            .clone()
    };
    
    // Create CharacterAck message
    let character_ack = match action {
        Some(RecoveryAction::Done) => {
            keepkey_rust::messages::CharacterAck {
                character: None,
                delete: Some(false),
                done: Some(true),
            }
        }
        Some(RecoveryAction::Delete) => {
            keepkey_rust::messages::CharacterAck {
                character: None,
                delete: Some(true),
                done: Some(false),
            }
        }
        Some(RecoveryAction::Space) => {
            keepkey_rust::messages::CharacterAck {
                character: Some(" ".to_string()),
                delete: Some(false),
                done: Some(false),
            }
        }
        None => {
            // Regular character input
            if let Some(ch) = character {
                // Validate character
                if ch.len() != 1 || !ch.chars().next().unwrap().is_alphabetic() {
                    return Err("Invalid character. Must be a single letter a-z".to_string());
                }
                
                keepkey_rust::messages::CharacterAck {
                    character: Some(ch.to_lowercase()),
                    delete: Some(false),
                    done: Some(false),
                }
            } else {
                return Err("No character or action provided".to_string());
            }
        }
    };
    
    match queue_handle.send_raw(keepkey_rust::messages::Message::CharacterAck(character_ack), false).await {
        Ok(response) => {
            match response {
                keepkey_rust::messages::Message::CharacterRequest(req) => {
                    // Update session state
                    if let Ok(mut sessions) = RECOVERY_SESSIONS.lock() {
                        if let Some(session) = sessions.get_mut(&session_id) {
                            session.current_word = req.word_pos;
                            session.current_character = req.character_pos;
                        }
                    }
                    
                    Ok(RecoveryProgress {
                        word_pos: req.word_pos,
                        character_pos: req.character_pos,
                        auto_completed: false,
                        is_complete: false,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Success(_) => {
                    // Recovery completed successfully
                    if let Ok(mut sessions) = RECOVERY_SESSIONS.lock() {
                        if let Some(session) = sessions.get_mut(&session_id) {
                            session.is_active = false;
                        }
                    }
                    
                    // Remove from recovery flow
                    let _ = unmark_device_in_recovery_flow(&device_id);
                    
                    Ok(RecoveryProgress {
                        word_pos: current_word,
                        character_pos: current_char,
                        auto_completed: false,
                        is_complete: true,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Failure(f) => {
                    // Mark session as failed
                    if let Ok(mut sessions) = RECOVERY_SESSIONS.lock() {
                        if let Some(session) = sessions.get_mut(&session_id) {
                            session.is_active = false;
                        }
                    }
                    
                    // Remove from recovery flow
                    let _ = unmark_device_in_recovery_flow(&device_id);
                    
                    Err(format!("Recovery failed: {}", f.message.unwrap_or_default()))
                }
                _ => {
                    Err(format!("Unexpected response: {:?}", response))
                }
            }
        }
        Err(e) => {
            Err(format!("Failed to send character: {}", e))
        }
    }
}

/// Send PIN matrix response during recovery flow
#[tauri::command]
pub async fn send_recovery_pin_response(
    session_id: String,
    positions: Vec<u8>,
    queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<RecoveryProgress, String> {
    log::info!("Sending recovery PIN for session: {} with {} positions", session_id, positions.len());
    
    // Validate positions
    if positions.is_empty() || positions.len() > 9 {
        return Err("PIN must be between 1 and 9 digits".to_string());
    }
    
    for &pos in &positions {
        if pos < 1 || pos > 9 {
            return Err("Invalid PIN position: positions must be 1-9".to_string());
        }
    }
    
    // Get session data
    let (device_id, current_word, current_char) = {
        let sessions = RECOVERY_SESSIONS.lock()
            .map_err(|_| "Failed to lock recovery sessions".to_string())?;
        
        let session = sessions.get(&session_id)
            .ok_or_else(|| "Recovery session not found".to_string())?;
        
        if !session.is_active {
            return Err("Recovery session is not active".to_string());
        }
        
        (session.device_id.clone(), session.current_word, session.current_character)
    };
    
    // Resolve canonical device ID in case the device reconnected with a different ID
    let canonical_device_id = get_canonical_device_id(&device_id);
    log::info!("Using canonical device ID: {} (original: {})", canonical_device_id, device_id);
    
    // Get device queue handle
    let queue_handle = {
        let manager = queue_manager.lock().await;
        
        // Try canonical ID first, then original ID
        manager.get(&canonical_device_id)
            .or_else(|| manager.get(&device_id))
            .ok_or_else(|| format!("Device queue not found for device: {} (canonical: {})", device_id, canonical_device_id))?
            .clone()
    };
    
    // Convert positions to PIN string for device protocol
    let pin_string: String = positions.iter()
        .map(|&pos| (b'0' + pos) as char)
        .collect();
    
    log::info!("Converted positions to PIN string for recovery PIN: {}", pin_string);
    
    // Create PinMatrixAck message
    let pin_matrix_ack = keepkey_rust::messages::PinMatrixAck {
        pin: pin_string.clone(),
    };
    
    // Send message to device
    match queue_handle.send_raw(keepkey_rust::messages::Message::PinMatrixAck(pin_matrix_ack), false).await {
        Ok(response) => {
            log::info!("Recovery PIN sent successfully: {:?}", response);
            
            match response {
                keepkey_rust::messages::Message::PinMatrixRequest(_) => {
                    // Device wants PIN confirmation
                    Ok(RecoveryProgress {
                        word_pos: current_word,
                        character_pos: current_char,
                        auto_completed: false,
                        is_complete: false,
                        error: Some("pin_confirm".to_string()), // Special signal for PIN confirmation
                    })
                }
                keepkey_rust::messages::Message::ButtonRequest(_) => {
                    // Device needs button confirmation
                    Ok(RecoveryProgress {
                        word_pos: current_word,
                        character_pos: current_char,
                        auto_completed: false,
                        is_complete: false,
                        error: Some("button_confirm".to_string()), // Special signal for button confirmation
                    })
                }
                keepkey_rust::messages::Message::CharacterRequest(req) => {
                    // Ready for character input
                    if let Ok(mut sessions) = RECOVERY_SESSIONS.lock() {
                        if let Some(session) = sessions.get_mut(&session_id) {
                            session.current_word = req.word_pos;
                            session.current_character = req.character_pos;
                        }
                    }
                    
                    Ok(RecoveryProgress {
                        word_pos: req.word_pos,
                        character_pos: req.character_pos,
                        auto_completed: false,
                        is_complete: false,
                        error: Some("phrase_entry".to_string()), // Special signal for phrase entry
                    })
                }
                keepkey_rust::messages::Message::Success(_) => {
                    // Recovery completed
                    if let Ok(mut sessions) = RECOVERY_SESSIONS.lock() {
                        if let Some(session) = sessions.get_mut(&session_id) {
                            session.is_active = false;
                        }
                    }
                    
                    let _ = unmark_device_in_recovery_flow(&device_id);
                    
                    Ok(RecoveryProgress {
                        word_pos: current_word,
                        character_pos: current_char,
                        auto_completed: false,
                        is_complete: true,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Failure(f) => {
                    Err(format!("Recovery PIN failed: {}", f.message.unwrap_or_default()))
                }
                _ => {
                    Err(format!("Unexpected response to recovery PIN: {:?}", response))
                }
            }
        }
        Err(e) => {
            Err(format!("Failed to send recovery PIN: {}", e))
        }
    }
}

/// Get recovery session status
#[tauri::command]
pub async fn get_recovery_status(session_id: String) -> Result<Option<RecoveryStatus>, String> {
    let sessions = RECOVERY_SESSIONS.lock()
        .map_err(|_| "Failed to lock recovery sessions".to_string())?;
    
    if let Some(session) = sessions.get(&session_id) {
        Ok(Some(RecoveryStatus {
            session: session.clone(),
            is_waiting_for_input: true,  // TODO: Track actual device state
            error: None,
        }))
    } else {
        Ok(None)
    }
}

/// Cancel recovery session
#[tauri::command]
pub async fn cancel_recovery_session(
    session_id: String,
    queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<bool, String> {
    log::info!("Cancelling recovery session: {}", session_id);
    
    // Get device_id and remove session (drop lock immediately)
    let device_id_opt = {
        let mut sessions = RECOVERY_SESSIONS.lock()
            .map_err(|_| "Failed to lock recovery sessions".to_string())?;
        
        if let Some(mut session) = sessions.remove(&session_id) {
            session.is_active = false;
            Some(session.device_id.clone())
        } else {
            None
        }
    }; // Recovery sessions lock is dropped here
    
    let device_id = match device_id_opt {
        Some(id) => id,
        None => {
            log::warn!("Recovery session {} not found for cancellation", session_id);
            return Ok(false);
        }
    };
    
    // Get canonical device ID and queue handle (drop lock immediately)
    let queue_handle = {
        let manager = queue_manager.lock().await;
        let canonical_device_id = get_canonical_device_id(&device_id);
        manager.get(&canonical_device_id)
            .or_else(|| manager.get(&device_id))
            .cloned()
    }; // Queue manager lock is dropped here
    
    // Send Cancel message to device to exit recovery mode
    if let Some(handle) = queue_handle {
        log::info!("Sending Cancel message to device {} to exit recovery mode", device_id);
        let cancel_msg = keepkey_rust::messages::Cancel {};
        match handle.send_raw(keepkey_rust::messages::Message::Cancel(cancel_msg), false).await {
            Ok(response) => {
                log::info!("Cancel message sent successfully: {:?}", response);
            }
            Err(e) => {
                log::warn!("Failed to send Cancel message (device may already be out of recovery): {}", e);
                // Don't return error - cancellation should still succeed even if device communication fails
            }
        }
    } else {
        log::warn!("Device queue not found for {}, cannot send Cancel message", device_id);
    }
    
    // Remove from recovery flow
    let _ = unmark_device_in_recovery_flow(&device_id);
    
    log::info!("Recovery session {} cancelled successfully for device: {}", session_id, device_id);
    Ok(true)
}

// ========== Seed Verification Commands (Dry Run Recovery) ==========

/// Start seed verification process (dry run recovery)
#[tauri::command]
pub async fn start_seed_verification(
    device_id: String,
    word_count: u32,
    queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<SeedVerificationSession, String> {
    log::info!("Starting seed verification (dry run) for device: {} with {} words", device_id, word_count);
    
    // Check if device is already in recovery flow
    if is_device_in_recovery_flow(&device_id) {
        return Err("Device is already in recovery flow".to_string());
    }
    
    // Validate word count
    if ![12, 18, 24].contains(&word_count) {
        return Err("Invalid word count. Must be 12, 18, or 24".to_string());
    }
    
    // Generate session ID
    let session_id = format!("verify_{}_{}", 
        device_id, 
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    
    // Create verification session
    let session = SeedVerificationSession {
        session_id: session_id.clone(),
        device_id: device_id.clone(),
        word_count,
        current_word: 0,
        current_character: 0,
        is_active: true,
        pin_verified: false,
    };
    
    // Store session
    {
        let mut sessions = VERIFICATION_SESSIONS.lock()
            .map_err(|_| "Failed to lock verification sessions".to_string())?;
        sessions.insert(session_id.clone(), session.clone());
    }
    
    // Mark device as being in recovery flow (we reuse the same tracking)
    mark_device_in_recovery_flow(&device_id)?;
    
    // Get or create device queue handle
    let queue_handle = {
        let mut manager = queue_manager.lock().await;
        
        if let Some(handle) = manager.get(&device_id) {
            handle.clone()
        } else {
            // Find the device by ID
            let devices = keepkey_rust::features::list_connected_devices();
            let device_info = devices
                .iter()
                .find(|d| d.unique_id == device_id)
                .ok_or_else(|| {
                    // Clean up session on device not found
                    let mut sessions = VERIFICATION_SESSIONS.lock().unwrap_or_else(|_| panic!("Failed to lock verification sessions"));
                    sessions.remove(&session_id);
                    format!("Device {} not found", device_id)
                })?;
            
            // Spawn a new device worker
            let handle = keepkey_rust::device_queue::DeviceQueueFactory::spawn_worker(device_id.clone(), device_info.clone());
            manager.insert(device_id.clone(), handle.clone());
            handle
        }
    };
    
    // Create RecoveryDevice message with dry_run = true
    let recovery_device = keepkey_rust::messages::RecoveryDevice {
        word_count: Some(word_count),
        passphrase_protection: None,  // Don't change passphrase settings
        pin_protection: None,         // Don't change PIN settings
        language: Some("english".to_string()),
        label: None,                  // Don't change label
        enforce_wordlist: Some(true),
        use_character_cipher: Some(true),  // Use scrambled keyboard
        auto_lock_delay_ms: None,     // Don't change settings
        u2f_counter: None,            // Don't change settings
        dry_run: Some(true),          // THIS IS THE KEY - Dry run mode!
    };
    
    // Send RecoveryDevice message
    match queue_handle.send_raw(keepkey_rust::messages::Message::RecoveryDevice(recovery_device), false).await {
        Ok(response) => {
            log::info!("Dry run RecoveryDevice sent, response: {:?}", response);
            
            match response {
                keepkey_rust::messages::Message::PinMatrixRequest(_) => {
                    // Expected - device wants PIN verification first
                    log::info!("Device requesting PIN for seed verification");
                    Ok(session)
                }
                keepkey_rust::messages::Message::CharacterRequest(req) => {
                    // Device might skip PIN if session is already authenticated
                    log::info!("Device ready for character input (PIN already verified): word {}, char {}", 
                        req.word_pos, req.character_pos);
                    // Update session state
                    if let Ok(mut sessions) = VERIFICATION_SESSIONS.lock() {
                        if let Some(s) = sessions.get_mut(&session_id) {
                            s.current_word = req.word_pos;
                            s.current_character = req.character_pos;
                            s.pin_verified = true;
                        }
                    }
                    Ok(session)
                }
                keepkey_rust::messages::Message::Failure(f) => {
                    // Clean up on failure
                    if let Ok(mut sessions) = VERIFICATION_SESSIONS.lock() {
                        sessions.remove(&session_id);
                    }
                    let _ = unmark_device_in_recovery_flow(&device_id);
                    Err(format!("Device rejected seed verification: {}", f.message.unwrap_or_default()))
                }
                _ => {
                    log::warn!("Unexpected response to dry run RecoveryDevice: {:?}", response);
                    Ok(session)
                }
            }
        }
        Err(e) => {
            // Clean up on error
            if let Ok(mut sessions) = VERIFICATION_SESSIONS.lock() {
                sessions.remove(&session_id);
            }
            let _ = unmark_device_in_recovery_flow(&device_id);
            Err(format!("Failed to start seed verification: {}", e))
        }
    }
}

/// Send verification character input
#[tauri::command]
pub async fn send_verification_character(
    session_id: String,
    character: Option<String>,
    action: Option<RecoveryAction>,
    _queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<RecoveryProgress, String> {
    log::info!("Sending verification character for session: {} - char: {:?}, action: {:?}", 
        session_id, character, action);
    
    // Similar implementation to send_recovery_character but for verification sessions
    // Implementation would be similar to the recovery character function above
    // For brevity, I'll implement a simplified version
    
    Err("Verification character sending not yet implemented".to_string())
}

/// Send PIN matrix response during seed verification
#[tauri::command]
pub async fn send_verification_pin(
    session_id: String,
    positions: Vec<u8>,
    _queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<bool, String> {
    log::info!("Sending verification PIN for session: {} with {} positions", session_id, positions.len());
    
    // Similar implementation to send_recovery_pin_response but for verification sessions
    // For brevity, I'll implement a simplified version
    
    Err("Verification PIN sending not yet implemented".to_string())
}

/// Get seed verification status
#[tauri::command]
pub async fn get_verification_status(session_id: String) -> Result<Option<SeedVerificationSession>, String> {
    let sessions = VERIFICATION_SESSIONS.lock()
        .map_err(|_| "Failed to lock verification sessions".to_string())?;
    
    Ok(sessions.get(&session_id).cloned())
}

/// Cancel seed verification session
#[tauri::command]
pub async fn cancel_seed_verification(session_id: String) -> Result<bool, String> {
    log::info!("Cancelling seed verification session: {}", session_id);
    
    let mut sessions = VERIFICATION_SESSIONS.lock()
        .map_err(|_| "Failed to lock verification sessions".to_string())?;
    
    if let Some(session) = sessions.remove(&session_id) {
        let device_id = session.device_id.clone();
        
        // Remove from recovery flow
        let _ = unmark_device_in_recovery_flow(&device_id);
        
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Force cleanup seed verification
#[tauri::command]
pub async fn force_cleanup_seed_verification(device_id: String) -> Result<bool, String> {
    log::info!("Force cleaning up seed verification for device: {}", device_id);
    
    // Remove any verification sessions for this device
    let mut cleanup_done = false;
    {
        let mut sessions = VERIFICATION_SESSIONS.lock()
            .map_err(|_| "Failed to lock verification sessions".to_string())?;
        
        // Find and remove any sessions for this device
        let mut to_remove = Vec::new();
        for (session_id, session) in sessions.iter() {
            if session.device_id == device_id {
                to_remove.push(session_id.clone());
            }
        }
        
        for session_id in to_remove {
            sessions.remove(&session_id);
            cleanup_done = true;
            log::info!("Removed verification session: {}", session_id);
        }
    }
    
    // Force remove from recovery flow
    let _ = unmark_device_in_recovery_flow(&device_id);
    log::info!("Device {} removed from recovery flow", device_id);
    
    Ok(cleanup_done)
}

// ========== Recovery Flow State Management ==========

/// Mark device as being in recovery flow to prevent duplicate operations
pub fn mark_device_in_recovery_flow(device_id: &str) -> Result<(), String> {
    let mut flows = RECOVERY_DEVICE_FLOWS.lock().map_err(|_| "Failed to lock recovery device flows".to_string())?;
    flows.insert(device_id.to_string());
    log::info!("Device {} marked as in recovery flow", device_id);
    Ok(())
}

/// Check if device is currently in recovery flow
pub fn is_device_in_recovery_flow(device_id: &str) -> bool {
    if let Ok(flows) = RECOVERY_DEVICE_FLOWS.lock() {
        flows.contains(device_id)
    } else {
        false
    }
}

/// Remove device from recovery flow state
pub fn unmark_device_in_recovery_flow(device_id: &str) -> Result<(), String> {
    let mut flows = RECOVERY_DEVICE_FLOWS.lock().map_err(|_| "Failed to lock recovery device flows".to_string())?;
    flows.remove(device_id);
    log::info!("Device {} removed from recovery flow", device_id);
    
    // Also clean up any aliases
    if let Ok(mut aliases) = RECOVERY_DEVICE_ALIASES.lock() {
        aliases.retain(|_, v| v != device_id);
    }
    
    Ok(())
}

/// Add device ID alias for recovery flow
pub fn add_recovery_device_alias(alias_id: &str, canonical_id: &str) -> Result<(), String> {
    let mut aliases = RECOVERY_DEVICE_ALIASES.lock()
        .map_err(|_| "Failed to lock recovery device aliases".to_string())?;
    aliases.insert(alias_id.to_string(), canonical_id.to_string());
    log::info!("Added recovery device alias: {} -> {}", alias_id, canonical_id);
    Ok(())
}

/// Get canonical device ID from alias
pub fn get_canonical_device_id(device_id: &str) -> String {
    if let Ok(aliases) = RECOVERY_DEVICE_ALIASES.lock() {
        if let Some(canonical) = aliases.get(device_id) {
            log::info!("Resolved device alias {} to canonical ID {}", device_id, canonical);
            return canonical.clone();
        }
    }
    device_id.to_string()
}

/// Check if two device IDs might be the same device
pub fn are_devices_potentially_same(id1: &str, id2: &str) -> bool {
    // Check if they're already the same
    if id1 == id2 {
        return true;
    }
    
    // Check if one is an alias of the other
    let canonical1 = get_canonical_device_id(id1);
    let canonical2 = get_canonical_device_id(id2);
    if canonical1 == canonical2 {
        return true;
    }
    
    // Check if they're both KeepKey devices (same VID/PID)
    let keepkey_pattern = "keepkey_2b24_0002_";
    if id1.contains(keepkey_pattern) && id2.contains(keepkey_pattern) {
        log::info!("Both {} and {} appear to be KeepKey devices", id1, id2);
        return true;
    }
    
    // Check if one has a serial and the other is a bus/address format
    let is_serial_format = |id: &str| id.len() == 24 && id.chars().all(|c| c.is_alphanumeric());
    let is_bus_addr_format = |id: &str| id.contains("bus") && id.contains("addr");
    
    if (is_serial_format(id1) && is_bus_addr_format(id2)) ||
       (is_bus_addr_format(id1) && is_serial_format(id2)) {
        log::info!("Device IDs {} and {} might be the same device (serial vs bus/addr)", id1, id2);
        return true;
    }
    
    false
}

/// Send PIN matrix response to device (for responding to PinMatrixRequest)
#[tauri::command]
pub async fn send_pin_matrix_ack(
    device_id: String,
    positions: Vec<u8>,  // Positions 1-9 that user clicked
    queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<bool, String> {
    log::info!("Sending PIN matrix ACK for device: {} with {} positions", device_id, positions.len());
    
    // Validate positions
    if positions.is_empty() || positions.len() > 9 {
        return Err("PIN must be between 1 and 9 digits".to_string());
    }
    
    for &pos in &positions {
        if pos < 1 || pos > 9 {
            return Err("Invalid PIN position: positions must be 1-9".to_string());
        }
    }
    
    // Convert positions to PIN string (positions are 1-9)
    let pin = positions.iter().map(|&p| p.to_string()).collect::<String>();
    
    log::info!("Sending PIN matrix ACK with {} digits", pin.len());
    
    // Get device queue handle
    let queue_manager_guard = queue_manager.lock().await;
    let queue_handle = queue_manager_guard.get(&device_id)
        .ok_or_else(|| {
            let _ = unmark_device_in_pin_flow(&device_id);
            format!("Device not found: {}", device_id)
        })?;
        
    // Create PinMatrixAck message
    let pin_ack = keepkey_rust::messages::PinMatrixAck { pin };
    
    // Send the PIN response and wait for device response
    match queue_handle.send_raw(pin_ack.into(), true).await {
        Ok(keepkey_rust::messages::Message::Success(_)) => {
            log::info!("✅ PIN accepted! Device unlocked successfully");
            // Unmark device from PIN flow as PIN has been accepted
            let _ = unmark_device_in_pin_flow(&device_id);
            Ok(true)
        }
        Ok(keepkey_rust::messages::Message::Failure(f)) => {
            log::error!("❌ PIN rejected: {:?}", f.message);
            // Unmark device from PIN flow on failure
            let _ = unmark_device_in_pin_flow(&device_id);
            
            // Determine if it's an incorrect PIN or other error
            let error_msg = f.message.unwrap_or_else(|| "PIN verification failed".to_string());
            if error_msg.contains("PIN") || error_msg.contains("Invalid") {
                Err("Incorrect PIN. Please try again.".to_string())
            } else {
                Err(format!("PIN verification failed: {}", error_msg))
            }
        }
        Ok(other_msg) => {
            log::warn!("Unexpected response to PIN: {:?}", other_msg.message_type());
            let _ = unmark_device_in_pin_flow(&device_id);
            
            // Some devices might respond with Address or other success messages
            if matches!(other_msg, keepkey_rust::messages::Message::Address(_)) {
                log::info!("✅ PIN accepted (got Address response)");
                Ok(true)
            } else {
                Err(format!("Unexpected response: {:?}", other_msg.message_type()))
            }
        }
        Err(e) => {
            log::error!("Failed to send PIN matrix ACK for device {}: {}", device_id, e);
            // Clean up PIN flow marking on error
            let _ = unmark_device_in_pin_flow(&device_id);
            Err(format!("Failed to send PIN: {}", e))
        }
    }
}

/// Trigger a PIN request by making an authenticated request (GetAddress)
#[tauri::command] 
pub async fn trigger_pin_request(
    device_id: String,
    queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<bool, String> {
    log::info!("Triggering PIN request for device: {}", device_id);
    
    // Check if already in PIN flow
    if is_device_in_pin_flow(&device_id) {
        log::info!("Device {} is already in PIN flow, returning success", device_id);
        return Ok(true);
    }
    
    // Mark device as in PIN flow to prevent other operations from interfering
    mark_device_in_pin_flow(&device_id)?;
    
    // Get device queue handle
    let queue_manager_guard = queue_manager.lock().await;
    let queue_handle = queue_manager_guard.get(&device_id)
        .ok_or_else(|| {
            // Clean up PIN flow marking on error
            let _ = unmark_device_in_pin_flow(&device_id);
            format!("Device not found: {}", device_id)
        })?;
        
    // Create a simple GetAddress request that will trigger PIN on locked device
    let get_address = keepkey_rust::messages::GetAddress {
        address_n: vec![44, 0, 0, 0, 0], // m/44'/0'/0'/0/0
        coin_name: Some("Bitcoin".to_string()),
        script_type: Some(0), // SPENDADDRESS
        show_display: Some(false),
        ..Default::default()
    };
    
    // Send the request - this will trigger PinMatrixRequest on locked device
    match queue_handle.send_raw(get_address.into(), false).await {
        Ok(keepkey_rust::messages::Message::PinMatrixRequest(_)) => {
            log::info!("Successfully triggered PIN request for device: {}", device_id);
            // Keep device marked as in PIN flow - will be unmarked when PIN is completed
            Ok(true)
        }
        Ok(keepkey_rust::messages::Message::Failure(f)) => {
            // Check if this is the expected "Unknown message" failure when device is already in PIN mode
            if f.message.as_deref() == Some("Unknown message") {
                log::info!("Device {} appears to already be in PIN mode (got 'Unknown message')", device_id);
                // Device is already waiting for PIN - this is success
                Ok(true)
            } else {
                log::warn!("PIN trigger failed with: {:?}", f.message);
                let _ = unmark_device_in_pin_flow(&device_id);
                Err(format!("Failed to trigger PIN: {}", f.message.unwrap_or_default()))
            }
        }
        Ok(other_msg) => {
            log::warn!("Unexpected response when triggering PIN request: {:?}", other_msg.message_type());
            // Clean up PIN flow marking on unexpected response
            let _ = unmark_device_in_pin_flow(&device_id);
            Err(format!("Unexpected response: {:?}", other_msg.message_type()))
        }
        Err(e) => {
            log::error!("Failed to trigger PIN request for device {}: {}", device_id, e);
            // Clean up PIN flow marking on error
            let _ = unmark_device_in_pin_flow(&device_id);
            Err(format!("Failed to trigger PIN request: {}", e))
        }
    }
}

/// Test command to verify bootloader mode device status evaluation
#[tauri::command]
pub async fn test_bootloader_mode_device_status() -> Result<String, String> {
    println!("🧪 Testing bootloader mode device status evaluation...");
    
    // Create mock DeviceFeatures for a device in bootloader mode with v2.1.4
    let bootloader_device_features = DeviceFeatures {
        label: Some("KeepKey".to_string()),
        vendor: Some("KeepKey LLC".to_string()),
        model: Some("KeepKey".to_string()),
        firmware_variant: None,
        device_id: Some("test-device-bootloader".to_string()),
        language: Some("english".to_string()),
        bootloader_mode: true, // Device is in bootloader mode
        version: "2.1.4".to_string(), // Modern bootloader version
        firmware_hash: None,
        bootloader_hash: None,
        bootloader_version: None,
        initialized: false, // Not applicable in bootloader mode
        imported: None,
        no_backup: true,
        pin_protection: false,
        pin_cached: false,
        passphrase_protection: false,
        passphrase_cached: false,
        wipe_code_protection: false,
        auto_lock_delay_ms: None,
        policies: vec![],
    };
    
    // Test the evaluation
    let status = evaluate_device_status("test-device-bootloader".to_string(), Some(&bootloader_device_features));
    
    println!("🔧 Bootloader Mode Device Status Results:");
    println!("  - bootloader_mode: {}", bootloader_device_features.bootloader_mode);
    println!("  - needs_bootloader_update: {}", status.needs_bootloader_update);
    println!("  - needs_firmware_update: {}", status.needs_firmware_update);
    println!("  - needs_initialization: {}", status.needs_initialization);
    println!("  - bootloader_check: {:?}", status.bootloader_check);
    
    // Verify that bootloader mode device always needs bootloader update
    if status.needs_bootloader_update && bootloader_device_features.bootloader_mode {
        println!("✅ CORRECT: Device in bootloader mode correctly marked as needing bootloader update");
        Ok("Test passed: Bootloader mode device correctly requires update".to_string())
    } else if !status.needs_bootloader_update {
        println!("❌ INCORRECT: Device in bootloader mode should always need update");
        Err("Test failed: Device in bootloader mode not marked as needing update".to_string())
    } else {
        println!("❌ UNEXPECTED: Unknown test condition");
        Err("Test failed: Unexpected evaluation result".to_string())
    }
}

/// Test command to verify OOB device status evaluation
#[tauri::command]
pub async fn test_oob_device_status_evaluation() -> Result<String, String> {
    println!("🧪 Testing OOB device status evaluation...");
    
    // Create mock DeviceFeatures for an OOB device with v1.0.3 bootloader AND firmware
    let oob_device_features = DeviceFeatures {
        label: Some("KeepKey".to_string()),
        vendor: Some("KeepKey LLC".to_string()),
        model: Some("KeepKey".to_string()),
        firmware_variant: None,
        device_id: Some("test-device-001".to_string()),
        language: Some("english".to_string()),
        bootloader_mode: false, // OOB device is in normal firmware mode
        version: "1.0.3".to_string(), // OOB firmware version
        firmware_hash: None,
        bootloader_hash: None,
        bootloader_version: None, // OOB devices don't expose this
        initialized: false, // OOB device is not initialized
        imported: None,
        no_backup: true, // No backup exists yet
        pin_protection: false, // No PIN set yet
        pin_cached: false,
        passphrase_protection: false,
        passphrase_cached: false,
        wipe_code_protection: false,
        auto_lock_delay_ms: None,
        policies: vec![],
    };
    
    // Test the evaluation
    let status = evaluate_device_status("test-device-001".to_string(), Some(&oob_device_features));
    
    println!("🔧 OOB Device Status Results:");
    println!("  - needs_bootloader_update: {}", status.needs_bootloader_update);
    println!("  - needs_firmware_update: {}", status.needs_firmware_update);
    println!("  - needs_initialization: {}", status.needs_initialization);
    println!("  - bootloader_check: {:?}", status.bootloader_check);
    println!("  - firmware_check: {:?}", status.firmware_check);
    
    // Verify that bootloader update is prioritized
    if status.needs_bootloader_update && !status.needs_firmware_update {
        println!("✅ CORRECT: Bootloader update is prioritized over firmware update for OOB device");
        Ok("Test passed: OOB device correctly prioritizes bootloader update".to_string())
    } else if status.needs_bootloader_update && status.needs_firmware_update {
        println!("❌ INCORRECT: Both bootloader and firmware updates are requested (should prioritize bootloader)");
        Err("Test failed: Firmware update should be suppressed until bootloader is updated".to_string())
    } else if !status.needs_bootloader_update {
        println!("❌ INCORRECT: OOB device v1.0.3 should need bootloader update");
        Err("Test failed: OOB device v1.0.3 should require bootloader update".to_string())
    } else {
        println!("❌ UNEXPECTED: Unknown test condition");
        Err("Test failed: Unexpected evaluation result".to_string())
    }
}

/// Check if device is ready for PIN operations
#[tauri::command]
pub async fn check_device_pin_ready(
    device_id: String,
    queue_manager: tauri::State<'_, DeviceQueueManager>,
) -> Result<bool, String> {
    log::info!("Checking if device {} is ready for PIN operations", device_id);
    
    // Check if device is already in PIN flow
    if is_device_in_pin_flow(&device_id) {
        log::info!("Device {} is already in PIN flow", device_id);
        return Ok(true);
    }
    
    // Get device status first
    let device_status = get_device_status(device_id.clone(), queue_manager.clone()).await?;
    
    match device_status {
        Some(status) => {
            // Device must be connected and need PIN unlock
            if !status.connected {
                log::info!("Device {} is not connected", device_id);
                return Ok(false);
            }
            
            if !status.needs_pin_unlock {
                log::info!("Device {} does not need PIN unlock", device_id);
                return Ok(false);
            }
            
            // Check if we can communicate with the device
            let queue_handle = {
                let manager = queue_manager.lock().await;
                manager.get(&device_id).cloned()
            };
            
            if queue_handle.is_none() {
                log::info!("No queue handle available for device {}", device_id);
                return Ok(false);
            }
            
            log::info!("Device {} is ready for PIN operations", device_id);
            Ok(true)
        }
        None => {
            log::info!("Device {} status not available", device_id);
            Ok(false)
        }
    }
}

/// Clean up device queue worker and resources
async fn cleanup_device_queue(device_id: &str, queue_manager: &DeviceQueueManager) -> Result<(), String> {
    let mut cleanup_tracker = DEVICE_CLEANUP_TRACKER.lock().await;
    
    // Prevent duplicate cleanup
    if cleanup_tracker.contains(device_id) {
        return Ok(());
    }
    cleanup_tracker.insert(device_id.to_string());
    
    // Remove from queue manager and shutdown worker
    let handle = {
        let mut manager = queue_manager.lock().await;
        manager.remove(device_id)
    };
    
    if let Some(handle) = handle {
        println!("🧹 Cleaning up device queue worker for: {}", device_id);
        if let Err(e) = handle.shutdown().await {
            eprintln!("⚠️ Failed to shutdown device worker {}: {}", device_id, e);
        }
    }
    
    // Remove from cleanup tracker
    cleanup_tracker.remove(device_id);
    Ok(())
}

/// Clean up device resources when device disconnects (public API)
#[tauri::command]
pub async fn cleanup_disconnected_device(
    device_id: String,
    queue_manager: State<'_, DeviceQueueManager>,
) -> Result<(), String> {
    println!("🔌❌ Device disconnected, cleaning up resources: {}", device_id);
    cleanup_device_queue(&device_id, &queue_manager).await
}

/// Get or create device queue handle with proper deduplication
pub async fn get_or_create_device_queue(device_id: &str, queue_manager: &DeviceQueueManager) -> Result<DeviceQueueHandle, String> {
    // First check if we already have a handle
    {
        let manager = queue_manager.lock().await;
        if let Some(handle) = manager.get(device_id) {
            // NEVER clean up a working device handle during normal operation
            // The device worker should persist for the entire session
            return Ok(handle.clone());
        }
    }
    
    // Find the device by ID
    let devices = keepkey_rust::features::list_connected_devices();
    let device_info = devices
        .iter()
        .find(|d| d.unique_id == device_id)
        .ok_or_else(|| format!("Device {} not found", device_id))?;

    // Create new worker with proper locking to prevent race conditions
    let mut manager = queue_manager.lock().await;
    
    // Double-check after acquiring lock (race condition protection)
    if let Some(handle) = manager.get(device_id) {
        // Handle already exists, use it - no validation needed during normal operation
        return Ok(handle.clone());
    }
    
    // Spawn a new device worker
    println!("🚀 Spawning new device worker for: {}", device_id);
    let handle = keepkey_rust::device_queue::DeviceQueueFactory::spawn_worker(device_id.to_string(), device_info.clone());
    manager.insert(device_id.to_string(), handle.clone());
    
    Ok(handle)
}

// ========== Cache Commands ==========

/// Helper function to get or initialize cache manager
pub async fn get_cache_manager(
    cache_cell: &Arc<once_cell::sync::OnceCell<Arc<crate::cache::CacheManager>>>,
) -> Result<Arc<crate::cache::CacheManager>, String> {
    if let Some(cache) = cache_cell.get() {
        Ok(cache.clone())
    } else {
        // Initialize cache for the first time
        match crate::cache::init_cache().await {
            Ok(cache_manager) => {
                if let Err(_) = cache_cell.set(cache_manager.clone()) {
                    // Another thread already set it, use the existing one
                    Ok(cache_cell.get().unwrap().clone())
                } else {
                    println!("✅ Cache system initialized on first use");
                    Ok(cache_manager)
                }
            }
            Err(e) => {
                eprintln!("⚠️ Failed to initialize cache system: {}", e);
                Err(format!("Failed to initialize cache: {}", e))
            }
        }
    }
}

/// Get cache status for a device
#[tauri::command]
pub async fn get_cache_status(
    device_id: String,
    cache_manager: State<'_, Arc<once_cell::sync::OnceCell<Arc<crate::cache::CacheManager>>>>,
) -> Result<crate::cache::types::CacheStatus, String> {
    let cache = get_cache_manager(cache_manager.inner()).await?;
    cache
        .get_cache_status(&device_id)
        .await
        .map_err(|e| format!("Failed to get cache status: {}", e))
}

/// Trigger frontload for a device
#[tauri::command]
pub async fn trigger_frontload(
    device_id: String,
    cache_manager: State<'_, Arc<once_cell::sync::OnceCell<Arc<crate::cache::CacheManager>>>>,
    queue_manager: State<'_, DeviceQueueManager>,
) -> Result<(), String> {
    let cache = get_cache_manager(cache_manager.inner()).await?;
    let frontload_controller = crate::cache::FrontloadController::new(
        cache,
        queue_manager.inner().clone(),
    );
    
    // Run frontload in background
    let device_id_clone = device_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = frontload_controller.frontload_device(&device_id_clone).await {
            log::error!("Frontload failed for device {}: {}", device_id_clone, e);
        }
    });
    
    Ok(())
}

/// Clear cache for a specific device
#[tauri::command]
pub async fn clear_device_cache(
    device_id: String,
    cache_manager: State<'_, Arc<once_cell::sync::OnceCell<Arc<crate::cache::CacheManager>>>>,
) -> Result<(), String> {
    let cache = get_cache_manager(cache_manager.inner()).await?;
    cache
        .clear_device_cache(&device_id)
        .await
        .map_err(|e| format!("Failed to clear device cache: {}", e))
}