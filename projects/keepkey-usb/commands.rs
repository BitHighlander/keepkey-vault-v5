// commands.rs - Separate module for Tauri commands
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::features::DeviceFeatures;
use crate::device_registry;

use crate::blocking_actions::{BlockingAction, BlockingActionType, BlockingActionsState};

use tauri::{Manager, Emitter};
use crate::device_queue::DeviceQueueHandle;
use crate::index_db::{IndexDb, RequiredPath, WalletXpub, PortfolioCache, PortfolioCacheInput, FeeRateCache};
use log;
use serde_json;
use std::collections::HashSet;
use tauri::command;
use anyhow::Result;

// ========== Recovery Session Management ==========

use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    Space,     // Move to next word
    Done,      // Complete recovery  
    Delete,    // Backspace
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryProgress {
    pub word_pos: u32,
    pub character_pos: u32,
    pub auto_completed: bool,
    pub is_complete: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStatus {
    pub session: RecoverySession,
    pub is_waiting_for_input: bool,
    pub error: Option<String>,
}

// Global recovery sessions
lazy_static::lazy_static! {
    static ref RECOVERY_SESSIONS: Mutex<HashMap<String, RecoverySession>> = 
        Mutex::new(HashMap::new());
}

/// Tauri command to get device features from a connected KeepKey
/// This now uses the device registry and returns the first device's features
/// for backward compatibility
#[tauri::command]
pub fn get_device_info() -> Result<DeviceFeatures, String> {
    match device_registry::get_first_device_features()? {
        Some(features) => Ok(features),
        None => Err("No device connected or features not available yet".to_string())
    }
}

/// Get features for a specific device by ID
#[tauri::command]
pub fn get_device_info_by_id(device_id: String) -> Result<DeviceFeatures, String> {
    match device_registry::get_device_features(&device_id)? {
        Some(features) => Ok(features),
        None => Err(format!("No features available for device {}", device_id))
    }
}

/// Get all connected devices with their features
#[tauri::command]
pub fn get_all_devices() -> Result<Vec<Value>, String> {
    log::debug!("Getting all devices from database");
    let db = IndexDb::open().map_err(|e| e.to_string())?;
    let devices = db.get_all_devices().map_err(|e| e.to_string())?;
    
    // Convert to JSON Value for frontend
    let json_devices = devices.into_iter()
        .map(|d| serde_json::to_value(d).unwrap())
        .collect();
    
    Ok(json_devices)
}

// ========== Vault Commands ==========

/// Check if a vault exists at the default location
#[tauri::command]
pub fn check_vault_exists() -> bool {
    crate::vault::Vault::exists()
}

/// Create a new encrypted vault
#[tauri::command]
pub fn create_vault(
    state: tauri::State<crate::vault::VaultState>,
    password: String,
    kk_signature: String,
) -> Result<(), String> {
    let vault_path = dirs::home_dir()
        .ok_or_else(|| "Could not find home directory".to_string())?
        .join(".keepkey/vault.db");
    
    let kk_sig_bytes = hex::decode(kk_signature)
        .map_err(|e| format!("Invalid signature hex: {}", e))?;
    
    let vault = crate::vault::Vault::create(&vault_path, &password, &kk_sig_bytes)
        .map_err(|e| format!("Failed to create vault: {}", e))?;
    
    *state.0.lock().unwrap() = Some(vault);
    Ok(())
}

/// Unlock an existing vault
#[tauri::command]
pub fn unlock_vault(
    state: tauri::State<crate::vault::VaultState>,
    password: String,
    kk_signature: String,
) -> Result<(), String> {
    let vault_path = dirs::home_dir()
        .ok_or_else(|| "Could not find home directory".to_string())?
        .join(".keepkey/vault.db");
    
    if !vault_path.exists() {
        return Err("Vault does not exist".to_string());
    }
    
    let kk_sig_bytes = hex::decode(kk_signature)
        .map_err(|e| format!("Invalid signature hex: {}", e))?;
    
    let vault = crate::vault::Vault::unlock(&vault_path, &password, &kk_sig_bytes)
        .map_err(|e| format!("Failed to unlock vault: {}", e))?;
    
    *state.0.lock().unwrap() = Some(vault);
    Ok(())
}

// ========== Onboarding Commands ==========

/// Check if this is a first-time install
#[tauri::command]
pub fn is_first_time_install() -> Result<bool, String> {
    log::info!("=== Checking if first time install ===");
    
    // First check if database file exists at all
    let db_exists = IndexDb::database_exists();
    log::info!("Database file exists: {}", db_exists);
    
    if !db_exists {
        log::info!("Database does not exist - this is a first time install");
        return Ok(true);
    }
    
    // If database exists, check its contents
    let db = IndexDb::open().map_err(|e| {
        log::error!("Failed to open database: {}", e);
        e.to_string()
    })?;
    let is_first_time = db.is_first_time_install().map_err(|e| {
        log::error!("Failed to check first time install: {}", e);
        e.to_string()
    })?;
    log::info!("=== First time install result: {} ===", is_first_time);
    Ok(is_first_time)
}

/// Check if user has completed onboarding
#[tauri::command]
pub fn is_onboarded() -> Result<bool, String> {
    log::info!("=== Checking if user is onboarded ===");
    let db = IndexDb::open().map_err(|e| {
        log::error!("Failed to open database: {}", e);
        e.to_string()
    })?;
    let is_onboarded = db.is_onboarded().map_err(|e| {
        log::error!("Failed to check onboarding status: {}", e);
        e.to_string()
    })?;
    log::info!("=== Onboarded status: {} ===", is_onboarded);
    Ok(is_onboarded)
}

/// Mark onboarding as completed
#[tauri::command]
pub fn set_onboarding_completed() -> Result<(), String> {
    log::info!("=== Setting onboarding as completed ===");
    let db = IndexDb::open().map_err(|e| {
        log::error!("Failed to open database for onboarding completion: {}", e);
        e.to_string()
    })?;
    
    // Verify current state before setting
    let current_state = db.is_onboarded().map_err(|e| {
        log::error!("Failed to check current onboarding state: {}", e);
        e.to_string()
    })?;
    log::info!("Current onboarding state before completion: {}", current_state);
    
    db.set_onboarding_completed().map_err(|e| {
        log::error!("Failed to set onboarding completed: {}", e);
        e.to_string()
    })?;
    
    // Verify the state was actually set
    let new_state = db.is_onboarded().map_err(|e| {
        log::error!("Failed to verify onboarding state after completion: {}", e);
        e.to_string()
    })?;
    
    log::info!("=== Onboarding completion successful! State changed from {} to {} ===", current_state, new_state);
    
    if !new_state {
        log::error!("ERROR: Onboarding completion failed - state is still false!");
        return Err("Failed to set onboarding completed state".to_string());
    }
    
    Ok(())
}

/// Debug command to check onboarding state and related database entries
#[tauri::command]
pub fn debug_onboarding_state() -> Result<String, String> {
    log::info!("=== Debug: Checking onboarding state ===");
    
    let db_exists = IndexDb::database_exists();
    log::info!("Database file exists: {}", db_exists);
    
    if !db_exists {
        return Ok("Database file does not exist".to_string());
    }
    
    let db = IndexDb::open().map_err(|e| {
        log::error!("Failed to open database: {}", e);
        e.to_string()
    })?;
    
    let is_first_time = db.is_first_time_install().map_err(|e| e.to_string())?;
    let is_onboarded = db.is_onboarded().map_err(|e| e.to_string())?;
    let onboarding_timestamp = db.get_onboarding_timestamp().map_err(|e| e.to_string())?;
    
    let result = format!(
        "Database exists: {}\nFirst time install: {}\nIs onboarded: {}\nOnboarding timestamp: {:?}",
        db_exists, is_first_time, is_onboarded, onboarding_timestamp
    );
    
    log::info!("Debug result: {}", result);
    Ok(result)
}

/// Get a user preference
#[tauri::command]
pub fn get_preference(key: String) -> Result<Option<String>, String> {
    log::debug!("Getting preference: {}", key);
    let db = IndexDb::open().map_err(|e| e.to_string())?;
    let value = db.get_preference(&key).map_err(|e| e.to_string())?;
    log::debug!("Preference {} = {:?}", key, value);
    Ok(value)
}

/// Set a user preference
#[tauri::command]
pub fn set_preference(key: String, value: String) -> Result<(), String> {
    log::info!("Setting preference: {} = {}", key, value);
    let db = IndexDb::open().map_err(|e| e.to_string())?;
    db.set_preference(&key, &value).map_err(|e| e.to_string())?;
    log::info!("Preference saved");
    Ok(())
}

/// Get API enable status
#[tauri::command]
pub fn get_api_enabled() -> Result<bool, String> {
    log::debug!("Getting API enabled status");
    let db = IndexDb::open().map_err(|e| e.to_string())?;
    let enabled = db.get_preference("api_enabled").map_err(|e| e.to_string())?;
    // Default to false (disabled) if not set
    let is_enabled = enabled.as_deref() == Some("true");
    log::debug!("API enabled status: {}", is_enabled);
    Ok(is_enabled)
}

/// Set API enable status
#[tauri::command]
pub fn set_api_enabled(enabled: bool) -> Result<(), String> {
    log::info!("Setting API enabled status: {}", enabled);
    let db = IndexDb::open().map_err(|e| e.to_string())?;
    let value = if enabled { "true" } else { "false" };
    db.set_preference("api_enabled", value).map_err(|e| e.to_string())?;
    log::info!("API enabled status saved: {}", enabled);
    Ok(())
}

/// Get API status (running or not)
#[tauri::command]
pub fn get_api_status() -> Result<serde_json::Value, String> {
    log::debug!("Getting API status");
    let db = IndexDb::open().map_err(|e| e.to_string())?;
    let enabled = db.get_preference("api_enabled").map_err(|e| e.to_string())?;
    let is_enabled = enabled.as_deref() == Some("true");
    
    // Check if server is actually running by trying to connect to it
    let is_running = if is_enabled {
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
        "enabled": is_enabled,
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

/// Restart API server based on current enabled setting
#[tauri::command]
pub async fn restart_api_server(app_handle: tauri::AppHandle) -> Result<bool, String> {
    log::info!("Restarting API server based on current settings");
    
    let db = IndexDb::open().map_err(|e| e.to_string())?;
    let enabled = db.get_preference("api_enabled").map_err(|e| e.to_string())?;
    let is_enabled = enabled.as_deref() == Some("true");
    
    if is_enabled {
        log::info!("API is enabled, starting server...");
        
        // Get device manager from app state
        let device_manager = app_handle.state::<std::sync::Arc<std::sync::Mutex<crate::usb_manager::DeviceManager>>>();
        let dm = device_manager.inner().clone();
        
        // Start server in background
        let server_handle = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::server::start_server(dm).await {
                log::error!("Failed to start API server: {}", e);
                // Emit error event
                server_handle.emit("api:error", serde_json::json!({
                    "error": format!("Failed to start API server: {}", e)
                })).ok();
            } else {
                log::info!("API server started successfully");
                // Emit success event
                server_handle.emit("api:started", serde_json::json!({
                    "message": "API server started successfully"
                })).ok();
            }
        });
        
        Ok(true)
    } else {
        log::info!("API is disabled, not starting server");
        // Note: We don't implement server shutdown here as it's more complex
        // The server will need to be stopped by restarting the application
        Ok(false)
    }
}

// Device tracking commands

#[tauri::command]
pub fn get_connected_devices() -> Result<Vec<Value>, String> {
    log::debug!("Getting connected devices from registry");
    
    // Get all device entries from the registry
    let entries = device_registry::get_all_device_entries()
        .map_err(|e| format!("Failed to get device entries: {}", e))?;
    
    // Convert to JSON Value for frontend, matching the expected structure
    let json_devices = entries.into_iter()
        .filter(|entry| entry.device.is_keepkey)
        .map(|entry| {
            // Create a structure that matches what the frontend expects
            serde_json::json!({
                "device": {
                    "unique_id": entry.device.unique_id,
                    "name": entry.device.name,
                    "vid": entry.device.vid,
                    "pid": entry.device.pid,
                    "manufacturer": entry.device.manufacturer,
                    "product": entry.device.product,
                    "serial_number": entry.device.serial_number,
                    "is_keepkey": entry.device.is_keepkey,
                },
                "features": entry.features,
            })
        })
        .collect();
    
    Ok(json_devices)
}

#[tauri::command]
pub fn get_disconnected_devices() -> Result<Vec<Value>, String> {
    log::debug!("Getting disconnected devices from database");
    let db = IndexDb::open().map_err(|e| e.to_string())?;
    let devices = db.get_disconnected_devices().map_err(|e| e.to_string())?;
    
    // Convert to JSON Value for frontend
    let json_devices = devices.into_iter()
        .map(|d| serde_json::to_value(d).unwrap())
        .collect();
    
    Ok(json_devices)
}

/// Get device status including update needs
#[tauri::command]
pub async fn get_device_status(device_id: String) -> Result<Option<DeviceStatus>, String> {
    log::info!("Getting device status for: {}", device_id);
    
    // Get all device entries
    let entries = device_registry::get_all_device_entries()
        .map_err(|e| format!("Failed to get device entries: {}", e))?;
    
    // Find the specific device
    let entry = entries.iter()
        .find(|e| e.device.unique_id == device_id);
    
    if let Some(entry) = entry {
        if let Some(features) = &entry.features {
            // Evaluate device status
            let status = evaluate_device_status(device_id, features);
            Ok(Some(status))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

// ========== Blocking Actions Commands ==========

/// Get all blocking actions for a specific device
#[tauri::command]
pub fn get_blocking_actions(
    state: tauri::State<'_, BlockingActionsState>,
    device_id: Option<String>
) -> Result<Vec<BlockingAction>, String> {
    let registry = state.registry();
    let registry_lock = registry.lock().map_err(|_| "Failed to lock registry".to_string())?;
    
    let actions = if let Some(device_id) = device_id {
        // Get actions for specific device
        registry_lock.get_actions_for_device(&device_id)
    } else {
        // Get all actions across all devices
        registry_lock.get_all_actions()
    };
    
    Ok(actions)
}

/// Mark a blocking action as resolved
#[tauri::command]
pub fn resolve_blocking_action(
    state: tauri::State<'_, BlockingActionsState>,
    device_id: String,
    action_type: BlockingActionType
) -> Result<bool, String> {
    let registry = state.registry();
    let mut registry_lock = registry.lock().map_err(|_| "Failed to lock registry".to_string())?;
    
    let removed = registry_lock.remove_action(&device_id, action_type);
    
    // Return whether an action was actually removed
    Ok(removed)
}

// ========== Wallet Creation Commands ==========

/// Set device label
#[tauri::command]
pub async fn set_device_label(device_id: String, label: String) -> Result<(), String> {
    log::info!("Setting device label for {}: '{}'", device_id, label);
    
    // Validate label (max 12 chars, ASCII only)
    if label.len() > 12 {
        return Err("Label must be 12 characters or less".to_string());
    }
    
    if !label.chars().all(|c| c.is_ascii() && !c.is_control()) {
        return Err("Label must contain only ASCII printable characters".to_string());
    }
    
    // Get device entry to find the actual device
    let entries = device_registry::get_all_device_entries()
        .map_err(|e| format!("Failed to get device entries: {}", e))?;
    
    let target_device = entries.iter()
        .find(|entry| entry.device.unique_id == device_id)
        .ok_or_else(|| format!("Device not found: {}", device_id))?;
    
    // Create ApplySettings message with the label
    let apply_settings = crate::messages::ApplySettings {
        language: None,
        label: Some(label.clone()),
        use_passphrase: None,
        auto_lock_delay_ms: None,
        u2f_counter: None,
    };
    
    // Try to use the same pattern as the features module
    let result = match crate::features::get_device_features_with_fallback(&target_device.device) {
        Ok(_) => {
            // Device is communicating, now find the physical device for transport
            let devices = crate::features::list_devices();
            
            let physical_device = if let Some(serial) = &target_device.device.serial_number {
                // Match by serial number
                devices.iter().find(|d| {
                    if let Ok(handle) = d.open() {
                        let timeout = std::time::Duration::from_millis(100);
                        if let Ok(langs) = handle.read_languages(timeout) {
                            if let Some(lang) = langs.first() {
                                if let Ok(desc) = d.device_descriptor() {
                                    if let Ok(device_serial) = handle.read_serial_number_string(*lang, &desc, timeout) {
                                        return device_serial == *serial;
                                    }
                                }
                            }
                        }
                    }
                    false
                }).cloned()
            } else {
                // Try to parse bus and address from unique_id
                let parts: Vec<&str> = target_device.device.unique_id.split('_').collect();
                if parts.len() >= 2 {
                    let bus_str = parts[0].strip_prefix("bus").unwrap_or("");
                    let addr_str = parts[1].strip_prefix("addr").unwrap_or("");
                    
                    if let (Ok(bus), Ok(addr)) = (bus_str.parse::<u8>(), addr_str.parse::<u8>()) {
                        devices.iter().find(|d| d.bus_number() == bus && d.address() == addr).cloned()
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            
            match physical_device {
                Some(device) => {
                    // Try USB transport first
                    match crate::transport::UsbTransport::new(&device, 0) {
                        Ok((mut transport, _, _)) => {
                            log::info!("Using USB transport to set label for device {}", device_id);
                            
                            // Use transport as protocol adapter
                            let adapter = &mut transport as &mut dyn crate::transport::ProtocolAdapter;
                            let mut handler = adapter.with_standard_handler();
                            
                            // Send ApplySettings message
                            match handler.handle(apply_settings.into()) {
                                Ok(crate::messages::Message::Success(s)) => {
                                    log::info!("✅ Device label set successfully via USB: {}", s.message());
                                    Ok(())
                                }
                                Ok(crate::messages::Message::Failure(f)) => {
                                    Err(format!("Device rejected label change: {}", f.message()))
                                }
                                Ok(other) => {
                                    Err(format!("Unexpected response from device: {:?}", other.message_type()))
                                }
                                Err(e) => {
                                    Err(format!("Failed to communicate with device: {}", e))
                                }
                            }
                        }
                        Err(usb_err) => {
                            log::warn!("USB transport failed for device {}: {}, trying HID fallback", device_id, usb_err);
                            
                            // Try HID fallback
                            match crate::transport::HidTransport::new_for_device(target_device.device.serial_number.as_deref()) {
                                Ok(mut hid_transport) => {
                                    log::info!("Using HID transport to set label for device {}", device_id);
                                    
                                    // Use transport as protocol adapter
                                    let adapter = &mut hid_transport as &mut dyn crate::transport::ProtocolAdapter;
                                    let mut handler = adapter.with_standard_handler();
                                    
                                    // Send ApplySettings message
                                    match handler.handle(apply_settings.into()) {
                                        Ok(crate::messages::Message::Success(s)) => {
                                            log::info!("✅ Device label set successfully via HID: {}", s.message());
                                            Ok(())
                                        }
                                        Ok(crate::messages::Message::Failure(f)) => {
                                            Err(format!("Device rejected label change: {}", f.message()))
                                        }
                                        Ok(other) => {
                                            Err(format!("Unexpected response from device: {:?}", other.message_type()))
                                        }
                                        Err(e) => {
                                            Err(format!("Failed to communicate with device via HID: {}", e))
                                        }
                                    }
                                }
                                Err(hid_err) => {
                                    Err(format!("Failed with both USB ({}) and HID ({})", usb_err, hid_err))
                                }
                            }
                        }
                    }
                }
                None => {
                    Err(format!("Physical device not found for {}", device_id))
                }
            }
        }
        Err(e) => {
            Err(format!("Device is not communicating: {}", e))
        }
    };
    
    // If successful, trigger a features refresh to update the device registry
    if result.is_ok() {
        log::info!("Label set successfully, triggering device features refresh for {}", device_id);
        
        // Trigger a refresh of device features in the background to pick up the new label
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            if let Ok(entries) = device_registry::get_all_device_entries() {
                if let Some(target_device) = entries.iter().find(|e| e.device.unique_id == device_id) {
                    match crate::features::get_device_features_with_fallback(&target_device.device) {
                        Ok(updated_features) => {
                            log::info!("Updated features after label change: label = {:?}", updated_features.label);
                            // Update the registry with new features
                            if let Ok(mut registry) = device_registry::DEVICE_REGISTRY.lock() {
                                if let Some(entry) = registry.get_mut(&device_id) {
                                    entry.features = Some(updated_features);
                                    log::info!("Registry updated with new label for device {}", device_id);
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to refresh features after label change: {}", e);
                        }
                    }
                }
            }
        });
    }
    
    result
}

// ========== PIN Creation Flow Implementation ==========

use std::sync::Arc;

// PIN creation session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinCreationSession {
    pub device_id: String,
    pub session_id: String,
    pub current_step: PinStep,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PinStep {
    AwaitingFirst,   // Waiting for first PIN entry
    AwaitingSecond,  // Waiting for PIN confirmation
    Completed,       // PIN creation done
    Failed,          // PIN creation failed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinMatrixResult {
    pub success: bool,
    pub next_step: Option<String>,
    pub session_id: String,
    pub error: Option<String>,
}

// Global PIN sessions
lazy_static::lazy_static! {
    static ref PIN_SESSIONS: Arc<Mutex<HashMap<String, PinCreationSession>>> = 
        Arc::new(Mutex::new(HashMap::new()));
}

/// Start PIN creation process by initiating ResetDevice with PIN protection
#[tauri::command]
pub async fn initialize_device_pin(device_id: String, label: Option<String>) -> Result<PinCreationSession, String> {
    log::info!("Starting PIN creation for device: {} with label: {:?}", device_id, label);
    
    // Check if device is already in PIN flow to prevent duplicate calls
    if is_device_in_pin_flow(&device_id) {
        return Err("Device is already in PIN creation flow".to_string());
    }
    
    // Generate unique session ID
    let session_id = format!("pin_session_{}_{}", device_id, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    
    // Mark device as being in PIN flow FIRST to prevent race conditions
    mark_device_in_pin_flow(&device_id)?;
    
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
    
    // Create ResetDevice message with PIN protection enabled
    let reset_device = crate::messages::ResetDevice {
        display_random: Some(false),  // Don't show confusing entropy screen to users
        strength: Some(256),
        passphrase_protection: Some(false),
        pin_protection: Some(true),  // This triggers PIN creation flow
        language: Some("english".to_string()),
        label: label.map(|l| l.to_string()),
        no_backup: Some(false),
        auto_lock_delay_ms: None,
        u2f_counter: None,
    };
    
    // Send ResetDevice message - THIS SHOULD RETURN PinMatrixRequest for frontend handling
    match send_message_to_device(&device_id, reset_device.into()).await {
        Ok(response) => {
            log::info!("✅ ResetDevice sent successfully, device responded with: {:?}", response.message_type());
            
            // Handle the response - should be PinMatrixRequest
            match response {
                crate::messages::Message::PinMatrixRequest(pmr) => {
                    log::info!("Device requesting PIN matrix input, type: {:?}", pmr.r#type);
                    // Device is ready for PIN input - return session to frontend
                    Ok(session)
                }
                crate::messages::Message::Success(_) => {
                    log::info!("Device reset completed without PIN request");
                    // Mark as completed
                    if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                        if let Some(session) = sessions.get_mut(&session_id) {
                            session.current_step = PinStep::Completed;
                            session.is_active = false;
                        }
                    }
                    let _ = unmark_device_in_pin_flow(&device_id);
                    Ok(session)
                }
                other => {
                    log::warn!("Unexpected response from ResetDevice: {:?}", other.message_type());
                    // Return session anyway - device might be ready for PIN
                    Ok(session)
                }
            }
        }
        Err(e) => {
            log::error!("Failed to send ResetDevice message: {}", e);
            // Remove from PIN flow and remove PIN session
            let _ = unmark_device_in_pin_flow(&device_id);
            let mut sessions = PIN_SESSIONS.lock().map_err(|_| "Failed to lock PIN sessions".to_string())?;
            sessions.remove(&session_id);
            Err(format!("Failed to start PIN creation: {}", e))
        }
    }
}

/// Send ButtonAck to acknowledge device button requests
/// NOTE: This is now handled automatically by the standard handler in session transport
/// This command is kept for compatibility but may not be needed in the new flow
#[tauri::command]
pub async fn send_button_ack(device_id: String) -> Result<(), String> {
    log::info!("Sending ButtonAck to device: {}", device_id);
    
    // Create ButtonAck message
    let button_ack = crate::messages::ButtonAck::default();
    
    // Send via transport
    match send_message_to_device(&device_id, button_ack.into()).await {
        Ok(response) => {
            log::info!("✅ ButtonAck sent successfully: {:?}", response.message_type());
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to send ButtonAck: {}", e);
            Err(format!("Failed to send ButtonAck: {}", e))
        }
    }
}

/// Send PIN matrix response (positions clicked by user)
#[tauri::command]
pub async fn send_pin_matrix_response(
    session_id: String,
    positions: Vec<u8>  // Positions 1-9 that user clicked
) -> Result<PinMatrixResult, String> {
    log::info!("Sending PIN matrix response for session: {} with {} positions", session_id, positions.len());
    
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
    let (device_id, current_step) = {
        let mut sessions = PIN_SESSIONS.lock().map_err(|_| "Failed to lock PIN sessions".to_string())?;
        let session = sessions.get_mut(&session_id)
            .ok_or_else(|| format!("PIN session not found: {}", session_id))?;
        
        if !session.is_active {
            return Err("PIN session is not active".to_string());
        }
        
        (session.device_id.clone(), session.current_step.clone())
    };
    
    // Convert positions to PIN string for device protocol (positions as characters)
    let pin_string: String = positions.iter()
        .map(|&pos| (b'0' + pos) as char)
        .collect();
    
    log::info!("Converted positions to PIN string for device communication: {}", pin_string);
    
    // Create PinMatrixAck message
    let pin_matrix_ack = crate::messages::PinMatrixAck {
        pin: pin_string.clone(),
    };
    
    // Send message to device (lock released)
    match send_message_to_device(&device_id, pin_matrix_ack.into()).await {
        Ok(response) => {
            log::info!("✅ PinMatrixAck sent successfully: {:?}", response.message_type());
            
            // Analyze response to determine next step
            match current_step {
                PinStep::AwaitingFirst => {
                    // First PIN entry - check what device wants next
                    match response {
                        crate::messages::Message::PinMatrixRequest(pmr) => {
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
                        crate::messages::Message::EntropyRequest(_) | 
                        crate::messages::Message::Success(_) => {
                            log::info!("✅ PIN creation completed in single step");
                            // Update session state
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Completed;
                                    session.is_active = false;
                                }
                            }
                            
                            Ok(PinMatrixResult {
                                success: true,
                                next_step: Some("complete".to_string()),
                                session_id: session_id.clone(),
                                error: None,
                            })
                        }
                        crate::messages::Message::Failure(f) => {
                            // Update session state
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Failed;
                                    session.is_active = false;
                                }
                            }
                            Err(format!("PIN creation failed: {}", f.message()))
                        }
                        _ => {
                            log::warn!("Unexpected response to first PIN: {:?}", response.message_type());
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
                        crate::messages::Message::EntropyRequest(_) => {
                            log::info!("✅ PIN confirmation accepted, device requesting entropy (handled automatically)");
                            // Entropy is handled automatically by PIN flow handler
                            // Update session state to completed
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Completed;
                                    session.is_active = false;
                                }
                            }
                            
                            Ok(PinMatrixResult {
                                success: true,
                                next_step: Some("complete".to_string()),
                                session_id: session_id.clone(),
                                error: None,
                            })
                        }
                        crate::messages::Message::Success(_) => {
                            log::info!("✅ PIN confirmation accepted, device initialization completed");
                            // Update session state
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Completed;
                                    session.is_active = false;
                                }
                            }
                            
                            Ok(PinMatrixResult {
                                success: true,
                                next_step: Some("complete".to_string()),
                                session_id: session_id.clone(),
                                error: None,
                            })
                        }
                        crate::messages::Message::Failure(f) => {
                            // Update session state
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Failed;
                                    session.is_active = false;
                                }
                            }
                            Err(format!("PIN confirmation failed: {}", f.message()))
                        }
                        _ => {
                            log::warn!("Unexpected response during PIN confirmation: {:?}", response.message_type());
                            // Update session state
                            if let Ok(mut sessions) = PIN_SESSIONS.lock() {
                                if let Some(session) = sessions.get_mut(&session_id) {
                                    session.current_step = PinStep::Completed;
                                    session.is_active = false;
                                }
                            }
                            
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
                    Err("PIN creation already completed".to_string())
                }
                PinStep::Failed => {
                    Err("PIN creation failed".to_string())
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
            Err(format!("Failed to send PIN to device: {}", e))
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
        
        // Remove from PIN flow
        drop(sessions); // Release lock before call
        let _ = unmark_device_in_pin_flow(&device_id);
        
        log::info!("PIN creation session cancelled and device session closed for: {}", device_id);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Complete PIN creation and close sessions
#[tauri::command]
pub async fn complete_pin_creation(session_id: String) -> Result<bool, String> {
    log::info!("Completing PIN creation session: {}", session_id);
    
    let mut sessions = PIN_SESSIONS.lock().map_err(|_| "Failed to lock PIN sessions".to_string())?;
    if let Some(session) = sessions.remove(&session_id) {
        let device_id = session.device_id.clone();
        
        // Remove from PIN flow
        drop(sessions); // Release lock before call
        let _ = unmark_device_in_pin_flow(&device_id);
        
        log::info!("PIN creation completed and device session closed for: {}", device_id);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Legacy command for compatibility - redirects to new PIN flow
#[tauri::command]
pub async fn confirm_device_pin(_device_id: String, _pin: String) -> Result<bool, String> {
    log::warn!("Legacy confirm_device_pin called - this should use the new PIN matrix flow");
    // This is kept for compatibility but should not be used in the new flow
    Ok(true)
}

/// Initialize/reset device to create new wallet
#[tauri::command]
pub async fn initialize_device_wallet(device_id: String, label: String) -> Result<(), String> {
    log::info!("Initializing wallet on device: {} with label: '{}'", device_id, label);
    
    // TODO: Implement actual device reset/initialization via HDWallet interface
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

/// Get recovery phrase from device (for backup display)
#[tauri::command]
pub async fn get_device_recovery_phrase(device_id: String) -> Result<Vec<String>, String> {
    log::info!("Getting recovery phrase from device: {}", device_id);
    
    // TODO: Implement actual recovery phrase retrieval via HDWallet interface
    // This should get the recovery phrase from the device for display/backup
    
    // Simulate device communication delay
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
    
    // For demo purposes, return a mock recovery phrase
    // In real implementation, this would come from the device
    let mock_phrase = vec![
        "abandon".to_string(), "ability".to_string(), "able".to_string(),
        "about".to_string(), "above".to_string(), "absent".to_string(),
        "absorb".to_string(), "abstract".to_string(), "absurd".to_string(),
        "abuse".to_string(), "access".to_string(), "accident".to_string(),
    ];
    
    log::info!("Recovery phrase retrieved successfully");
    Ok(mock_phrase)
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

/// Wipe device completely (reset to factory state)
#[tauri::command]
pub async fn wipe_device(device_id: String) -> Result<(), String> {
    log::info!("🗑️ [wipe_device] BACKEND: Wipe device command received for: {}", device_id);
    println!("🗑️ [wipe_device] BACKEND: Wipe device command received for: {}", device_id);
    
    // Get device entry
    println!("🗑️ [wipe_device] Getting device registry entries...");
    let entries = device_registry::get_all_device_entries()
        .map_err(|e| {
            let error = format!("Failed to get device entries: {}", e);
            println!("❌ [wipe_device] {}", error);
            error
        })?;
    
    println!("🗑️ [wipe_device] Found {} device entries, searching for device: {}", entries.len(), device_id);
    let _target_device = entries.iter()
        .find(|entry| entry.device.unique_id == device_id)
        .ok_or_else(|| {
            let error = format!("Device not found: {}", device_id);
            println!("❌ [wipe_device] {}", error);
            error
        })?;
    
    println!("✅ [wipe_device] Device found in registry");
    
    // Create WipeDevice message
    println!("🗑️ [wipe_device] Creating WipeDevice message...");
    let wipe_device = crate::messages::WipeDevice {};
    
    // Send wipe message to device
    println!("🗑️ [wipe_device] Sending wipe message to device...");
    match send_message_to_device(&device_id, wipe_device.into()).await {
        Ok(response) => {
            println!("✅ [wipe_device] Got response from device: {:?}", response.message_type());
            match response {
                crate::messages::Message::Success(success) => {
                    println!("✅ [wipe_device] Device wiped successfully: {}", success.message());
                    log::info!("✅ Device wiped successfully: {}", success.message());
                    
                    // Trigger a features refresh to update the device registry with wiped state
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                        if let Ok(entries) = device_registry::get_all_device_entries() {
                            if let Some(target_device) = entries.iter().find(|e| e.device.unique_id == device_id) {
                                match crate::features::get_device_features_with_fallback(&target_device.device) {
                                    Ok(updated_features) => {
                                        log::info!("Updated features after wipe: initialized = {}", updated_features.initialized);
                                        // Update the registry with new features
                                        if let Ok(mut registry) = device_registry::DEVICE_REGISTRY.lock() {
                                            if let Some(entry) = registry.get_mut(&device_id) {
                                                entry.features = Some(updated_features);
                                                log::info!("Registry updated with wiped device state for device {}", device_id);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to refresh features after wipe: {}", e);
                                    }
                                }
                            }
                        }
                    });
                    
                    Ok(())
                }
                crate::messages::Message::Failure(failure) => {
                    let error = format!("Device rejected wipe request: {}", failure.message());
                    println!("❌ [wipe_device] {}", error);
                    Err(error)
                }
                other => {
                    let error = format!("Unexpected response from device: {:?}", other.message_type());
                    println!("❌ [wipe_device] {}", error);
                    Err(error)
                }
            }
        }
        Err(e) => {
            let error = format!("Failed to communicate with device: {}", e);
            println!("❌ [wipe_device] {}", error);
            log::error!("Failed to send wipe command to device: {}", e);
            Err(error)
        }
    }
}

// ========== Dialog Queue Management Commands ==========

use std::collections::BinaryHeap;
use std::cmp::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogRequest {
    pub id: String,
    pub dialog_type: String,
    pub device_id: Option<String>,
    pub priority_points: u32,
    pub persistent: bool,
    pub metadata: serde_json::Value,
}

impl PartialEq for DialogRequest {
    fn eq(&self, other: &Self) -> bool {
        self.priority_points == other.priority_points
    }
}

impl Eq for DialogRequest {}

impl PartialOrd for DialogRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DialogRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority points = higher priority
        self.priority_points.cmp(&other.priority_points)
    }
}

// Global dialog queue managed by backend
lazy_static::lazy_static! {
    static ref DIALOG_QUEUE: Mutex<BinaryHeap<DialogRequest>> = Mutex::new(BinaryHeap::new());
    static ref ACTIVE_DIALOG: Mutex<Option<DialogRequest>> = Mutex::new(None);
}

/// Add a dialog to the backend-managed priority queue
#[tauri::command]
pub fn queue_dialog(dialog_request: DialogRequest) -> Result<DialogRequest, String> {
    log::info!("Queueing dialog: {} with priority: {}", dialog_request.id, dialog_request.priority_points);
    
    let mut queue = DIALOG_QUEUE.lock().map_err(|_| "Failed to lock dialog queue".to_string())?;
    let mut active = ACTIVE_DIALOG.lock().map_err(|_| "Failed to lock active dialog".to_string())?;
    
    // Check if there's no active dialog or if this one has higher priority
    let should_show_immediately = match &*active {
        None => {
            // No active dialog, show this one
            true
        },
        Some(current) => {
            // There's an active dialog, check priority
            if dialog_request.priority_points > current.priority_points {
                // Higher priority, queue the current one and show this one
                queue.push(current.clone());
                true
            } else {
                // Lower or equal priority, queue this one
                false
            }
        }
    };
    
    if should_show_immediately {
        log::info!("Showing dialog immediately: {}", dialog_request.id);
        *active = Some(dialog_request.clone());
        Ok(dialog_request)
    } else {
        log::info!("Queueing dialog for later: {}", dialog_request.id);
        queue.push(dialog_request.clone());
        // Return the currently active dialog to indicate what should be shown
        Ok(active.as_ref().unwrap().clone())
    }
}

/// Get the next dialog to show from the queue
#[tauri::command]
pub fn get_next_dialog() -> Result<Option<DialogRequest>, String> {
    let mut queue = DIALOG_QUEUE.lock().map_err(|_| "Failed to lock dialog queue".to_string())?;
    let mut active = ACTIVE_DIALOG.lock().map_err(|_| "Failed to lock active dialog".to_string())?;
    
    if let Some(next_dialog) = queue.pop() {
        log::info!("Next dialog from queue: {}", next_dialog.id);
        *active = Some(next_dialog.clone());
        Ok(Some(next_dialog))
    } else {
        log::info!("No more dialogs in queue");
        *active = None;
        Ok(None)
    }
}

/// Remove a dialog from the queue or mark active as complete
#[tauri::command]
pub fn complete_dialog(dialog_id: String) -> Result<Option<DialogRequest>, String> {
    log::info!("Completing dialog: {}", dialog_id);
    
    let mut queue = DIALOG_QUEUE.lock().map_err(|_| "Failed to lock dialog queue".to_string())?;
    let mut active = ACTIVE_DIALOG.lock().map_err(|_| "Failed to lock active dialog".to_string())?;
    
    // Check if this is the active dialog
    if let Some(ref current) = *active {
        if current.id == dialog_id {
            // Clear active dialog and get next from queue
            *active = None;
            if let Some(next_dialog) = queue.pop() {
                log::info!("Activating next dialog from queue: {}", next_dialog.id);
                *active = Some(next_dialog.clone());
                return Ok(Some(next_dialog));
            } else {
                log::info!("No more dialogs in queue after completing: {}", dialog_id);
                return Ok(None);
            }
        }
    }
    
    // Remove from queue if it's there
    let mut temp_queue = BinaryHeap::new();
    let mut found = false;
    
    while let Some(dialog) = queue.pop() {
        if dialog.id != dialog_id {
            temp_queue.push(dialog);
        } else {
            found = true;
            log::info!("Removed dialog from queue: {}", dialog_id);
        }
    }
    
    *queue = temp_queue;
    
    if found {
        Ok(active.clone())
    } else {
        Err(format!("Dialog not found: {}", dialog_id))
    }
}

/// Get current dialog queue status
#[tauri::command]
pub fn get_dialog_queue_status() -> Result<(Option<DialogRequest>, Vec<DialogRequest>), String> {
    let queue = DIALOG_QUEUE.lock().map_err(|_| "Failed to lock dialog queue".to_string())?;
    let active = ACTIVE_DIALOG.lock().map_err(|_| "Failed to lock active dialog".to_string())?;
    
    let queue_items: Vec<DialogRequest> = queue.clone().into_sorted_vec();
    
    Ok((active.clone(), queue_items))
}

// ========== Session-based Transport Manager ===========

// Active PIN sessions - tracks devices currently in PIN creation flow
lazy_static::lazy_static! {
    static ref ACTIVE_PIN_DEVICES: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
}

// Active recovery sessions - tracks devices currently in recovery flow
lazy_static::lazy_static! {
    static ref ACTIVE_RECOVERY_DEVICES: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
}

/// Mark device as being in PIN creation flow
fn mark_device_in_pin_flow(device_id: &str) -> Result<(), String> {
    let mut devices = ACTIVE_PIN_DEVICES.lock().map_err(|_| "Failed to lock active PIN devices".to_string())?;
    devices.insert(device_id.to_string());
    log::info!("Device {} marked as in PIN flow", device_id);
    Ok(())
}

/// Check if device is in PIN creation flow
fn is_device_in_pin_flow(device_id: &str) -> bool {
    ACTIVE_PIN_DEVICES.lock()
        .map(|devices| devices.contains(device_id))
        .unwrap_or(false)
}

/// Remove device from PIN creation flow tracking
fn unmark_device_in_pin_flow(device_id: &str) -> Result<(), String> {
    let mut devices = ACTIVE_PIN_DEVICES.lock().map_err(|_| "Failed to lock active PIN devices".to_string())?;
    devices.remove(device_id);
    log::info!("Device {} removed from PIN flow", device_id);
    Ok(())
}

/// Mark device as being in recovery flow
fn mark_device_in_recovery_flow(device_id: &str) -> Result<(), String> {
    let mut devices = ACTIVE_RECOVERY_DEVICES.lock().map_err(|_| "Failed to lock active recovery devices".to_string())?;
    devices.insert(device_id.to_string());
    log::info!("Device {} marked as in recovery flow", device_id);
    Ok(())
}

/// Check if device is in recovery flow
fn is_device_in_recovery_flow(device_id: &str) -> bool {
    ACTIVE_RECOVERY_DEVICES.lock()
        .map(|devices| devices.contains(device_id))
        .unwrap_or(false)
}

/// Remove device from recovery flow tracking
fn unmark_device_in_recovery_flow(device_id: &str) -> Result<(), String> {
    let mut devices = ACTIVE_RECOVERY_DEVICES.lock().map_err(|_| "Failed to lock active recovery devices".to_string())?;
    devices.remove(device_id);
    log::info!("Device {} removed from recovery flow", device_id);
    Ok(())
}

/// Create transport for device (tries USB then HID)
async fn create_device_transport(target_device: &FriendlyUsbDevice) -> Result<Box<dyn crate::transport::ProtocolAdapter>, String> {
    // Find physical device for transport
    let devices = crate::features::list_devices();
    let physical_device = if let Some(serial) = &target_device.serial_number {
        // Match by serial number
        devices.iter().find(|d| {
            if let Ok(handle) = d.open() {
                let timeout = std::time::Duration::from_millis(100);
                if let Ok(langs) = handle.read_languages(timeout) {
                    if let Some(lang) = langs.first() {
                        if let Ok(desc) = d.device_descriptor() {
                            if let Ok(device_serial) = handle.read_serial_number_string(*lang, &desc, timeout) {
                                return device_serial == *serial;
                            }
                        }
                    }
                }
            }
            false
        }).cloned()
    } else {
        // Try to parse bus and address from unique_id
        let parts: Vec<&str> = target_device.unique_id.split('_').collect();
        if parts.len() >= 2 {
            let bus_str = parts[0].strip_prefix("bus").unwrap_or("");
            let addr_str = parts[1].strip_prefix("addr").unwrap_or("");
            
            if let (Ok(bus), Ok(addr)) = (bus_str.parse::<u8>(), addr_str.parse::<u8>()) {
                devices.iter().find(|d| d.bus_number() == bus && d.address() == addr).cloned()
            } else {
                None
            }
        } else {
            None
        }
    };
    
    match physical_device {
        Some(device) => {
            // Try USB transport first
            match crate::transport::UsbTransport::new(&device, 0) {
                Ok((transport, _, _)) => {
                    log::info!("Created USB transport for device {}", target_device.unique_id);
                    Ok(Box::new(transport))
                }
                Err(usb_err) => {
                    log::warn!("USB transport failed for device {}: {}, trying HID fallback", target_device.unique_id, usb_err);
                    
                    // Try HID fallback
                    match crate::transport::HidTransport::new_for_device(target_device.serial_number.as_deref()) {
                        Ok(hid_transport) => {
                            log::info!("Created HID transport for device {}", target_device.unique_id);
                            Ok(Box::new(hid_transport))
                        }
                        Err(hid_err) => {
                            Err(format!("Failed with both USB ({}) and HID ({})", usb_err, hid_err))
                        }
                    }
                }
            }
        }
        None => {
            Err(format!("Physical device not found for {}", target_device.unique_id))
        }
    }
}

/// Send message to device (creates transport on-demand)
async fn send_message_to_device(device_id: &str, message: crate::messages::Message) -> Result<crate::messages::Message, String> {
    log::info!("Sending message to device: {} (type: {:?})", device_id, message.message_type());
    
    // Check if device is in special flow modes that require raw transport access
    let in_pin_flow = is_device_in_pin_flow(device_id);
    let in_recovery_flow = is_device_in_recovery_flow(device_id);
    
    // For special flows or when queue is not available, use direct transport
    if in_pin_flow || in_recovery_flow {
        log::info!("Device {} is in special flow, using direct transport", device_id);
        
        // Get device entry
        let entries = device_registry::get_all_device_entries()
            .map_err(|e| format!("Failed to get device entries: {}", e))?;
        
        let target_device = entries.iter()
            .find(|entry| entry.device.unique_id == device_id)
            .ok_or_else(|| format!("Device not found: {}", device_id))?;
        
        // Create transport
        let mut transport = create_device_transport(&target_device.device).await?;
        
        if in_recovery_flow {
            log::info!("Device {} is in recovery flow, using recovery flow handler", device_id);
            // Use recovery flow handler that handles ButtonRequest but passes through CharacterRequest
            let mut handler = transport.with_recovery_flow_handler();
            handler.handle(message).map_err(|e| format!("Failed to send message: {}", e))
        } else if in_pin_flow {
            log::info!("Device {} is in PIN flow, using PIN flow handler", device_id);
            // Use PIN flow handler that handles ButtonRequest but passes through PinMatrixRequest
            let mut handler = transport.with_pin_flow_handler();
            handler.handle(message).map_err(|e| format!("Failed to send message: {}", e))
        } else {
            log::info!("Device {} is not in special flow, using standard handler for automatic responses", device_id);
            // Use standard handler for automatic button/pin handling
            let mut handler = transport.with_standard_handler();
            handler.handle(message).map_err(|e| format!("Failed to send message: {}", e))
        }
    } else {
        // Try to use device queue first (preferred method)
        log::info!("Device {} not in special flow, trying device queue", device_id);
        
        match send_message_via_queue(Some(device_id), message.clone()).await {
            Ok(response) => {
                log::info!("✅ Message sent successfully via device queue");
                Ok(response)
            }
            Err(queue_err) => {
                log::warn!("Queue communication failed: {}, falling back to direct transport", queue_err);
                
                // Fallback to direct transport creation (legacy behavior)
                let entries = device_registry::get_all_device_entries()
                    .map_err(|e| format!("Failed to get device entries: {}", e))?;
                
                let target_device = entries.iter()
                    .find(|entry| entry.device.unique_id == device_id)
                    .ok_or_else(|| format!("Device not found: {}", device_id))?;
                
                // Create transport
                let mut transport = create_device_transport(&target_device.device).await?;
                
                log::info!("Using fallback transport with standard handler");
                let mut handler = transport.with_standard_handler();
                handler.handle(message).map_err(|e| format!("Failed to send message via fallback: {}", e))
            }
        }
    }
}

// ========== Recovery Commands ==========

/// Start device recovery process
#[tauri::command]
pub async fn start_device_recovery(
    device_id: String,
    word_count: u32,
    passphrase_protection: bool,
    label: String,
) -> Result<RecoverySession, String> {
    log::info!("Starting recovery for device: {} with {} words", device_id, word_count);
    
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
    
    // Create RecoveryDevice message
    let recovery_device = crate::messages::RecoveryDevice {
        word_count: Some(word_count),
        passphrase_protection: Some(passphrase_protection),
        pin_protection: Some(true),  // Always use PIN
        language: Some("english".to_string()),
        label: Some(label),
        enforce_wordlist: Some(true),
        use_character_cipher: Some(true),  // Use scrambled keyboard
        auto_lock_delay_ms: Some(600000),  // 10 minutes
        u2f_counter: Some((std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() / 1000) as u32),
        dry_run: Some(false),
    };
    
    // Send RecoveryDevice message
    match send_message_to_device(&device_id, recovery_device.into()).await {
        Ok(response) => {
            log::info!("RecoveryDevice sent, response: {:?}", response.message_type());
            
            match response {
                crate::messages::Message::PinMatrixRequest(_) => {
                    // Expected - device wants PIN setup
                    log::info!("Device requesting PIN setup for recovery");
                    Ok(session)
                }
                crate::messages::Message::CharacterRequest(req) => {
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
                crate::messages::Message::ButtonRequest(_) => {
                    // Device needs user confirmation
                    log::info!("Device requesting button press for recovery");
                    Ok(session)
                }
                crate::messages::Message::Failure(f) => {
                    // Clean up session only on actual device failure
                    if let Ok(mut sessions) = RECOVERY_SESSIONS.lock() {
                        sessions.remove(&session_id);
                    }
                    let _ = unmark_device_in_recovery_flow(&device_id);
                    Err(format!("Device rejected recovery: {}", f.message()))
                }
                _ => {
                    log::warn!("Unexpected response to RecoveryDevice: {:?}", response.message_type());
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
    
    // Create CharacterAck message
    let character_ack = match action {
        Some(RecoveryAction::Done) => {
            crate::messages::CharacterAck {
                character: None,
                delete: Some(false),
                done: Some(true),
            }
        }
        Some(RecoveryAction::Delete) => {
            crate::messages::CharacterAck {
                character: None,
                delete: Some(true),
                done: Some(false),
            }
        }
        Some(RecoveryAction::Space) => {
            crate::messages::CharacterAck {
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
                
                crate::messages::CharacterAck {
                    character: Some(ch.to_lowercase()),
                    delete: Some(false),
                    done: Some(false),
                }
            } else {
                return Err("No character or action provided".to_string());
            }
        }
    };
    
    // Send message
    match send_message_to_device(&device_id, character_ack.into()).await {
        Ok(response) => {
            match response {
                crate::messages::Message::CharacterRequest(req) => {
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
                crate::messages::Message::Success(_) => {
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
                crate::messages::Message::Failure(f) => {
                    // Mark session as failed
                    if let Ok(mut sessions) = RECOVERY_SESSIONS.lock() {
                        if let Some(session) = sessions.get_mut(&session_id) {
                            session.is_active = false;
                        }
                    }
                    
                    // Remove from recovery flow
                    let _ = unmark_device_in_recovery_flow(&device_id);
                    
                    Err(format!("Recovery failed: {}", f.message()))
                }
                _ => {
                    Err(format!("Unexpected response: {:?}", response.message_type()))
                }
            }
        }
        Err(e) => {
            Err(format!("Failed to send character: {}", e))
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

/// Send PIN matrix response during recovery flow
#[tauri::command]
pub async fn send_recovery_pin_response(
    session_id: String,
    positions: Vec<u8>  // Positions 1-9 that user clicked
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
    
    // Convert positions to PIN string for device protocol
    let pin_string: String = positions.iter()
        .map(|&pos| (b'0' + pos) as char)
        .collect();
    
    log::info!("Converted positions to PIN string for recovery PIN: {}", pin_string);
    
    // Create PinMatrixAck message
    let pin_matrix_ack = crate::messages::PinMatrixAck {
        pin: pin_string.clone(),
    };
    
    // Send message to device
    match send_message_to_device(&device_id, pin_matrix_ack.into()).await {
        Ok(response) => {
            log::info!("Recovery PIN sent successfully: {:?}", response.message_type());
            
            match response {
                crate::messages::Message::PinMatrixRequest(_) => {
                    // Device wants PIN confirmation
                    Ok(RecoveryProgress {
                        word_pos: current_word,
                        character_pos: current_char,
                        auto_completed: false,
                        is_complete: false,
                        error: Some("pin_confirm".to_string()), // Special signal for PIN confirmation
                    })
                }
                crate::messages::Message::ButtonRequest(_) => {
                    // Device needs button confirmation
                    Ok(RecoveryProgress {
                        word_pos: current_word,
                        character_pos: current_char,
                        auto_completed: false,
                        is_complete: false,
                        error: Some("button_confirm".to_string()), // Special signal for button confirmation
                    })
                }
                crate::messages::Message::CharacterRequest(req) => {
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
                crate::messages::Message::Success(_) => {
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
                crate::messages::Message::Failure(f) => {
                    Err(format!("Recovery PIN failed: {}", f.message()))
                }
                _ => {
                    Err(format!("Unexpected response to recovery PIN: {:?}", response.message_type()))
                }
            }
        }
        Err(e) => {
            Err(format!("Failed to send recovery PIN: {}", e))
        }
    }
}

/// Cancel recovery session
#[tauri::command]  
pub async fn cancel_recovery_session(session_id: String) -> Result<bool, String> {
    log::info!("Cancelling recovery session: {}", session_id);
    
    let mut sessions = RECOVERY_SESSIONS.lock()
        .map_err(|_| "Failed to lock recovery sessions".to_string())?;
    
    if let Some(mut session) = sessions.remove(&session_id) {
        let device_id = session.device_id.clone();
        session.is_active = false;
        
        // Remove from recovery flow
        let _ = unmark_device_in_recovery_flow(&device_id);
        
        // Send cancel message to device if needed
        // Note: The device might need a Cancel message to exit recovery mode
        
        Ok(true)
    } else {
        Ok(false)
    }
}

// ========== Seed Verification Commands (Dry Run Recovery) ==========

/// Seed verification session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedVerificationSession {
    pub session_id: String,
    pub device_id: String,
    pub word_count: u32,
    pub current_word: u32,
    pub current_character: u32,
    pub is_active: bool,
    pub pin_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedVerificationResult {
    pub verified: bool,
    pub message: String,
}

// Global seed verification sessions
lazy_static::lazy_static! {
    static ref VERIFICATION_SESSIONS: Mutex<HashMap<String, SeedVerificationSession>> = 
        Mutex::new(HashMap::new());
}

/// Start seed verification process (dry run recovery)
#[tauri::command]
pub async fn start_seed_verification(
    device_id: String,
    word_count: u32,
) -> Result<SeedVerificationSession, String> {
    log::info!("Starting seed verification (dry run) for device: {} with {} words", device_id, word_count);
    
    // Check if device is initialized
    let features = device_registry::get_device_features(&device_id)?
        .ok_or_else(|| "Device features not available".to_string())?;
    
    if !features.initialized {
        return Err("Device is not initialized. Cannot verify seed on uninitialized device.".to_string());
    }
    
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
    
    // Create RecoveryDevice message with dry_run = true
    let recovery_device = crate::messages::RecoveryDevice {
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
    match send_message_to_device(&device_id, recovery_device.into()).await {
        Ok(response) => {
            log::info!("Dry run RecoveryDevice sent, response: {:?}", response.message_type());
            
            match response {
                crate::messages::Message::PinMatrixRequest(pmr) => {
                    // Expected - device wants PIN verification first
                    log::info!("Device requesting PIN for seed verification, type: {:?}", pmr.r#type);
                    Ok(session)
                }
                crate::messages::Message::CharacterRequest(req) => {
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
                crate::messages::Message::Failure(f) => {
                    // Clean up on failure
                    if let Ok(mut sessions) = VERIFICATION_SESSIONS.lock() {
                        sessions.remove(&session_id);
                    }
                    let _ = unmark_device_in_recovery_flow(&device_id);
                    Err(format!("Device rejected seed verification: {}", f.message()))
                }
                _ => {
                    log::warn!("Unexpected response to dry run RecoveryDevice: {:?}", response.message_type());
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

/// Send PIN for seed verification
#[tauri::command]
pub async fn send_verification_pin(
    session_id: String,
    positions: Vec<u8>  // Positions 1-9 that user clicked
) -> Result<bool, String> {
    log::info!("Sending PIN for seed verification session: {}", session_id);
    
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
    let device_id = {
        let mut sessions = VERIFICATION_SESSIONS.lock()
            .map_err(|_| "Failed to lock verification sessions".to_string())?;
        
        let session = sessions.get_mut(&session_id)
            .ok_or_else(|| "Verification session not found".to_string())?;
        
        if !session.is_active {
            return Err("Verification session is not active".to_string());
        }
        
        session.device_id.clone()
    };
    
    // Convert positions to PIN string
    let pin_string: String = positions.iter()
        .map(|&pos| (b'0' + pos) as char)
        .collect();
    
    // Create PinMatrixAck message
    let pin_matrix_ack = crate::messages::PinMatrixAck {
        pin: pin_string,
    };
    
    // Send message to device
    match send_message_to_device(&device_id, pin_matrix_ack.into()).await {
        Ok(response) => {
            match response {
                crate::messages::Message::CharacterRequest(req) => {
                    // PIN accepted, ready for seed input
                    if let Ok(mut sessions) = VERIFICATION_SESSIONS.lock() {
                        if let Some(session) = sessions.get_mut(&session_id) {
                            session.pin_verified = true;
                            session.current_word = req.word_pos;
                            session.current_character = req.character_pos;
                        }
                    }
                    Ok(true)
                }
                crate::messages::Message::Failure(f) => {
                    Err(format!("Invalid PIN: {}", f.message()))
                }
                _ => {
                    Err(format!("Unexpected response to PIN: {:?}", response.message_type()))
                }
            }
        }
        Err(e) => {
            Err(format!("Failed to send PIN: {}", e))
        }
    }
}

/// Send character for seed verification
#[tauri::command]
pub async fn send_verification_character(
    session_id: String,
    character: Option<String>,
    action: Option<RecoveryAction>,
) -> Result<RecoveryProgress, String> {
    log::info!("Sending verification character for session: {} - char: {:?}, action: {:?}", 
        session_id, character, action);
    
    // Get session
    let (device_id, current_word, current_char) = {
        let sessions = VERIFICATION_SESSIONS.lock()
            .map_err(|_| "Failed to lock verification sessions".to_string())?;
        
        let session = sessions.get(&session_id)
            .ok_or_else(|| "Verification session not found".to_string())?;
        
        if !session.is_active {
            return Err("Verification session is not active".to_string());
        }
        
        if !session.pin_verified {
            return Err("PIN not verified yet".to_string());
        }
        
        (session.device_id.clone(), session.current_word, session.current_character)
    };
    
    // Create CharacterAck message (same as recovery)
    let character_ack = match action {
        Some(RecoveryAction::Done) => {
            crate::messages::CharacterAck {
                character: None,
                delete: Some(false),
                done: Some(true),
            }
        }
        Some(RecoveryAction::Delete) => {
            crate::messages::CharacterAck {
                character: None,
                delete: Some(true),
                done: Some(false),
            }
        }
        Some(RecoveryAction::Space) => {
            crate::messages::CharacterAck {
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
                
                crate::messages::CharacterAck {
                    character: Some(ch.to_lowercase()),
                    delete: Some(false),
                    done: Some(false),
                }
            } else {
                return Err("No character or action provided".to_string());
            }
        }
    };
    
    // Send message
    match send_message_to_device(&device_id, character_ack.into()).await {
        Ok(response) => {
            match response {
                crate::messages::Message::CharacterRequest(req) => {
                    // Update session state
                    if let Ok(mut sessions) = VERIFICATION_SESSIONS.lock() {
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
                crate::messages::Message::Success(s) => {
                    // Verification successful - seed matches!
                    log::info!("✅ Seed verification successful: {}", s.message());
                    
                    // Clean up session
                    if let Ok(mut sessions) = VERIFICATION_SESSIONS.lock() {
                        sessions.remove(&session_id);
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
                crate::messages::Message::Failure(f) => {
                    // Verification failed - seed doesn't match
                    log::warn!("❌ Seed verification failed: {}", f.message());
                    
                    // Clean up session
                    if let Ok(mut sessions) = VERIFICATION_SESSIONS.lock() {
                        sessions.remove(&session_id);
                    }
                    let _ = unmark_device_in_recovery_flow(&device_id);
                    
                    // Return as complete but with error to indicate mismatch
                    Ok(RecoveryProgress {
                        word_pos: current_word,
                        character_pos: current_char,
                        auto_completed: false,
                        is_complete: true,
                        error: Some(f.message().to_string()),
                    })
                }
                _ => {
                    Err(format!("Unexpected response: {:?}", response.message_type()))
                }
            }
        }
        Err(e) => {
            Err(format!("Failed to send character: {}", e))
        }
    }
}

/// Get verification session status
#[tauri::command]
pub async fn get_verification_status(session_id: String) -> Result<Option<SeedVerificationSession>, String> {
    let sessions = VERIFICATION_SESSIONS.lock()
        .map_err(|_| "Failed to lock verification sessions".to_string())?;
    
    Ok(sessions.get(&session_id).cloned())
}

/// Cancel seed verification
#[tauri::command]
pub async fn cancel_seed_verification(session_id: String) -> Result<bool, String> {
    log::info!("Cancelling seed verification session: {}", session_id);
    
    let device_id = {
        let mut sessions = VERIFICATION_SESSIONS.lock()
            .map_err(|_| "Failed to lock verification sessions".to_string())?;
        
        if let Some(session) = sessions.remove(&session_id) {
            let device_id = session.device_id.clone();
            
            // Remove from recovery flow
            let _ = unmark_device_in_recovery_flow(&device_id);
            
            Some(device_id)
        } else {
            None
        }
    }; // Drop the mutex guard here
    
    if let Some(device_id) = device_id {
        // Send Cancel message to device to exit dry run mode
        let cancel_msg = crate::messages::Cancel {};
        let _ = send_message_to_device(&device_id, cancel_msg.into()).await;
        
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Force cleanup seed verification by device ID (when session is unknown)
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
    
    // Send Cancel message to device to exit any dry run mode
    let cancel_msg = crate::messages::Cancel {};
    match send_message_to_device(&device_id, cancel_msg.into()).await {
        Ok(_) => {
            log::info!("Cancel message sent to device {}", device_id);
        }
        Err(e) => {
            log::warn!("Failed to send cancel to device {}: {}", device_id, e);
            // Don't fail the cleanup - device might not be in a state to receive messages
        }
    }
    
    Ok(cleanup_done)
}

/// Get device queue handle for a specific device, creating one if needed
async fn get_device_queue_or_fallback(device_id: &str) -> Result<DeviceQueueHandle, String> {
    log::info!("Getting device queue handle for: {}", device_id);
    
    // Get all device entries from the registry to find the target device
    let entries = device_registry::get_all_device_entries()
        .map_err(|e| format!("Failed to get device entries: {}", e))?;
    
    // Find the device in the registry
    let device_entry = entries.iter()
        .find(|e| e.device.unique_id == device_id)
        .ok_or_else(|| format!("Device {} not found in registry", device_id))?;
    
    // Create a new queue handle using the device queue factory
    let queue_handle = crate::device_queue::DeviceQueueFactory::spawn_worker(
        device_id.to_string(), 
        device_entry.device.clone()
    );
    
    log::info!("Created device queue handle for: {}", device_id);
    Ok(queue_handle)
}

/// Send a raw message to device via queue system
async fn send_message_via_queue(device_id: Option<&str>, message: crate::messages::Message) -> Result<crate::messages::Message, String> {
    if let Some(device_id) = device_id {
        let queue_handle = get_device_queue_or_fallback(device_id).await?;
        queue_handle.send_raw(message, false).await
            .map_err(|e| format!("Queue communication failed: {}", e))
    } else {
        // If no device ID specified, try to use first available device
        let entries = device_registry::get_all_device_entries()
            .map_err(|e| format!("Failed to get device entries: {}", e))?;
        
        if let Some(entry) = entries.first() {
            let queue_handle = get_device_queue_or_fallback(&entry.device.unique_id).await?;
            queue_handle.send_raw(message, false).await
                .map_err(|e| format!("Queue communication failed: {}", e))
        } else {
            Err("No devices available".to_string())
        }
    }
}

// ========== Wallet Context Commands (vault-v2 pattern adapted for vault v1) ==========

#[command]
pub async fn get_required_paths() -> Result<Vec<RequiredPath>, String> {
    log::info!("📋 Getting required derivation paths for wallet");
    Ok(IndexDb::get_required_paths())
}

#[command]
pub async fn get_wallet_xpubs(
    device_id: Option<String>,
) -> Result<Vec<WalletXpub>, String> {
    log::info!("📚 Getting wallet xpubs{}", 
               device_id.as_ref().map_or_else(|| " (all devices)".to_string(), |id| format!(" for device: {}", id)));
    
    let db = IndexDb::open().map_err(|e| {
        log::error!("Failed to open database: {}", e);
        format!("Database error: {}", e)
    })?;

    let xpubs = if let Some(device_id) = device_id {
        db.get_wallet_xpubs(&device_id)
    } else {
        db.get_all_wallet_xpubs()
    }.map_err(|e| {
        log::error!("Failed to get wallet xpubs: {}", e);
        format!("Failed to get wallet xpubs: {}", e)
    })?;

    log::info!("Found {} wallet xpubs", xpubs.len());
    Ok(xpubs)
}

#[command]
pub async fn sync_device_xpubs(
    device_id: String,
    app_handle: tauri::AppHandle,
) -> Result<Vec<WalletXpub>, String> {
    log::info!("🔄 Starting xpub sync for device: {}", device_id);
    
    // Get required derivation paths
    let required_paths = IndexDb::get_required_paths();
    
    // Create database connection
    let db = IndexDb::open().map_err(|e| {
        log::error!("Failed to open database: {}", e);
        format!("Database error: {}", e)
    })?;
    
    // Get device queue handle from device registry
    let queue_handle = match get_device_queue_or_fallback(&device_id).await {
        Ok(handle) => handle,
        Err(e) => {
            log::error!("Failed to get device queue for {}: {}", device_id, e);
            return Err(format!("Device queue unavailable: {}", e));
        }
    };
    
    for path_info in required_paths {
        log::info!("📡 Requesting xpub for {} ({})", path_info.path, path_info.label);
        
        // Parse derivation path to vector
        let derivation_path = crate::utils::parse_derivation_path(&path_info.path).map_err(|e| {
            log::error!("Failed to parse derivation path {}: {}", path_info.path, e);
            format!("Invalid derivation path: {}", e)
        })?;
        
        // Use the device queue to get address (which contains xpub info for account level)
        match queue_handle.get_address(derivation_path, "Bitcoin".to_string(), None).await {
            Ok(response) => {
                log::info!("✅ Got response for {}: {}", path_info.path, response);
                
                // For now, use the response as xpub (this will need improvement for real xpub extraction)
                // Store in database
                if let Err(e) = db.insert_xpub_from_queue(&device_id, &path_info.path, &response) {
                    log::error!("Failed to store xpub in database: {}", e);
                    return Err(format!("Failed to store xpub: {}", e));
                }
                
                // Emit progress event
                let _ = app_handle.emit("wallet-sync-progress", serde_json::json!({
                    "device_id": device_id,
                    "path": path_info.path,
                    "label": path_info.label,
                    "status": "completed",
                    "xpub": response
                }));
                
            },
            Err(e) => {
                log::error!("❌ Failed to get xpub for {}: {}", path_info.path, e);
                
                // Emit error event
                let _ = app_handle.emit("wallet-sync-progress", serde_json::json!({
                    "device_id": device_id,
                    "path": path_info.path,
                    "label": path_info.label,
                    "status": "error",
                    "error": e.to_string()
                }));
                
                return Err(format!("Failed to get xpub for {}: {}", path_info.path, e));
            }
        }
    }
    
    // Get all xpubs for this device from database
    let xpubs = db.get_wallet_xpubs(&device_id).map_err(|e| {
        log::error!("Failed to get stored xpubs: {}", e);
        format!("Failed to get stored xpubs: {}", e)
    })?;
    
    log::info!("🎉 Sync completed! Stored {} xpubs for device {}", xpubs.len(), device_id);
    
    // Emit completion event
    let _ = app_handle.emit("wallet-sync-completed", serde_json::json!({
        "device_id": device_id,
        "xpubs": xpubs
    }));
    
    Ok(xpubs)
}

#[command]
pub async fn get_portfolio_cache() -> Result<Vec<PortfolioCache>, String> {
    log::info!("💰 Getting portfolio cache");
    
    let db = IndexDb::open().map_err(|e| {
        log::error!("Failed to open database: {}", e);
        format!("Database error: {}", e)
    })?;

    let cache = db.get_portfolio_cache().map_err(|e| {
        log::error!("Failed to get portfolio cache: {}", e);
        format!("Failed to get portfolio cache: {}", e)
    })?;

    log::info!("Found {} cached portfolio entries", cache.len());
    Ok(cache)
}

#[command]
pub async fn refresh_portfolio(
    force: Option<bool>,
    app_handle: tauri::AppHandle,
) -> Result<Vec<PortfolioCache>, String> {
    log::info!("🔄 Refreshing portfolio (force: {})", force.unwrap_or(false));
    
    let db = IndexDb::open().map_err(|e| {
        log::error!("Failed to open database: {}", e);
        format!("Database error: {}", e)
    })?;

    // Check if cache is expired (unless force refresh)
    if !force.unwrap_or(false) {
        match db.is_cache_expired(10) { // 10 minutes TTL
            Ok(false) => {
                log::info!("Cache is still fresh, returning cached data");
                return get_portfolio_cache().await;
            },
            Ok(true) => {
                log::info!("Cache is expired, fetching fresh data");
            },
            Err(e) => {
                log::warn!("Failed to check cache expiry: {}, proceeding with refresh", e);
            }
        }
    }

    // Get all wallet xpubs
    let xpubs = db.get_all_wallet_xpubs().map_err(|e| {
        log::error!("Failed to get wallet xpubs: {}", e);
        format!("Failed to get wallet xpubs: {}", e)
    })?;

    if xpubs.is_empty() {
        log::warn!("No wallet xpubs found, cannot refresh portfolio");
        return Ok(vec![]);
    }

    log::info!("Fetching portfolio data for {} xpubs", xpubs.len());

    let mut portfolio_data = Vec::new();
    
    // Emit progress event
    let _ = app_handle.emit("portfolio-refresh-progress", serde_json::json!({
        "status": "fetching",
        "total": xpubs.len(),
        "completed": 0
    }));

    // For now, create mock data since we don't have Pioneer client
    // TODO: Implement real balance fetching from external API
    for (index, xpub) in xpubs.iter().enumerate() {
        log::info!("📡 Mock fetching balance for {} ({})", xpub.label, &xpub.pubkey[0..20]);
        
        // Mock balance data (replace with real API call)
        let mock_balance = "0.00000000";
        let mock_usd_value = "0.00";
        let mock_price = "50000.00";
        
        portfolio_data.push(PortfolioCacheInput {
            pubkey: xpub.pubkey.clone(),
            caip: xpub.caip.clone(),
            balance: mock_balance.to_string(),
            balance_usd: mock_usd_value.to_string(),
            price_usd: mock_price.to_string(),
            symbol: Some("BTC".to_string()),
        });
        
        // Emit progress event
        let _ = app_handle.emit("portfolio-refresh-progress", serde_json::json!({
            "status": "fetching",
            "total": xpubs.len(),
            "completed": index + 1,
            "current": xpub.label
        }));
    }

    // Cache the results
    if !portfolio_data.is_empty() {
        if let Err(e) = db.cache_portfolio_data(&portfolio_data) {
            log::error!("Failed to cache portfolio data: {}", e);
            return Err(format!("Failed to cache portfolio data: {}", e));
        }
    }

    // Get the cached data to return
    let cached_data = db.get_portfolio_cache().map_err(|e| {
        log::error!("Failed to get cached portfolio data: {}", e);
        format!("Failed to get cached portfolio data: {}", e)
    })?;

    log::info!("🎉 Portfolio refresh completed! Cached {} entries", cached_data.len());
    
    // Emit completion event
    let _ = app_handle.emit("portfolio-refresh-completed", serde_json::json!({
        "status": "completed",
        "entries": cached_data.len(),
        "data": cached_data
    }));

    Ok(cached_data)
}

#[command]
pub async fn clear_portfolio_cache() -> Result<(), String> {
    log::info!("🧹 Clearing portfolio cache");
    
    let db = IndexDb::open().map_err(|e| {
        log::error!("Failed to open database: {}", e);
        format!("Database error: {}", e)
    })?;

    db.clear_portfolio_cache().map_err(|e| {
        log::error!("Failed to clear portfolio cache: {}", e);
        format!("Failed to clear portfolio cache: {}", e)
    })
}

#[command]
pub async fn get_fee_rates(
    caip: String,
) -> Result<Option<FeeRateCache>, String> {
    log::info!("💸 Getting fee rates for: {}", caip);
    
    let db = IndexDb::open().map_err(|e| {
        log::error!("Failed to open database: {}", e);
        format!("Database error: {}", e)
    })?;

    let fee_rates = db.get_fee_rates(&caip).map_err(|e| {
        log::error!("Failed to get fee rates: {}", e);
        format!("Failed to get fee rates: {}", e)
    })?;

    if let Some(ref rates) = fee_rates {
        log::info!("Found cached fee rates: fastest={}, fast={}, average={}", 
                   rates.fastest, rates.fast, rates.average);
    } else {
        log::info!("No cached fee rates found for {}", caip);
    }

    Ok(fee_rates)
}

#[command]
pub async fn get_wallet_summary() -> Result<serde_json::Value, String> {
    log::info!("📊 Getting wallet summary");
    
    let db = IndexDb::open().map_err(|e| {
        log::error!("Failed to open database: {}", e);
        format!("Database error: {}", e)
    })?;

    // Get xpubs and portfolio data
    let xpubs = db.get_all_wallet_xpubs().map_err(|e| {
        log::error!("Failed to get wallet xpubs: {}", e);
        format!("Failed to get wallet xpubs: {}", e)
    })?;

    let portfolio = db.get_portfolio_cache().map_err(|e| {
        log::error!("Failed to get portfolio cache: {}", e);
        format!("Failed to get portfolio cache: {}", e)
    })?;

    // Calculate totals
    let total_balance_usd: f64 = portfolio.iter()
        .filter_map(|p| p.balance_usd.parse::<f64>().ok())
        .sum();

    let total_balance_btc: f64 = portfolio.iter()
        .filter_map(|p| p.balance.parse::<f64>().ok())
        .sum();

    // Group by device
    let mut devices_summary = HashMap::new();
    for xpub in &xpubs {
        let device_entry = devices_summary.entry(xpub.device_id.clone()).or_insert_with(|| {
            serde_json::json!({
                "device_id": xpub.device_id,
                "xpubs": [],
                "balance_btc": 0.0,
                "balance_usd": 0.0
            })
        });

        // Find matching portfolio entry
        if let Some(portfolio_entry) = portfolio.iter().find(|p| p.pubkey == xpub.pubkey) {
            let balance_btc = portfolio_entry.balance.parse::<f64>().unwrap_or(0.0);
            let balance_usd = portfolio_entry.balance_usd.parse::<f64>().unwrap_or(0.0);
            
            device_entry["balance_btc"] = serde_json::json!(
                device_entry["balance_btc"].as_f64().unwrap_or(0.0) + balance_btc
            );
            device_entry["balance_usd"] = serde_json::json!(
                device_entry["balance_usd"].as_f64().unwrap_or(0.0) + balance_usd
            );
        }

        device_entry["xpubs"].as_array_mut().unwrap().push(serde_json::json!({
            "path": xpub.path,
            "label": xpub.label,
            "pubkey": format!("{}...", &xpub.pubkey[0..20])
        }));
    }

    let summary = serde_json::json!({
        "total_balance_btc": total_balance_btc,
        "total_balance_usd": total_balance_usd,
        "total_xpubs": xpubs.len(),
        "devices": devices_summary.values().collect::<Vec<_>>(),
        "last_updated": portfolio.first().map(|p| p.last_updated).unwrap_or(0)
    });

    log::info!("Wallet summary: {} BTC (${:.2}), {} xpubs", 
               total_balance_btc, total_balance_usd, xpubs.len());

    Ok(summary)
}

/// Extract xpubs from device cache and populate wallet_xpubs table
/// This bridges the gap between vault v1's frontload system and our new WalletContext
#[command]
pub async fn extract_xpubs_from_cache(
    device_id: String,
    app_handle: tauri::AppHandle,
) -> Result<Vec<WalletXpub>, String> {
    log::info!("🔄 Extracting xpubs from device cache for: {}", device_id);
    
    // Open database
    let db = IndexDb::open().map_err(|e| {
        log::error!("Failed to open database: {}", e);
        format!("Database error: {}", e)
    })?;
    
    // Open device cache
    let cache = crate::cache::DeviceCache::open().map_err(|e| {
        log::error!("Failed to open device cache: {}", e);
        format!("Cache error: {}", e)
    })?;
    
    // Get all cached addresses for this device
    let addresses = cache.debug_load_addresses(&device_id).await.map_err(|e| {
        log::error!("Failed to get cached addresses: {}", e);
        format!("Failed to get cached addresses: {}", e)
    })?;
    
    log::info!("Found {} cached addresses to process", addresses.len());
    
    let mut xpubs_added = 0;
    let required_paths = IndexDb::get_required_paths();
    
    // Process each address looking for xpub entries
    for address in addresses {
        // Check if this is an xpub entry by looking for "_xpub" in the address
        if address.contains("_xpub/") {
            // Parse the format: "Bitcoin/p2pkh_xpub/xpub6CLbypuZz..."
            let parts: Vec<&str> = address.split('/').collect();
            if parts.len() >= 3 {
                let script_type_xpub = parts[1]; // e.g., "p2pkh_xpub"
                let xpub_value = parts[2]; // The actual xpub
                
                // Map script types to our required paths
                let (path, label) = match script_type_xpub {
                    "p2pkh_xpub" => ("m/44'/0'/0'", "Bitcoin Legacy"),
                    "p2sh-p2wpkh_xpub" => ("m/49'/0'/0'", "Bitcoin Segwit"),
                    "p2wpkh_xpub" => ("m/84'/0'/0'", "Bitcoin Native Segwit"),
                    _ => {
                        log::warn!("Unknown script type: {}", script_type_xpub);
                        continue;
                    }
                };
                
                // Find the matching required path to get the CAIP
                if let Some(path_info) = required_paths.iter().find(|p| p.path == path) {
                    log::info!("📝 Extracting xpub for {} ({}): {}...", path, label, &xpub_value[0..20]);
                    
                    // Store in database
                    if let Err(e) = db.insert_xpub_from_queue(&device_id, path, xpub_value) {
                        log::error!("Failed to store xpub: {}", e);
                        continue;
                    }
                    
                    xpubs_added += 1;
                    
                    // Emit progress event
                    let _ = app_handle.emit("xpub-extraction-progress", serde_json::json!({
                        "device_id": device_id,
                        "path": path,
                        "label": label,
                        "status": "completed",
                        "xpub": format!("{}...", &xpub_value[0..20])
                    }));
                }
            }
        }
    }
    
    // Get all xpubs for this device from database
    let xpubs = db.get_wallet_xpubs(&device_id).map_err(|e| {
        log::error!("Failed to get stored xpubs: {}", e);
        format!("Failed to get stored xpubs: {}", e)
    })?;
    
    log::info!("🎉 Extracted {} xpubs from cache! Total stored: {}", xpubs_added, xpubs.len());
    
    // Emit completion event
    let _ = app_handle.emit("xpub-extraction-completed", serde_json::json!({
        "device_id": device_id,
        "extracted": xpubs_added,
        "total": xpubs.len(),
        "xpubs": xpubs
    }));
    
    Ok(xpubs)
}

/// Auto-extract xpubs from cache when device becomes ready
/// This should be called automatically after frontload completes
#[command]
pub async fn auto_extract_xpubs_on_ready(
    device_id: String,
    app_handle: tauri::AppHandle,
) -> Result<bool, String> {
    log::info!("🔄 Auto-extracting xpubs for ready device: {}", device_id);
    
    // Check if we already have xpubs for this device
    let db = IndexDb::open().map_err(|e| {
        log::error!("Failed to open database: {}", e);
        format!("Database error: {}", e)
    })?;
    
    let existing_xpubs = db.get_wallet_xpubs(&device_id).map_err(|e| {
        log::error!("Failed to check existing xpubs: {}", e);
        format!("Failed to check existing xpubs: {}", e)
    })?;
    
    if !existing_xpubs.is_empty() {
        log::info!("Device {} already has {} xpubs, skipping extraction", device_id, existing_xpubs.len());
        return Ok(false);
    }
    
    // Extract xpubs from cache
    match extract_xpubs_from_cache(device_id.clone(), app_handle).await {
        Ok(xpubs) => {
            log::info!("✅ Auto-extracted {} xpubs for device {}", xpubs.len(), device_id);
            Ok(true)
        },
        Err(e) => {
            log::error!("❌ Auto-extraction failed for device {}: {}", device_id, e);
            Err(e)
        }
    }
}

