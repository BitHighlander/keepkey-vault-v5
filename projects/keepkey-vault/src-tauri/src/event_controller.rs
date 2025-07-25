use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tauri::{AppHandle, Emitter, Manager};
use keepkey_rust::friendly_usb::FriendlyUsbDevice;

// Device operation timeout - matches the timeout used in commands.rs
const DEVICE_OPERATION_TIMEOUT_SECS: u64 = 30;
const FRONTLOAD_DEVICE_TIMEOUT_MS: u64 = 500; // Optimized timeout for frontload operations
// Grace period before treating device as truly disconnected
const DEVICE_DISCONNECTION_GRACE_PERIOD_SECS: u64 = 10;

#[derive(Clone)]
struct DeviceConnectionInfo {
    device: FriendlyUsbDevice,
    disconnected_at: Option<Instant>,
}

pub struct EventController {
    cancellation_token: CancellationToken,
    task_handle: Option<tauri::async_runtime::JoinHandle<()>>,
    is_running: bool,
    // Track devices with connection state (use async mutex for await compatibility)
    known_devices: Arc<tokio::sync::Mutex<HashMap<String, DeviceConnectionInfo>>>,
    // Track events that have been emitted to prevent infinite loops
    emitted_events: Arc<tokio::sync::Mutex<HashMap<String, i64>>>,
}

