/*
      KeepKey GUI
            -Highlander
*/
// =============================================================
//  lib.rs – top‑level entry for the Tauri application backend
//  ----------------------------------------------------------
//  2025‑05‑31 refactor notes
//    • Keep this file minimal – only wiring + high‑level glue.
//    • USB watcher + cache consolidated in `start_usb_service`.
//    • Ready for extraction into `Backend` singleton in next pass.
// =============================================================

// ---------- Crate‑wide constants ----------
const TAG: &str = " | lib | ";

// ---------- Public modules ----------
pub mod messages;
pub mod transport;
pub mod commands;

pub mod features;
pub mod device_queue;
pub mod friendly_usb;




// ---------- Std / 3rd‑party ----------
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Emitter};
use futures::executor::block_on;

// UI payload
#[derive(serde::Serialize, Clone)]
struct ApplicationState {
    status: String,
    connected: bool,
    devices: Vec<device_registry::DeviceEntrySerializable>,
    blocking_actions_count: usize, // Count of blocking actions across all devices
}

// =============================================================
//  USB subsystem (consolidated)
// =============================================================
/// Boots the DeviceManager and DeviceController for background device management
fn start_usb_service(app_handle: &AppHandle, blocking_actions: blocking_actions::BlockingActionsState) -> Arc<Mutex<usb_manager::DeviceManager>> {
    // 1️⃣  Construct manager *before* spawning – avoids races.
    let mut device_manager = usb_manager::DeviceManager::new(app_handle.clone());
    
    // 2️⃣  Create and connect DeviceController
    let (controller, device_updates_tx, mut event_rx) = device_controller::DeviceController::new(blocking_actions.clone(), app_handle.clone());
    
    // Connect the DeviceManager to DeviceController
    device_manager.set_device_controller_tx(device_updates_tx);
    
    // Start listening for USB events
    if let Err(e) = device_manager.start_listening() {
        log::error!("Failed to start USB device listener: {e}");
    }
    
    let dm_arc = Arc::new(Mutex::new(device_manager));
    
    // 3️⃣  Start the DeviceController in the background
    tauri::async_runtime::spawn(async move {
        controller.run().await;
    });
    
    // 3️⃣  Listen for DeviceController events and emit to frontend
    let emitter = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            log::debug!("DeviceController event: {:?}", event);
            
            // Emit specific events based on type
            match event {
                device_controller::DeviceControllerEvent::DeviceConnected(device) => {
                    let _ = emitter.emit("device:connected", &device);
                }
                device_controller::DeviceControllerEvent::DeviceDisconnected(device_id) => {
                    let _ = emitter.emit("device:disconnected", &device_id);
                }
                device_controller::DeviceControllerEvent::FeaturesFetched { device_id, features, status } => {
                    // AGGRESSIVE DEBUG LOGGING FOR FRONTLOAD TRIGGER
                    log::info!("🔍 [FRONTLOAD DEBUG] Device features fetched for {}", device_id);
                    log::info!("🔍 [FRONTLOAD DEBUG] Device version: {}", features.version);
                    log::info!("🔍 [FRONTLOAD DEBUG] Device initialized: {}", features.initialized);
                    
                    // Get latest firmware version for comparison
                    let latest_firmware_result = device_update::get_latest_firmware_version();
                    log::info!("🔍 [FRONTLOAD DEBUG] Latest firmware result: {:?}", latest_firmware_result);
                    
                    // Check if device is ready (on latest firmware and initialized)
                    let is_device_ready = {
                        if let Ok(latest_firmware) = latest_firmware_result {
                            log::info!("🔍 [FRONTLOAD DEBUG] Latest firmware: {}", latest_firmware);
                            
                            let version_check_result = utils::is_version_older(&features.version, &latest_firmware);
                            log::info!("🔍 [FRONTLOAD DEBUG] Version check result: {:?}", version_check_result);
                            
                            if let Ok(is_outdated) = version_check_result {
                                log::info!("🔍 [FRONTLOAD DEBUG] Is outdated: {}", is_outdated);
                                let cache_ready = {
                                    match cache::DeviceCache::open() {
                                        Ok(c) => {
                                            match tokio::task::block_in_place(|| block_on(c.has_cached_addresses(&device_id))) {
                                                Ok(has) => has,
                                                Err(e) => { log::warn!("Cache readiness check failed: {}", e); false }
                                            }
                                        }
                                        Err(e) => { log::warn!("Failed to open cache: {}", e); false }
                                    }
                                };
                                let ready = !is_outdated && features.initialized && cache_ready;
                                log::info!("🔍 [FRONTLOAD DEBUG] Device ready: {}", ready);
                                ready
                            } else {
                                log::error!("🔍 [FRONTLOAD DEBUG] VERSION CHECK FAILED - THIS IS THE BUG");
                                false
                            }
                        } else {
                            log::error!("🔍 [FRONTLOAD DEBUG] FAILED TO GET LATEST FIRMWARE - THIS IS THE BUG");
                            false
                        }
                    };
                    
                    // If device is ready, set context first, then trigger frontload
                    if is_device_ready {
                        log::info!("🚀 [FRONTLOAD TRIGGER] Device {} is ready (firmware v{}, initialized), starting frontload...", 
                            device_id, features.version);
                        
                        // Clone what we need for context setting
                        let device_id_for_context = device_id.clone();
                        let device_label = features.label.clone();
                        
                        // Set device context immediately before frontload
                        // This ensures V1 API calls will work even if frontload fails
                        log::info!("Setting device context for {} before frontload", device_id_for_context);
                        
                        tauri::async_runtime::spawn(async move {
                            // Use the helper function that gets real Ethereum address from device
                            let status = crate::server::context::set_context_with_real_eth_address(
                                device_id_for_context.clone(),
                                device_label.clone()
                            ).await;
                            
                            if status == axum::http::StatusCode::NO_CONTENT {
                                log::info!("✅ Device context set successfully for {} before frontload", device_id_for_context);
                            } else {
                                log::error!("❌ Failed to set device context for {} before frontload", device_id_for_context);
                            }
                        });
                        
                        // Clone what we need for the async task
                        let device_id_clone = device_id.clone();
                        let emitter_clone = emitter.clone();
                        
                        // Emit "registering device" status immediately
                        if let Ok(entries) = device_registry::get_all_device_entries() {
                            let payload = ApplicationState {
                                status: "Registering device...".to_string(),
                                connected: !entries.is_empty(),
                                devices: entries.iter().map(|e| e.into()).collect(),
                                blocking_actions_count: 0,
                            };
                            let _ = emitter_clone.emit("application:state", &payload);
                        }
                        
                        // Start device frontload in background with crash protection
                        tokio::spawn(async move {
                            log::info!("Device {} is ready for frontload", device_id_clone);
                            
                            // Wrap frontload in proper async error handling instead of panic catch
                            let frontload_result = async {
                                // Find the device entry in the registry
                                let device_entry = match device_registry::get_all_device_entries() {
                                    Ok(entries) => entries.into_iter()
                                        .find(|e| e.device.unique_id == device_id_clone),
                                    Err(e) => {
                                        log::error!("Failed to get device entries from registry: {}", e);
                                        return Err(anyhow::anyhow!("Registry access failed: {}", e));
                                    }
                                };
                                
                                if let Some(device_entry) = device_entry {
                                    log::info!("Found device entry for frontload: {}", device_id_clone);

                                    // Get cache from device manager if available
                                    let cache = match cache::DeviceCache::open() {
                                        Ok(cache) => cache,
                                        Err(e) => {
                                            log::error!("Failed to open device cache for frontload: {}", e);
                                            return Err(anyhow::anyhow!("Cache open failed: {}", e));
                                        }
                                    };

                                    // Validate device cache before frontload
                                    let cache_valid = crate::server::context::validate_device_cache(&device_id_clone).await;
                                    if cache_valid {
                                        log::info!("✅ Device cache validated for {}", device_id_clone);
                                    } else {
                                        log::warn!("⚠️  Device cache validation failed for {}", device_id_clone);
                                    }

                                    // Create frontloader using the new factory pattern with device info
                                    // This ensures proper transport creation with USB/HID fallback
                                    let frontloader = cache::DeviceFrontloader::new_with_device(
                                        cache,
                                        device_entry.device.clone()
                                    );
                                                        
                                    // Track progress for UI updates
                                    let _last_progress = 0;
                                    let _total_steps = 50; // Approximate number of addresses to load
                                                        
                                    // Create progress callback that emits to frontend
                                    let emitter_for_progress = emitter_clone.clone();
                                    let progress_callback = move |msg: String| {
                                        log::info!("Frontload progress: {}", msg);
                                        if let Ok(entries) = device_registry::get_all_device_entries() {
                                            let payload = ApplicationState {
                                                status: msg,
                                                connected: !entries.is_empty(),
                                                devices: entries.iter().map(|e| e.into()).collect(),
                                                blocking_actions_count: 0,
                                            };
                                            let _ = emitter_for_progress.emit("application:state", &payload);
                                        }
                                    };
                                    
                                    // Start frontload with progress tracking
                                    frontloader.frontload_all_with_progress(Some(progress_callback)).await
                                } else {
                                    log::error!("Device {} not found in registry for frontload", device_id_clone);
                                    Err(anyhow::anyhow!("Device not found in registry"))
                                }
                            }.await;
                            
                            // Handle the result of frontload (successful or failed)
                            match frontload_result {
                                Ok(_) => {
                                    log::info!("✅ Frontload completed successfully for device {}", device_id_clone);
                                    
                                    // Auto-extract xpubs from cache to populate wallet_xpubs table
                                    log::info!("🔄 Auto-extracting xpubs from cache for device {}", device_id_clone);
                                    if let Err(e) = crate::commands::auto_extract_xpubs_on_ready(device_id_clone.clone(), emitter_clone.clone()).await {
                                        log::error!("❌ Failed to auto-extract xpubs for device {}: {}", device_id_clone, e);
                                    } else {
                                        log::info!("✅ Xpubs auto-extracted for device {}", device_id_clone);
                                    }
                                    
                                    // Wait a moment for all async operations to settle
                                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                                    
                                    log::info!("🚀 Device {} is fully ready", device_id_clone);
                                    
                                    if let Ok(entries) = device_registry::get_all_device_entries() {
                                        let payload = ApplicationState {
                                            status: "Device ready".to_string(),
                                            connected: !entries.is_empty(),
                                            devices: entries.iter().map(|e| e.into()).collect(),
                                            blocking_actions_count: 0,
                                        };
                                        let _ = emitter_clone.emit("application:state", &payload);
                                    }
                                }
                                Err(e) => {
                                    log::error!("❌ Frontload failed for device {}: {}", device_id_clone, e);
                                    
                                    // Check if this is a transport/device access error that requires troubleshooting
                                    let error_msg = e.to_string();
                                    let needs_troubleshooter = error_msg.contains("Physical device not found") ||
                                                             error_msg.contains("No transport available") ||
                                                             error_msg.contains("transport failures") ||
                                                             error_msg.contains("Failed with both USB") ||
                                                             error_msg.contains("Entity not found") ||
                                                             error_msg.contains("Transport creation failed");
                                    
                                    if needs_troubleshooter {
                                        // Show troubleshooting view instead of error
                                        if let Ok(entries) = device_registry::get_all_device_entries() {
                                            let payload = ApplicationState {
                                                status: "Device connection issues - troubleshooting".to_string(),
                                                connected: !entries.is_empty(),
                                                devices: entries.iter().map(|e| e.into()).collect(),
                                                blocking_actions_count: 0,
                                            };
                                            let _ = emitter_clone.emit("application:state", &payload);
                                        }
                                    } else {
                                        // Show generic error
                                        if let Ok(entries) = device_registry::get_all_device_entries() {
                                            let payload = ApplicationState {
                                                status: format!("Setup failed: {}", e).to_string(),
                                                connected: !entries.is_empty(),
                                                devices: entries.iter().map(|e| e.into()).collect(),
                                                blocking_actions_count: 0,
                                            };
                                            let _ = emitter_clone.emit("application:state", &payload);
                                        }
                                    }
                                }
                            }
                        });
                    }
                    // Check bootloader version against required version and create blocking action if needed
                    let mut actions_added = false;
                    if let Some(bootloader_version) = features.bootloader_version.as_deref() {
                        let required_version = "2.1.4"; // Required minimum bootloader version
                        if let Ok(is_outdated) = utils::is_version_older(bootloader_version, required_version) {
                            if is_outdated {
                                log::info!("Device {} has outdated bootloader v{}, required v{}", 
                                    device_id, bootloader_version, required_version);
                                
                                // Get the blocking actions registry from app state
                                if let Some(state) = emitter.try_state::<blocking_actions::BlockingActionsState>() {
                                    let registry = state.registry();
                                    let mut registry_lock = registry.lock().unwrap();
                                    
                                    // Create a bootloader update blocking action
                                    let action = blocking_actions::BlockingAction::new_bootloader_update(
                                        &device_id,
                                        bootloader_version,
                                        required_version
                                    );
                                    
                                    // Add to registry
                                    registry_lock.add_action(action);
                                    actions_added = true;
                                    
                                    // Drop the lock to allow firmware check to take it
                                    drop(registry_lock);
                                }
                            }
                        }
                    }
                    
                            // Also check firmware version for updates
        let firmware_version = &features.version; // Direct access since it's a String
        let latest_firmware = match device_update::get_latest_firmware_version() {
            Ok(version) => version,
            Err(e) => {
                log::error!("Failed to get latest firmware version from releases.json: {}. Skipping firmware update check.", e);
                continue; // Skip this device's firmware check
            }
        };
        if let Ok(is_outdated) = utils::is_version_older(firmware_version, &latest_firmware) {
                        if is_outdated {
                            log::info!("Device {} has outdated firmware v{}, latest v{}", 
                                device_id, firmware_version, latest_firmware);
                            
                            // Get the blocking actions registry from app state
                            if let Some(state) = emitter.try_state::<blocking_actions::BlockingActionsState>() {
                                let registry = state.registry();
                                let mut registry_lock = registry.lock().unwrap();
                                
                                // Create a firmware update blocking action
                                let action = blocking_actions::BlockingAction::new_firmware_update(
                                    &device_id,
                                                                firmware_version,
                            &latest_firmware
                                );
                                
                                // Add to registry
                                registry_lock.add_action(action);
                                actions_added = true;
                                
                                // Keep lock until we emit the event
                                let _count = registry_lock.total_action_count();
                                drop(registry_lock); // Release lock before emit
                            }
                        }
                    }
                    
                    // Emit blocking actions update event if any actions were added
                    if actions_added {
                        if let Some(state) = emitter.try_state::<blocking_actions::BlockingActionsState>() {
                            let registry = state.registry();
                            let registry_lock = registry.lock().unwrap();
                            let count = registry_lock.total_action_count();
                            drop(registry_lock); // Release lock before emit
                            
                            log::info!("Emitting blocking actions update: {} actions", count);
                            let _ = emitter.emit("blocking:actions_updated", count);
                        }
                    }
                    
                    // Emit the regular features updated event
                    let _ = emitter.emit("device:features-updated", serde_json::json!({
                        "deviceId": device_id,
                        "features": features,
                        "status": status
                    }));
                }
                device_controller::DeviceControllerEvent::FeatureFetchFailed { device_id, error } => {
                    log::warn!("Failed to fetch features for {}: {}", device_id, error);
                }
                _ => {}
            }
            
            // Always emit the current state after any change
            if let Ok(entries) = device_registry::get_all_device_entries() {
                let status = if entries.is_empty() {
                    "No devices connected".to_string()
                } else {
                    // Check if any device is ready (on latest firmware and no blocking actions)
                    let ready_devices = entries.iter().filter(|entry| {
                        if let Some(features) = &entry.features {
                            // Check if firmware is up to date AND cache ready
                            if let Ok(latest_firmware) = device_update::get_latest_firmware_version() {
                                if let Ok(is_outdated) = utils::is_version_older(&features.version, &latest_firmware) {
                                    if !is_outdated && features.initialized {
                                        // Check cached addresses synchronously
                                        if let Ok(cache) = cache::DeviceCache::open() {
                                            if let Ok(all_cached) = tokio::task::block_in_place(|| block_on(cache.has_cached_addresses(&entry.device.unique_id))) {
                                                return all_cached;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        false
                    }).count();
                    
                    if ready_devices > 0 {
                        "Device ready".to_string()
                    } else {
                        format!("{} device(s) connected", entries.len())
                    }
                };
                
                let payload = ApplicationState {
                    status,
                    connected: !entries.is_empty(),
                    devices: entries.iter().map(|e| e.into()).collect(),
                    blocking_actions_count: 0, // Will need to get actual count in the future
                };
                let _ = emitter.emit("application:state", &payload);
            }
        }
    });
    
    // 5️⃣  Initial state poll (to handle devices already connected)
    let poll_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        // Wait a bit for the system to initialize
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        if let Ok(entries) = device_registry::get_all_device_entries() {
            let status = if entries.is_empty() {
                "No devices connected".to_string()
            } else {
                // Check if any device is ready (on latest firmware and no blocking actions)
                let ready_devices = entries.iter().filter(|entry| {
                    if let Some(features) = &entry.features {
                        // Check if firmware is up to date AND cache ready
                        if let Ok(latest_firmware) = device_update::get_latest_firmware_version() {
                            if let Ok(is_outdated) = utils::is_version_older(&features.version, &latest_firmware) {
                                if !is_outdated && features.initialized {
                                    // Check cached addresses synchronously
                                    if let Ok(cache) = cache::DeviceCache::open() {
                                        if let Ok(all_cached) = tokio::task::block_in_place(|| block_on(cache.has_cached_addresses(&entry.device.unique_id))) {
                                            return all_cached;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    false
                }).count();
                
                if ready_devices > 0 {
                    "Device ready".to_string()
                } else {
                    format!("{} device(s) connected", entries.len())
                }
            };
            
            let payload = ApplicationState {
                status,
                connected: !entries.is_empty(),
                devices: entries.iter().map(|e| e.into()).collect(),
                blocking_actions_count: 0,
            };
            let _ = poll_handle.emit("application:state", &payload);
        }
    });

    dm_arc
}

// =============================================================
//  Tauri Commands (example)
// =============================================================
#[tauri::command]
fn greet(name: &str) -> String {
    match device_registry::get_all_device_entries() {
        Ok(entries) if !entries.is_empty() => {
            let device_info = entries.iter()
                .filter_map(|entry| entry.features.as_ref())
                .map(|f| format!(
                    "{} {} v{}",
                    f.vendor.as_deref().unwrap_or("Unknown"),
                    f.model.as_deref().unwrap_or("Device"),
                    f.version
                ))
                .collect::<Vec<_>>()
                .join(", ");
            format!("Hello, {name}! Connected devices: {device_info}")
        }
        _ => format!("Hello, {name}! No devices connected."),
    }
}

#[tauri::command]
async fn restart_backend_startup(app_handle: AppHandle) -> Result<(), String> {
    log::info!("Restart backend startup requested via logo click");
    
    // Reset status to starting
    let starting_payload = ApplicationState {
        status: "Restarting...".to_string(),
        connected: false,
        devices: vec![],
        blocking_actions_count: 0,
    };
    app_handle.emit("application:state", &starting_payload)
        .map_err(|e| format!("Failed to emit restart state: {}", e))?;
    
    // Give a moment for the UI to update
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Force a device scan and state update
    let scan_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        // Wait a bit for the restart message to be processed
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        // Update status to scanning
        let scanning_payload = ApplicationState {
            status: "Scanning for devices...".to_string(),
            connected: false,
            devices: vec![],
            blocking_actions_count: 0,
        };
        let _ = scan_handle.emit("application:state", &scanning_payload);
        
        // Give some time for device detection
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Poll current device state and emit
        if let Ok(entries) = device_registry::get_all_device_entries() {
            let status = if entries.is_empty() {
                "No devices connected".to_string()
            } else {
                // Check if any device is ready (on latest firmware and initialized)
                let ready_devices = entries.iter().filter(|entry| {
                    if let Some(features) = &entry.features {
                        // Check if firmware is up to date
                        if let Ok(latest_firmware) = device_update::get_latest_firmware_version() {
                            if let Ok(is_outdated) = utils::is_version_older(&features.version, &latest_firmware) {
                                if !is_outdated && features.initialized {
                                    // Device is on latest firmware and initialized
                                    return true;
                                }
                            }
                        }
                    }
                    false
                }).count();
                
                if ready_devices > 0 {
                    "Device ready".to_string()
                } else {
                    format!("{} device(s) connected", entries.len())
                }
            };
            
            let payload = ApplicationState {
                status,
                connected: !entries.is_empty(),
                devices: entries.iter().map(|e| e.into()).collect(),
                blocking_actions_count: 0,
            };
            let _ = scan_handle.emit("application:state", &payload);
        }
    });
    
    Ok(())
}

// Vault UI control commands
#[tauri::command]
async fn vault_change_view(app_handle: AppHandle, view: String) -> Result<(), String> {
    log::info!("Vault view change requested: {}", view);
    
    // Emit event to frontend to change view
    app_handle.emit("vault:change_view", serde_json::json!({
        "view": view
    })).map_err(|e| format!("Failed to emit view change event: {}", e))?;
    
    Ok(())
}

#[tauri::command]
async fn vault_open_app(app_handle: AppHandle, app_id: String, app_name: String, url: String) -> Result<(), String> {
    log::info!("Vault app open requested: {} ({}) -> {}", app_name, app_id, url);
    
    // First switch to browser view
    app_handle.emit("vault:change_view", serde_json::json!({
        "view": "browser"
    })).map_err(|e| format!("Failed to emit view change event: {}", e))?;
    
    // Then navigate to the app URL
    app_handle.emit("browser:navigate", serde_json::json!({
        "url": url
    })).map_err(|e| format!("Failed to emit navigation event: {}", e))?;
    
    Ok(())
}

#[tauri::command]
async fn vault_open_support(app_handle: AppHandle) -> Result<(), String> {
    log::info!("Vault support requested");
    
    // Switch to browser view and navigate to support
    app_handle.emit("vault:change_view", serde_json::json!({
        "view": "browser"
    })).map_err(|e| format!("Failed to emit view change event: {}", e))?;
    
    app_handle.emit("browser:navigate", serde_json::json!({
        "url": "https://support.keepkey.com"
    })).map_err(|e| format!("Failed to emit navigation event: {}", e))?;
    
    Ok(())
}

#[tauri::command]
async fn browser_navigate(app_handle: AppHandle, url: String) -> Result<(), String> {
    log::info!("Browser navigation requested: {}", url);
    
    // Store current URL in backend state if needed
    // For now, just emit the navigation event back to frontend
    app_handle.emit("browser:navigate", serde_json::json!({
        "url": url
    })).map_err(|e| format!("Failed to emit navigation event: {}", e))?;
    
    Ok(())
}

// =============================================================
//  Application bootstrap
// =============================================================
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    println!("{TAG} Starting application…");

    // Logging plugin
    let log_plugin = tauri_plugin_log::Builder::default()
        .level(log::LevelFilter::Info)
        .build();

    tauri::Builder::default()
        .plugin(log_plugin)
        .setup(|app| {
            log::info!("{TAG} set‑up phase");

            // Capture an owned AppHandle immediately
            let app_handle = app.handle();

            // Set dynamic window title with version
            if let Some(window) = app.get_webview_window("main") {
                let version = app.config().version.as_ref().map(|v| v.to_string()).unwrap_or_else(|| "0.1.1".to_string());
                let title = format!("KeepKey Vault v{} (Bitcoin Only)", version);
                if let Err(e) = window.set_title(&title) {
                    log::warn!("Failed to set window title: {}", e);
                }
            }

            // Create blocking actions state first
            let blocking_actions = blocking_actions::BlockingActionsState::new();
            app.manage(blocking_actions.clone());

            // 1️⃣  USB service & cache (with blocking actions)
            let dm = start_usb_service(&app_handle, blocking_actions);
            app.manage(dm.clone());

            // Manage vault state
            app.manage(vault::VaultState::default());

            // 2️⃣  Initial placeholder
            let initial = ApplicationState {
                status: "Starting application".into(),
                connected: false,
                devices: vec![],
                blocking_actions_count: 0, // Add count of blocking actions
            };
            app_handle.emit("application:state", &initial).ok();

            // 3️⃣  Start REST / MCP server in background (only if enabled in preferences)
            let server_handle = app_handle.clone();
            let server_dm = dm.clone(); // Use the same device manager instance instead of creating a new one
            tauri::async_runtime::spawn(async move {
                // Check if API is enabled in preferences
                let api_enabled = match index_db::IndexDb::open() {
                    Ok(db) => {
                        match db.get_preference("api_enabled") {
                            Ok(Some(value)) => value == "true",
                            _ => false, // Default to disabled if not set or error
                        }
                    }
                    Err(_) => false, // Default to disabled if database error
                };
                
                if api_enabled {
                    log::info!("{TAG} API is enabled in preferences, starting server...");
                    
                    if let Err(e) = server::start_server(server_dm).await {
                        log::error!("{TAG} Server error: {e}");
                        let err_payload = ApplicationState {
                            status: format!("Server error: {e}"),
                            connected: false,
                            devices: vec![],
                            blocking_actions_count: 0, // Add count of blocking actions
                        };
                        server_handle.emit("application:state", &err_payload).ok();
                    }
                } else {
                    log::info!("{TAG} API is disabled in preferences, skipping server startup");
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            restart_backend_startup,
            // Vault UI control commands
            vault_change_view,
            vault_open_app,
            vault_open_support,
            browser_navigate,
            commands::get_device_info,
            commands::get_device_info_by_id,
            commands::get_all_devices,
            commands::get_connected_devices,
            commands::get_disconnected_devices,
            commands::get_device_status,
            commands::check_vault_exists,
            commands::create_vault,
            commands::unlock_vault,
            commands::is_first_time_install,
            commands::is_onboarded,
            commands::set_onboarding_completed,
            commands::debug_onboarding_state,
            commands::get_preference,
            commands::set_preference,
            commands::get_api_enabled,
            commands::set_api_enabled,
            commands::get_api_status,
            commands::restart_api_server,
            usb_manager::list_usb_devices,
            commands::get_blocking_actions,
            commands::resolve_blocking_action,
            updates::update_device_bootloader,
            updates::update_device_firmware,
            updates::update_resolve_blocking_action,
            // Wallet creation commands
            commands::set_device_label,
            commands::initialize_device_pin,
            commands::send_button_ack,
            commands::send_pin_matrix_response,
            commands::get_pin_session_status,
            commands::cancel_pin_creation,
            commands::complete_pin_creation,
            commands::confirm_device_pin,
            commands::initialize_device_wallet,
            commands::get_device_recovery_phrase,
            commands::complete_wallet_creation,
            commands::wipe_device,
            // Dialog queue management commands
            commands::queue_dialog,
            commands::get_next_dialog,
            commands::complete_dialog,
            commands::get_dialog_queue_status,
            // Recovery commands
            commands::start_device_recovery,
            commands::send_recovery_character,
            commands::send_recovery_pin_response,
            commands::get_recovery_status,
            commands::cancel_recovery_session,
            // Seed verification commands (dry run recovery)
            commands::start_seed_verification,
            commands::send_verification_pin,
            commands::send_verification_character,
            commands::get_verification_status,
            commands::cancel_seed_verification,
            commands::force_cleanup_seed_verification,
            // Wallet Context Commands (vault-v2 pattern)
            commands::get_required_paths,
            commands::get_wallet_xpubs,
            commands::sync_device_xpubs,
            commands::get_portfolio_cache,
            commands::refresh_portfolio,
            commands::clear_portfolio_cache,
            commands::get_fee_rates,
            commands::get_wallet_summary,
            commands::extract_xpubs_from_cache,
            commands::auto_extract_xpubs_on_ready,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// =============================================================
//  EOF – next step: introduce Backend struct + migrate caches
// =============================================================