impl EventController {
    pub fn new() -> Self {
        Self {
            cancellation_token: CancellationToken::new(),
            task_handle: None,
            is_running: false,
            known_devices: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            emitted_events: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }
    
    pub fn start(&mut self, app: &AppHandle) {
        if self.is_running {
            println!("⚠️ Event controller already running");
            return;
        }
        
        let app_handle = app.clone();
        let cancellation_token = self.cancellation_token.clone();
        
        let known_devices = self.known_devices.clone();
        let emitted_events = self.emitted_events.clone();
        
        let task_handle = tauri::async_runtime::spawn(async move {
            let mut interval = interval(Duration::from_millis(1000)); // Check every second
            let mut last_devices: Vec<FriendlyUsbDevice> = Vec::new();
            
            println!("✅ Event controller started - monitoring device connections");
            
            // Wait a moment for frontend to set up listeners, then emit initial scanning status
            tokio::time::sleep(Duration::from_millis(500)).await;
            println!("📡 Emitting status: Scanning for devices...");
            let scanning_payload = serde_json::json!({
                "status": "Scanning for devices..."
            });
            println!("📡 Scanning payload: {}", scanning_payload);
            if let Err(e) = app_handle.emit("status:update", scanning_payload) {
                println!("❌ Failed to emit scanning status: {}", e);
            } else {
                println!("✅ Successfully emitted scanning status");
            }

            // Test emission after longer delay to check if frontend is listening
//             let app_for_test = app_handle.clone();
//             tokio::spawn(async move {
//                 tokio::time::sleep(Duration::from_millis(3000)).await;
//                 println!("📡 Test: Emitting delayed test status...");
//                 let test_payload = serde_json::json!({
//                     "status": "Test message after 3 seconds"
//                 });
//                 println!("📡 Test payload: {}", test_payload);
//                 if let Err(e) = app_for_test.emit("status:update", test_payload) {
//                     println!("❌ Failed to emit delayed test status: {}", e);
//                 } else {
//                     println!("✅ Successfully emitted delayed test status");
//                 }
//             });
            
            println!("🔍 DEBUG: Starting event controller main loop");
            let mut loop_count = 0;
            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        println!("🛑 Event controller shutting down on cancellation signal");
                        break;
                    }
                    _ = interval.tick() => {
                        loop_count += 1;
                        if loop_count % 10 == 0 {
                            println!("🔍 DEBUG: Event loop still running - iteration {}", loop_count);
                        }
                        
                        // Get current devices using high-level API
                        let current_devices = keepkey_rust::features::list_connected_devices();
                        log::trace!("Device scan #{} - found {} devices", loop_count, current_devices.len());
                        
                        // Handle device disconnections by checking which devices are no longer present
                        let current_device_ids: std::collections::HashSet<String> = current_devices.iter()
                            .map(|d| d.unique_id.clone())
                            .collect();
                        let last_device_ids: std::collections::HashSet<String> = last_devices.iter()
                            .map(|d| d.unique_id.clone())
                            .collect();
                        
                        // Find disconnected devices
                        let disconnected_devices: Vec<String> = last_device_ids.difference(&current_device_ids)
                            .cloned()
                            .collect();
                        
                        if !disconnected_devices.is_empty() {
                            println!("🔌 Detected {} device(s) disconnected: {:?}", disconnected_devices.len(), disconnected_devices);
                            
                            // Clear tracking state for disconnected devices to allow fresh reconnection
                            let mut known = known_devices.lock().await;
                            let mut emitted = emitted_events.lock().await;
                            for device_id in &disconnected_devices {
                                // Remove all tracking entries for this device
                                let keys_to_remove: Vec<String> = known.keys()
                                    .filter(|key| key.starts_with(device_id))
                                    .cloned()
                                    .collect();
                                
                                for key in keys_to_remove {
                                    known.remove(&key);
                                    println!("🧹 Cleared tracking state: {}", key);
                                }
                                
                                // Remove all emitted event entries for this device
                                let emitted_keys_to_remove: Vec<String> = emitted.keys()
                                    .filter(|key| key.starts_with(device_id))
                                    .cloned()
                                    .collect();
                                
                                for key in emitted_keys_to_remove {
                                    emitted.remove(&key);
                                    println!("🧹 Cleared emitted event: {}", key);
                                }
                                
                                // Emit device disconnected event
                                let disconnect_payload = serde_json::json!({
                                    "device_id": device_id,
                                    "status": "disconnected"
                                });
                                let _ = app_handle.emit("device:disconnected", disconnect_payload);
                                println!("📡 Emitted device:disconnected for {}", device_id);
                            }
                            drop(known); // Release lock
                            drop(emitted); // Release lock
                        }
                        
                        // Process device connections and updates
                        {
                            let mut known = known_devices.lock().await;
                            for device in &current_devices {
                                    let device_key = device.serial_number.as_ref()
                                        .unwrap_or(&device.unique_id)
                                        .clone();
                                    
                                    if let Some(info) = known.get_mut(&device_key) {
                                        // Device was known - check if it was temporarily disconnected
                                        if info.disconnected_at.is_some() {
                                            println!("🔄 Device {} reconnected (was temporarily disconnected)", device_key);
                                            info.disconnected_at = None;
                                            info.device = device.clone();
                                            
                                            // Emit reconnection event
                                            let _ = app_handle.emit("device:reconnected", serde_json::json!({
                                                "deviceId": device.unique_id,
                                                "wasTemporary": true
                                            }));
                                            continue; // Skip new device processing
                                        }
                                    } else {
                                        // New device - add to tracking
                                        known.insert(device_key.clone(), DeviceConnectionInfo {
                                            device: device.clone(),
                                            disconnected_at: None,
                                        });
                                    }
                                }
                        } // Release lock
                        
                        // Check for newly connected devices (original logic)
                        println!("🔍 DEBUG: Checking {} current devices against {} last devices", current_devices.len(), last_devices.len());
                        for device in &current_devices {
                            let is_new = !last_devices.iter().any(|d| d.unique_id == device.unique_id);
                            println!("🔍 DEBUG: Device {} - is_new: {}", device.unique_id, is_new);
                            
                            if is_new {
                                println!("🔌 Device connected: {} (VID: 0x{:04x}, PID: 0x{:04x})", 
                                         device.unique_id, device.vid, device.pid);
                                println!("   Device info: {} - {}", 
                                         device.manufacturer.as_deref().unwrap_or("Unknown"), 
                                         device.product.as_deref().unwrap_or("Unknown"));
                                
                                // Check if this might be a recovery device reconnecting with a different ID
                                if let Some(state) = app_handle.try_state::<crate::commands::DeviceQueueManager>() {
                                    let queue_manager_arc = state.inner().clone();
                                    let manager = queue_manager_arc.lock().await;
                                    
                                    // Check if any existing device might be the same physical device
                                    for (existing_id, _) in manager.iter() {
                                        if crate::commands::are_devices_potentially_same(&device.unique_id, existing_id) &&
                                           crate::commands::is_device_in_recovery_flow(existing_id) {
                                            println!("🔄 Device {} appears to be recovery device {} reconnecting", 
                                                    device.unique_id, existing_id);
                                            let _ = crate::commands::add_recovery_device_alias(&device.unique_id, existing_id);
                                            
                                            // Emit special reconnection event
                                            let _ = app_handle.emit("device:recovery-reconnected", serde_json::json!({
                                                "new_id": &device.unique_id,
                                                "original_id": existing_id,
                                                "status": "reconnected"
                                            }));
                                        }
                                    }
                                }
                                
                                // Emit device found status
                                let device_short = &device.unique_id[device.unique_id.len().saturating_sub(8)..];
                                println!("📡 Emitting status: Device found {}", device_short);
                                let device_found_payload = serde_json::json!({
                                    "status": format!("Device found {}", device_short)
                                });
                                println!("📡 Device found payload: {}", device_found_payload);
                                if let Err(e) = app_handle.emit("status:update", device_found_payload) {
                                    println!("❌ Failed to emit device found status: {}", e);
                                } else {
                                    println!("✅ Successfully emitted device found status");
                                }
                                
                                // Emit basic device connected event first
                                println!("🔍 DEBUG: About to emit device:connected event");
                                let _ = app_handle.emit("device:connected", device);
                                println!("🔍 DEBUG: device:connected event emitted");
                                
                                // 🚀 TRIGGER FRONTLOAD IMMEDIATELY - Don't wait for features!
                                let device_id_for_immediate_frontload = device.unique_id.clone();
                                let app_for_immediate_frontload = app_handle.clone();
                                
                                tauri::async_runtime::spawn(async move {
                                    println!("🚀 IMMEDIATE FRONTLOAD: Triggering frontload on device connect for: {}", device_id_for_immediate_frontload);
                                    
                                    // Get the cache manager from app state
                                    if let Some(cache_state) = app_for_immediate_frontload.try_state::<std::sync::Arc<once_cell::sync::OnceCell<std::sync::Arc<crate::cache::CacheManager>>>>() {
                                        match crate::commands::get_cache_manager(cache_state.inner()).await {
                                            Ok(cache_manager) => {
                                                println!("✅ Cache manager obtained for immediate frontload");
                                                
                                                // Get device queue manager from app state
                                                if let Some(queue_state) = app_for_immediate_frontload.try_state::<crate::commands::DeviceQueueManager>() {
                                                    println!("✅ Device queue manager obtained for immediate frontload");
                                                    let device_queue_manager = queue_state.inner().clone();
                                                    
                                                    // Create frontload controller
                                                    let frontload_controller = crate::cache::FrontloadController::new(
                                                        cache_manager.clone(),
                                                        device_queue_manager,
                                                    );
                                                    
                                                    println!("🚀 Starting IMMEDIATE frontload for device: {}", device_id_for_immediate_frontload);
                                                    
                                                    // Start frontload immediately
                                                    match frontload_controller.frontload_device(&device_id_for_immediate_frontload).await {
                                                        Ok(_) => {
                                                            println!("✅ IMMEDIATE frontload completed successfully for device: {}", device_id_for_immediate_frontload);
                                                            
                                                            // Emit frontload completion event
                                                            let _ = app_for_immediate_frontload.emit("cache:frontload-completed", serde_json::json!({
                                                                "device_id": device_id_for_immediate_frontload,
                                                                "success": true,
                                                                "immediate": true
                                                            }));
                                                        }
                                                        Err(e) => {
                                                            println!("⚠️ IMMEDIATE frontload failed for device {}: {}", device_id_for_immediate_frontload, e);
                                                        }
                                                    }
                                                } else {
                                                    println!("❌ Device queue manager not available for immediate frontload");
                                                }
                                            }
                                            Err(e) => {
                                                println!("❌ Failed to get cache manager for immediate frontload: {}", e);
                                            }
                                        }
                                    } else {
                                        println!("❌ Cache state not available for immediate frontload");
                                    }
                                });
                                
                                // Proactively fetch features and emit device:ready when successful
                                // DIRECT EXECUTION - NO SPAWNING
                                println!("📡 Fetching device features for: {}", device.unique_id);
                                
                                // Emit getting features status
                                println!("📡 Emitting status: Getting features...");
                                if let Err(e) = app_handle.emit("status:update", serde_json::json!({
                                    "status": "Getting features..."
                                })) {
                                    println!("❌ Failed to emit getting features status: {}", e);
                                }
                                
                                // Clone what we need for the async block
                                let device_for_features = device.clone();
                                let app_for_features = app_handle.clone();
                                
                                match try_get_device_features(&device_for_features, &app_for_features).await {
                                        Ok(features) => {
                                            let device_label = features.label.as_deref().unwrap_or("Unlabeled");
                                            let device_version = &features.version;
                                            
                                            println!("📡 Got device features: {} v{} ({})", 
                                                   device_label,
                                                   device_version,
                                                   device_for_features.unique_id);
                                            
                                            // Emit device info status
                                            println!("📡 Emitting status: {} v{}", device_label, device_version);
                                            if let Err(e) = app_for_features.emit("status:update", serde_json::json!({
                                                "status": format!("{} v{}", device_label, device_version)
                                            })) {
                                                println!("❌ Failed to emit device info status: {}", e);
                                            }
                                            
                                            // Evaluate device status to determine if updates are needed
                                            let status = crate::commands::evaluate_device_status(
                                                device_for_features.unique_id.clone(), 
                                                Some(&features)
                                            );
                                            
                                                                        // Check if device is locked with PIN before determining if it's ready
                            let has_pin_protection = features.pin_protection;
                            let pin_cached = features.pin_cached;
                            let is_pin_locked = features.initialized && has_pin_protection && !pin_cached;
                            
                            // Emit status updates based on what the device needs
                            // CRITICAL: Device in bootloader mode is NEVER ready
                            let is_actually_ready = !features.bootloader_mode &&  // Never ready if in bootloader mode
                                                   !status.needs_bootloader_update && 
                                                   !status.needs_firmware_update && 
                                                   !status.needs_initialization &&
                                                   !is_pin_locked;  // Device is NOT ready if locked with PIN
                            
                            if is_actually_ready {
                                                println!("✅ Device is fully ready, emitting device:ready event");
                                                println!("📡 Emitting status: Device ready");
                                                if let Err(e) = app_for_features.emit("status:update", serde_json::json!({
                                                    "status": "Device ready"
                                                })) {
                                                    println!("❌ Failed to emit device ready status: {}", e);
                                                }
                                                                                let ready_payload = serde_json::json!({
                                    "device": device_for_features,
                                    "features": features,
                                    "status": "ready"
                                });
                                
                                // Queue device:ready event as it's important for wallet initialization
                                if let Err(e) = crate::commands::emit_or_queue_event(&app_for_features, "device:ready", ready_payload).await {
                                    println!("❌ Failed to emit/queue device:ready event: {}", e);
                                } else {
                                    println!("📡 Successfully emitted/queued device:ready for {}", device_for_features.unique_id);
                                    
                                    // 🔍 Check onboarding status before triggering frontload
                                    let device_id_for_onboarding_check = device_for_features.unique_id.clone();
                                    let app_for_onboarding_check = app_for_features.clone();
                                    
                                    tauri::async_runtime::spawn(async move {
                                        // Check if user has completed onboarding first
                                        let is_onboarded = match crate::commands::is_onboarded().await {
                                            Ok(onboarded) => onboarded,
                                            Err(e) => {
                                                println!("⚠️ Failed to check onboarding status: {}", e);
                                                false // Default to not onboarded on error
                                            }
                                        };
                                        
                                        if !is_onboarded {
                                            println!("📚 Device {} is ready, but user has NOT completed onboarding - proceeding with frontload AND showing onboarding", device_id_for_onboarding_check);
                                            
                                            // Emit onboarding event but continue with frontload (don't return early)
                                            let onboarding_needed_payload = serde_json::json!({
                                                "device_id": device_id_for_onboarding_check,
                                                "status": "device_ready_onboarding_needed",
                                                "message": "Device is ready and frontload will proceed, but onboarding dialog will be shown"
                                            });
                                            
                                            if let Err(e) = crate::commands::emit_or_queue_event(&app_for_onboarding_check, "device:onboarding-required", onboarding_needed_payload).await {
                                                println!("❌ Failed to emit onboarding required event: {}", e);
                                            }
                                            // Continue with frontload below instead of returning
                                        }
                                        
                                        // User is onboarded, proceed with frontload
                                        println!("✅ User has completed onboarding - proceeding with automatic frontload for device {}", device_id_for_onboarding_check);
                                        println!("🚀 Device {} is ready, starting automatic frontload...", device_id_for_onboarding_check);
                                        
                                        let device_id_for_frontload = device_id_for_onboarding_check;
                                        let app_for_frontload = app_for_onboarding_check;
                                        
                                        println!("🔄 Starting automatic frontload for device: {}", device_id_for_frontload);
                                        
                                        // Get the cache manager from app state
                                        if let Some(cache_state) = app_for_frontload.try_state::<std::sync::Arc<once_cell::sync::OnceCell<std::sync::Arc<crate::cache::CacheManager>>>>() {
                                            match crate::commands::get_cache_manager(cache_state.inner()).await {
                                                Ok(cache_manager) => {
                                                    println!("✅ Cache manager obtained for frontload");
                                                    
                                                    // Get device queue manager from app state
                                                    if let Some(queue_state) = app_for_frontload.try_state::<crate::commands::DeviceQueueManager>() {
                                                        println!("✅ Device queue manager obtained for frontload");
                                                        let device_queue_manager = queue_state.inner().clone();
                                                        
                                                        // Create frontload controller with cloned cache manager
                                                        let cache_manager_for_controller = cache_manager.clone();
                                                        let frontload_controller = crate::cache::FrontloadController::new(
                                                            cache_manager_for_controller,
                                                            device_queue_manager,
                                                        );
                                                        
                                                        println!("🚀 Starting frontload for device: {}", device_id_for_frontload);
                                                        
                                                        // Start frontload
                                                        match frontload_controller.frontload_device(&device_id_for_frontload).await {
                                                            Ok(_) => {
                                                                println!("✅ Automatic frontload completed successfully for device: {}", device_id_for_frontload);
                                                                
                                                                // Emit frontload completion event
                                                                let _ = app_for_frontload.emit("cache:frontload-completed", serde_json::json!({
                                                                    "device_id": device_id_for_frontload,
                                                                    "success": true
                                                                }));
                                                                
                                                                // 📊 LOG PORTFOLIO SUMMARY FOR ALL DEVICES
                                                                tokio::spawn(async move {
                                                                    if let Err(e) = log_all_devices_portfolio_summary(&cache_manager).await {
                                                                        println!("⚠️ Failed to log portfolio summary: {}", e);
                                                                    }
                                                                });
                                                            }
                                                            Err(e) => {
                                                                println!("⚠️ Automatic frontload failed for device {}: {}", device_id_for_frontload, e);
                                                                
                                                                // Emit frontload error event (but don't block device ready state)
                                                                let _ = app_for_frontload.emit("cache:frontload-failed", serde_json::json!({
                                                                    "device_id": device_id_for_frontload,
                                                                    "error": e.to_string()
                                                                }));
                                                            }
                                                        }
                                                    } else {
                                                        println!("❌ Failed to get device queue manager for automatic frontload");
                                                    }
                                                }
                                                Err(e) => {
                                                    println!("❌ Failed to get cache manager for automatic frontload: {}", e);
                                                    println!("🔄 This usually means cache is still initializing - frontload will retry later");
                                                    
                                                    // Retry frontload after cache initialization delay
                                                    tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
                                                    println!("🔄 Retrying frontload after cache initialization delay...");
                                                    
                                                    // Try again with more detailed error logging
                                                    match crate::commands::get_cache_manager(cache_state.inner()).await {
                                                        Ok(cache_manager) => {
                                                            println!("✅ Cache manager ready on retry, proceeding with frontload");
                                                            
                                                            if let Some(queue_state) = app_for_frontload.try_state::<crate::commands::DeviceQueueManager>() {
                                                                let device_queue_manager = queue_state.inner().clone();
                                                                let frontload_controller = crate::cache::FrontloadController::new(
                                                                    cache_manager.clone(),
                                                                    device_queue_manager,
                                                                );
                                                                
                                                                match frontload_controller.frontload_device(&device_id_for_frontload).await {
                                                                    Ok(_) => {
                                                                        println!("✅ Frontload completed on retry for device: {}", device_id_for_frontload);
                                                                        let _ = app_for_frontload.emit("cache:frontload-completed", serde_json::json!({
                                                                            "device_id": device_id_for_frontload,
                                                                            "success": true
                                                                        }));
                                                                    }
                                                                    Err(retry_err) => {
                                                                        println!("❌ Frontload failed on retry: {}", retry_err);
                                                                        let _ = app_for_frontload.emit("cache:frontload-failed", serde_json::json!({
                                                                            "device_id": device_id_for_frontload,
                                                                            "error": retry_err.to_string()
                                                                        }));
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Err(retry_err) => {
                                                            println!("❌ Cache manager still not ready on retry: {}", retry_err);
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            println!("❌ Failed to get cache state for automatic frontload");
                                        }
                                    });
                                }
                                            } else {
                                                                                println!("⚠️ Device connected but needs updates (bootloader_mode: {}, bootloader: {}, firmware: {}, init: {}, pin_locked: {})", 
                                        features.bootloader_mode,
                                        status.needs_bootloader_update, 
                                        status.needs_firmware_update, 
                                        status.needs_initialization,
                                        is_pin_locked);
                                                
                                                if is_pin_locked {
                                                    println!("🔒 Device is initialized but locked with PIN - emitting unlock event");
                                                    
                                                    // Emit PIN unlock needed event
                                                                                                    let pin_unlock_payload = serde_json::json!({
                                                "deviceId": device_for_features.unique_id,
                                                        "features": features,
                                                        "status": status,
                                                        "needsPinUnlock": true
                                                    });
                                                    
                                                    if let Err(e) = crate::commands::emit_or_queue_event(&app_for_features, "device:pin-unlock-needed", pin_unlock_payload).await {
                                                        println!("❌ Failed to emit/queue device:pin-unlock-needed event: {}", e);
                                                    } else {
                                                        println!("📡 Successfully emitted/queued device:pin-unlock-needed for {}", device_for_features.unique_id);
                                                    }
                                                }
                                                
                                                // Emit appropriate status message based on what updates are needed
                                                let status_message = if features.bootloader_mode {
                                                    if status.needs_bootloader_update {
                                                        "Device in bootloader mode - update needed"
                                                    } else {
                                                        "Device in bootloader mode - reboot needed"
                                                    }
                                                } else if is_pin_locked {
                                                    "Device locked - enter PIN"
                                                } else if status.needs_bootloader_update && status.needs_firmware_update && status.needs_initialization {
                                                    "Device needs updates"
                                                } else if status.needs_bootloader_update {
                                                    "Bootloader update needed"
                                                } else if status.needs_firmware_update {
                                                    "Firmware update needed"
                                                } else if status.needs_initialization {
                                                    "Device setup needed"
                                                } else {
                                                    "Device ready"
                                                };
                                                
                                                println!("📡 Emitting status: {}", status_message);
                                                if let Err(e) = app_for_features.emit("status:update", serde_json::json!({
                                                    "status": status_message
                                                })) {
                                                    println!("❌ Failed to emit update status: {}", e);
                                                }
                                            }
                                            
                                                                        // Emit device:features-updated event with evaluated status (for DeviceUpdateManager)
                        // This is a critical event that should be queued if frontend isn't ready
                        // 🚨 ONLY EMIT ONCE PER DEVICE - Track to prevent infinite loops
                        let device_key = format!("{}:features-updated", device_for_features.unique_id);
                        let mut emitted = emitted_events.lock().await;
                        
                        if !emitted.contains_key(&device_key) {
                            // This is the first time emitting features-updated for this device
                            let features_payload = serde_json::json!({
                                "deviceId": device_for_features.unique_id,
                                "features": features,
                                "status": status  // Use evaluated status instead of hardcoded "ready"
                            });
                            
                            if let Err(e) = crate::commands::emit_or_queue_event(&app_for_features, "device:features-updated", features_payload).await {
                                println!("❌ Failed to emit/queue device:features-updated event: {}", e);
                            } else {
                                println!("📡 Successfully emitted/queued device:features-updated for {}", device_for_features.unique_id);
                                // Mark this device as having had features-updated emitted
                                emitted.insert(device_key, chrono::Utc::now().timestamp());
                            }
                        } else {
                            println!("🔄 Skipping duplicate device:features-updated for {}", device_for_features.unique_id);
                        }
                                        }
                                        Err(e) => {
                                            println!("❌ Failed to get features for {}: {}", device_for_features.unique_id, e);
                                            
                                            // Check for timeout errors specifically
                                            if e.contains("Timeout while fetching device features") || e.contains("Device operation timed out") {
                                                println!("⏱️ Device timeout detected - triggering frontload with cached data");
                                                
                                                // 🚀 TRIGGER FRONTLOAD ANYWAY - with cached data only
                                                let device_id_for_frontload = device_for_features.unique_id.clone();
                                                let app_for_frontload = app_for_features.clone();
                                                
                                                tauri::async_runtime::spawn(async move {
                                                    println!("🔄 Starting cache-only frontload for unresponsive device: {}", device_id_for_frontload);
                                                    
                                                    // Get the cache manager from app state
                                                    if let Some(cache_state) = app_for_frontload.try_state::<std::sync::Arc<once_cell::sync::OnceCell<std::sync::Arc<crate::cache::CacheManager>>>>() {
                                                        match crate::commands::get_cache_manager(cache_state.inner()).await {
                                                            Ok(cache_manager) => {
                                                                println!("✅ Cache manager obtained for cache-only frontload");
                                                                
                                                                // Get device queue manager from app state
                                                                if let Some(queue_state) = app_for_frontload.try_state::<crate::commands::DeviceQueueManager>() {
                                                                    println!("✅ Device queue manager obtained for cache-only frontload");
                                                                    let device_queue_manager = queue_state.inner().clone();
                                                                    
                                                                    // Create frontload controller
                                                                    let frontload_controller = crate::cache::FrontloadController::new(
                                                                        cache_manager.clone(),
                                                                        device_queue_manager,
                                                                    );
                                                                    
                                                                    println!("🚀 Starting cache-only frontload for unresponsive device: {}", device_id_for_frontload);
                                                                    
                                                                    // Start frontload (will work with cached data)
                                                                    match frontload_controller.frontload_device(&device_id_for_frontload).await {
                                                                        Ok(_) => {
                                                                            println!("✅ Cache-only frontload completed for unresponsive device: {}", device_id_for_frontload);
                                                                            
                                                                            // Emit frontload completion event
                                                                            let _ = app_for_frontload.emit("cache:frontload-completed", serde_json::json!({
                                                                                "device_id": device_id_for_frontload,
                                                                                "success": true,
                                                                                "cache_only": true
                                                                            }));
                                                                        }
                                                                        Err(e) => {
                                                                            println!("⚠️ Cache-only frontload failed for device {}: {}", device_id_for_frontload, e);
                                                                        }
                                                                    }
                                                                } else {
                                                                    println!("❌ Device queue manager not available for cache-only frontload");
                                                                }
                                                            }
                                                            Err(e) => {
                                                                println!("❌ Failed to get cache manager for cache-only frontload: {}", e);
                                                            }
                                                        }
                                                    } else {
                                                        println!("❌ Cache state not available for cache-only frontload");
                                                    }
                                                });
                                                
                                                // Also emit status update
                                                let _ = app_for_features.emit("status:update", serde_json::json!({
                                                    "status": "Device unresponsive - using cached data"
                                                }));
                                            }
                                            // Check if this is a device access error
                                            else if e.contains("Device Already In Use") || 
                                               e.contains("already claimed") ||
                                               e.contains("🔒") {
                                                
                                                let user_friendly_error = if e.contains("🔒") {
                                                    e.clone()
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
                                                
                                                // Emit device access error event
                                                let error_payload = serde_json::json!({
                                                    "deviceId": device_for_features.unique_id,
                                                    "error": user_friendly_error,
                                                    "errorType": "DEVICE_CLAIMED",
                                                    "status": "error"
                                                });
                                                let _ = app_for_features.emit("device:access-error", &error_payload);
                                            }
                                        }
                                    }
                            }
                        }
                        
                        // Check for disconnected devices with grace period
                        let mut known = known_devices.lock().await;
                        for device in &last_devices {
                            if !current_devices.iter().any(|d| d.unique_id == device.unique_id) {
                                // Check if this device has grace period tracking
                                let device_key = device.serial_number.as_ref()
                                    .unwrap_or(&device.unique_id)
                                    .clone();
                                
                                if let Some(info) = known.get_mut(&device_key) {
                                    if info.disconnected_at.is_none() {
                                        // First time noticing disconnection - start grace period
                                        println!("⏱️ Device {} temporarily disconnected - starting grace period", device.unique_id);
                                        info.disconnected_at = Some(Instant::now());
                                        continue; // Don't process as disconnected yet
                                    } else if let Some(disconnected_at) = info.disconnected_at {
                                        // Check if grace period has expired
                                        if disconnected_at.elapsed() < Duration::from_secs(DEVICE_DISCONNECTION_GRACE_PERIOD_SECS) {
                                            // Still in grace period
                                            continue;
                                        }
                                        // Grace period expired - process as disconnected
                                        println!("🔌❌ Device disconnected: {} (grace period expired)", device.unique_id);
                                    }
                                } else {
                                    // Unknown device - add to tracking with disconnection time
                                    known.insert(device_key, DeviceConnectionInfo {
                                        device: device.clone(),
                                        disconnected_at: Some(Instant::now()),
                                    });
                                    continue; // Start grace period for new device
                                }
                                
                                println!("🔌❌ Device disconnected: {}", device.unique_id);
                                
                                // Check if device is in recovery flow before cleaning up
                                let is_in_recovery = crate::commands::is_device_in_recovery_flow(&device.unique_id);
                                
                                if is_in_recovery {
                                    println!("🛡️ Device {} is in recovery flow - preserving queue and state", device.unique_id);
                                    // Don't emit disconnection or clean up queue - just wait for reconnection
                                    continue;
                                }
                                
                                // Emit device disconnected status
                                println!("📡 Emitting status: Device disconnected");
                                if let Err(e) = app_handle.emit("status:update", serde_json::json!({
                                    "status": "Device disconnected"
                                })) {
                                    println!("❌ Failed to emit disconnect status: {}", e);
                                }
                                
                                // Clean up device queue for disconnected device
                                if let Some(state) = app_handle.try_state::<crate::commands::DeviceQueueManager>() {
                                    let device_id = device.unique_id.clone();
                                    // Clone the underlying Arc so it outlives this scope
                                    let queue_manager_arc = state.inner().clone();
                                    tauri::async_runtime::spawn(async move {
                                        println!("♻️ Cleaning up device queue for disconnected device: {}", device_id);
                                        let mut manager = queue_manager_arc.lock().await;
                                        if let Some(handle) = manager.remove(&device_id) {
                                            let _ = handle.shutdown().await;
                                            println!("✅ Device queue cleaned up for: {}", device_id);
                                        }
                                    });
                                }
                                
                                let _ = app_handle.emit("device:disconnected", &device.unique_id);
                            }
                        }
                        
                        // If no devices connected after checking disconnections, emit scanning status
                        if current_devices.is_empty() && !last_devices.is_empty() {
                            // After a short delay, go back to scanning
                            let app_for_scanning = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(Duration::from_millis(1000)).await;
                                println!("📡 Emitting status: Scanning for devices... (after disconnect)");
                                if let Err(e) = app_for_scanning.emit("status:update", serde_json::json!({
                                    "status": "Scanning for devices..."
                                })) {
                                    println!("❌ Failed to emit scanning status after disconnect: {}", e);
                                }
                            });
                        }
                        
                        // Clean up devices that have been disconnected for too long
                        {
                            let mut known = known_devices.lock().await;
                            known.retain(|device_key, info| {
                                if let Some(disconnected_at) = info.disconnected_at {
                                    if disconnected_at.elapsed() > Duration::from_secs(DEVICE_DISCONNECTION_GRACE_PERIOD_SECS * 2) {
                                        println!("🧹 Removing device {} from tracking (disconnected too long)", device_key);
                                        return false;
                                    }
                                }
                                true
                            });
                        }
                        
                        last_devices = current_devices;
                    }
                }
            }
            
            println!("✅ Event controller stopped cleanly");
        });
        
        self.task_handle = Some(task_handle);
        self.is_running = true;
    }
    
    pub fn stop(&mut self) {
        if !self.is_running {
            return;
        }
        
        println!("🛑 Stopping event controller...");
        
        // Cancel the background task
        self.cancellation_token.cancel();
        self.is_running = false;
        
        // Wait for the task to complete if it exists
        if let Some(handle) = self.task_handle.take() {
            // Try to wait for completion with a timeout
            tauri::async_runtime::spawn(async move {
                                        if let Err(e) = tokio::time::timeout(Duration::from_secs(DEVICE_OPERATION_TIMEOUT_SECS), handle).await {
                    println!("⚠️ Event controller task did not stop within timeout: {}", e);
                } else {
                    println!("✅ Event controller task stopped successfully");
                }
            });
        }
    }
}

impl Drop for EventController {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Try to get device features without blocking the event loop
/// Returns features if successful, error message if failed
/// This function handles OOB bootloader detection by trying Initialize message when GetFeatures fails
async fn try_get_device_features(device: &FriendlyUsbDevice, app_handle: &AppHandle) -> Result<keepkey_rust::features::DeviceFeatures, String> {
    // Check if device is in PIN flow - if so, skip automatic feature fetching to avoid interference
    if crate::commands::is_device_in_pin_flow(&device.unique_id) {
        return Err("Device is in PIN flow - skipping automatic feature fetch".to_string());
    }
    
    // Use the shared device queue manager to prevent race conditions
    if let Some(queue_manager_state) = app_handle.try_state::<crate::commands::DeviceQueueManager>() {
        let queue_manager = queue_manager_state.inner().clone();
        
        // Get or create a single device queue handle for this device
        let queue_handle = {
            let mut manager = queue_manager.lock().await;
            
            if let Some(handle) = manager.get(&device.unique_id) {
                // Use existing handle to prevent multiple workers
                handle.clone()
            } else {
                // Create a new worker only if one doesn't exist
                // Use the centralized get_or_create_device_queue function if available
                // For now, create a new worker (this code path should be avoided in production)
                let handle = keepkey_rust::device_queue::DeviceQueueFactory::spawn_worker(
                    device.unique_id.clone(),
                    device.clone()
                );
                manager.insert(device.unique_id.clone(), handle.clone());
                handle
            }
        };
        
        // Double-check PIN flow status before making the call (race condition protection)
        if crate::commands::is_device_in_pin_flow(&device.unique_id) {
            return Err("Device entered PIN flow - aborting feature fetch".to_string());
        }
        
        // Try to get features with a timeout using the shared worker
        match tokio::time::timeout(Duration::from_secs(DEVICE_OPERATION_TIMEOUT_SECS), queue_handle.get_features()).await {
            Ok(Ok(raw_features)) => {
                // Convert features to our DeviceFeatures format
                let device_features = crate::commands::convert_features_to_device_features(raw_features);
                Ok(device_features)
            }
            Ok(Err(e)) => {
                let error_str = e.to_string();
                
                // Check if this looks like an OOB bootloader that doesn't understand GetFeatures
                if error_str.contains("Unknown message") || 
                   error_str.contains("Failure: Unknown message") ||
                   error_str.contains("Unexpected response") {
                    
                    println!("🔧 Device may be in OOB bootloader mode, trying Initialize message...");
                    
                    // Try the direct approach using keepkey-rust's proven method
                    match try_oob_bootloader_detection(device).await {
                        Ok(features) => {
                            println!("✅ Successfully detected OOB bootloader mode for device {}", device.unique_id);
                            Ok(features)
                        }
                        Err(oob_err) => {
                            println!("❌ OOB bootloader detection also failed for {}: {}", device.unique_id, oob_err);
                            Err(format!("Failed to get device features: {} (OOB attempt: {})", error_str, oob_err))
                        }
                    }
                } else {
                    Err(format!("Failed to get device features: {}", error_str))
                }
            }
            Err(_) => {
                Err("Timeout while fetching device features".to_string())
            }
        }
    } else {
        // Fallback to the old method if queue manager is not available
        println!("⚠️ DeviceQueueManager not available, using fallback method");
        
        // Check PIN flow status before fallback too
        if crate::commands::is_device_in_pin_flow(&device.unique_id) {
            return Err("Device is in PIN flow - skipping fallback feature fetch".to_string());
        }
        
        // Create a temporary device queue to fetch features
        // This is a non-blocking operation that will fail fast if device is busy
        let queue_handle = keepkey_rust::device_queue::DeviceQueueFactory::spawn_worker(
            device.unique_id.clone(),
            device.clone()
        );
        
        // Try to get features with a timeout
        match tokio::time::timeout(Duration::from_secs(DEVICE_OPERATION_TIMEOUT_SECS), queue_handle.get_features()).await {
            Ok(Ok(raw_features)) => {
                // Convert features to our DeviceFeatures format
                let device_features = crate::commands::convert_features_to_device_features(raw_features);
                Ok(device_features)
            }
            Ok(Err(e)) => Err(format!("Failed to get device features: {}", e)),
            Err(_) => Err("Timeout while fetching device features".to_string()),
        }
    }
}

/// Try to detect OOB bootloader mode using the proven keepkey-rust methods
/// This handles the case where older bootloaders don't understand GetFeatures messages
/// Uses the documented OOB detection heuristics from docs/usb/oob_mode_detection.md
async fn try_oob_bootloader_detection(device: &FriendlyUsbDevice) -> Result<keepkey_rust::features::DeviceFeatures, String> {
    println!("🔧 Attempting OOB bootloader detection via HID for device {}", device.unique_id);
    
    // Use keepkey-rust's proven fallback method that handles OOB bootloaders correctly
    let result = tokio::task::spawn_blocking({
        let device = device.clone();
        move || -> Result<keepkey_rust::features::DeviceFeatures, String> {
            // Use the robust USB/HID fallback helper which includes retries and OOB heuristics
            keepkey_rust::features::get_device_features_with_fallback(&device)
                .map_err(|e| e.to_string())
        }
    }).await;
    
    match result {
        Ok(Ok(features)) => {
            // Apply OOB detection heuristics from docs/usb/oob_mode_detection.md
            let likely_oob_bootloader = 
                features.bootloader_mode ||
                features.version == "Legacy Bootloader" ||
                features.version.contains("0.0.0") ||
                (!features.initialized && features.version.starts_with("1."));
            
            if likely_oob_bootloader {
                println!("🔧 Device {} appears to be in OOB bootloader mode (version: {}, bootloader_mode: {}, initialized: {})", 
                        device.unique_id, features.version, features.bootloader_mode, features.initialized);
            } else {
                println!("🔧 Device {} appears to be in OOB wallet mode (version: {}, initialized: {})", 
                        device.unique_id, features.version, features.initialized);
            }
            
            Ok(features)
        }
        Ok(Err(e)) => Err(e),
        Err(e) => Err(format!("Task execution error: {}", e)),
    }
}

/// Helper function to log portfolio summary for all paired devices
async fn log_all_devices_portfolio_summary(cache_manager: &std::sync::Arc<crate::cache::CacheManager>) -> Result<(), anyhow::Error> {
    use anyhow::anyhow;
    
    // Get all device metadata from cache
    let all_metadata = cache_manager.get_all_device_metadata().await.unwrap_or_default();
    
    if all_metadata.is_empty() {
        println!("📊 No paired devices found in cache");
        return Ok(());
    }
    
    let mut total_portfolio_value = 0.0;
    let mut device_summaries = Vec::new();
    let mut all_balances_debug = Vec::new();
    
    for metadata in &all_metadata {
        // Get portfolio balances for this device
        let balances = cache_manager.get_device_portfolio(&metadata.device_id).await.unwrap_or_default();
        
        // Calculate total USD value for this device and track individual balances
        let mut device_total = 0.0;
        let mut device_balances_detail = Vec::new();
        
        for balance in &balances {
            if let Ok(value) = balance.value_usd.parse::<f64>() {
                device_total += value;
                
                // Only log balances with non-zero value
                if value > 0.0 {
                    device_balances_detail.push((
                        balance.ticker.clone().unwrap_or_else(|| "UNKNOWN".to_string()),
                        balance.balance.clone(),
                        value,
                        balance.caip.clone(),
                        balance.address.clone(),
                        balance.pubkey.clone(),
                    ));
                }
            }
        }
        
        total_portfolio_value += device_total;
        
        let device_label = metadata.label.as_deref().unwrap_or("Unnamed KeepKey");
        let device_short = &metadata.device_id[metadata.device_id.len().saturating_sub(8)..];
        
        device_summaries.push((device_label.to_string(), device_short.to_string(), device_total, balances.len()));
        all_balances_debug.push((device_label.to_string(), metadata.device_id.clone(), device_balances_detail));
    }
    
    // Log the summary
    println!("📊 ===============================================");
    println!("📊 PORTFOLIO SUMMARY - ALL PAIRED DEVICES");
    println!("📊 ===============================================");
    println!("💰 TOTAL PORTFOLIO VALUE: ${:.2} USD", total_portfolio_value);
    println!("🔌 PAIRED DEVICES: {}", all_metadata.len());
    println!("📊 ===============================================");
    
    for (label, device_short, value, balance_count) in device_summaries {
        if value > 0.0 {
            println!("   🏷️ {}: ${:.2} USD ({} assets) [{}]", label, value, balance_count, device_short);
        } else {
            println!("   🏷️ {}: $0.00 USD (no balances) [{}]", label, device_short);
        }
    }
    
    println!("📊 ===============================================");
    
    // Log detailed balances for debugging
    println!("\n🔍 DETAILED BALANCE BREAKDOWN:");
    println!("================================================");
    for (device_label, device_id, balances) in all_balances_debug {
        if !balances.is_empty() {
            println!("\n📱 Device: {} ({})", device_label, &device_id[device_id.len().saturating_sub(8)..]);
            println!("   Individual Balances:");
            
            // Sort balances by value descending
            let mut sorted_balances = balances;
            sorted_balances.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
            
            for (ticker, balance, value_usd, caip, address, pubkey) in sorted_balances {
                println!("   - {}: {} = ${:.2} USD", ticker, balance, value_usd);
                if value_usd > 10.0 {  // Only show details for significant balances
                    println!("     CAIP: {}", caip);
                    if let Some(addr) = address {
                        println!("     Address: {}", addr);
                    }
                    println!("     Pubkey: {}", pubkey);
                }
            }
        }
    }
    println!("================================================");
    
    // Check for potential duplicates
    let mut balance_map: std::collections::HashMap<String, Vec<(String, f64)>> = std::collections::HashMap::new();
    
    for metadata in &all_metadata {
        let balances = cache_manager.get_device_portfolio(&metadata.device_id).await.unwrap_or_default();
        
        for balance in &balances {
            if let Ok(value) = balance.value_usd.parse::<f64>() {
                if value > 0.0 {
                    let key = format!("{}-{}-{}", 
                        balance.caip,
                        balance.address.as_deref().unwrap_or("no-address"),
                        balance.balance
                    );
                    balance_map.entry(key).or_insert_with(Vec::new).push((metadata.device_id.clone(), value));
                }
            }
        }
    }
    
    // Log any duplicates found
    let mut found_duplicates = false;
    for (key, entries) in balance_map.iter() {
        if entries.len() > 1 {
            if !found_duplicates {
                println!("\n⚠️  POTENTIAL DUPLICATE BALANCES DETECTED:");
                println!("================================================");
                found_duplicates = true;
            }
            println!("   Balance key: {}", key);
            for (device_id, value) in entries {
                println!("     - Device {}: ${:.2}", &device_id[device_id.len().saturating_sub(8)..], value);
            }
        }
    }
    
    if !found_duplicates {
        println!("\n✅ No duplicate balances detected");
    }
    
    Ok(())
}

// Create and manage event controller with proper Arc<Mutex<>> wrapper
pub fn spawn_event_controller(app: &AppHandle) -> Arc<tokio::sync::Mutex<EventController>> {
    let mut controller = EventController::new();
    controller.start(app);
    
    let controller_arc = Arc::new(tokio::sync::Mutex::new(controller));
    
    // Store the controller in app state so it can be properly cleaned up
    app.manage(controller_arc.clone());
    
    controller_arc
}
