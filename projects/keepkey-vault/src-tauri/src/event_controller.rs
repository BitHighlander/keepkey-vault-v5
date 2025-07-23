use keepkey_rust::friendly_usb::FriendlyUsbDevice;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::interval;
use tokio_util::sync::CancellationToken;

// Device operation timeout - matches the timeout used in commands.rs
const DEVICE_OPERATION_TIMEOUT_SECS: u64 = 30;
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
    // Track devices with connection state
    known_devices: Arc<Mutex<HashMap<String, DeviceConnectionInfo>>>,
}

impl EventController {
    pub fn new() -> Self {
        Self {
            cancellation_token: CancellationToken::new(),
            task_handle: None,
            is_running: false,
            known_devices: Arc::new(Mutex::new(HashMap::new())),
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
                        println!("🔍 DEBUG: Device scan #{} - found {} devices", loop_count, current_devices.len());
                        
                        // Track device connections and reconnections
                        {
                            let mut known = known_devices.lock().unwrap();
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
                        }
                        
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
                                    
                                    // 🚀 Trigger automatic frontload for ready devices
                                    println!("🚀 Device {} is ready, starting automatic frontload...", device_for_features.unique_id);
                                    
                                    let device_id_for_frontload = device_for_features.unique_id.clone();
                                    let app_for_frontload = app_for_features.clone();
                                    
                                    tauri::async_runtime::spawn(async move {
                                        println!("🔄 Starting automatic frontload for device: {}", device_id_for_frontload);
                                        
                                        // Get the cache manager from app state
                                        if let Some(cache_state) = app_for_frontload.try_state::<std::sync::Arc<once_cell::sync::OnceCell<std::sync::Arc<crate::cache::CacheManager>>>>() {
                                            match crate::commands::get_cache_manager(cache_state.inner()).await {
                                                Ok(cache_manager) => {
                                                    // Get device queue manager from app state
                                                    if let Some(queue_state) = app_for_frontload.try_state::<crate::commands::DeviceQueueManager>() {
                                                        let device_queue_manager = queue_state.inner().clone();
                                                        
                                                        // Create frontload controller
                                                        let frontload_controller = crate::cache::FrontloadController::new(
                                                            cache_manager,
                                                            device_queue_manager,
                                                        );
                                                        
                                                        // Start frontload
                                                        match frontload_controller.frontload_device(&device_id_for_frontload).await {
                                                            Ok(_) => {
                                                                println!("✅ Automatic frontload completed successfully for device: {}", device_id_for_frontload);
                                                                
                                                                // Emit frontload completion event
                                                                let _ = app_for_frontload.emit("cache:frontload-completed", serde_json::json!({
                                                                    "device_id": device_id_for_frontload,
                                                                    "success": true
                                                                }));
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
                                                        println!("⚠️ Failed to get device queue manager for automatic frontload");
                                                    }
                                                }
                                                Err(e) => {
                                                    println!("⚠️ Failed to get cache manager for automatic frontload: {}", e);
                                                }
                                            }
                                        } else {
                                            println!("⚠️ Failed to get cache state for automatic frontload");
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
                            let features_payload = serde_json::json!({
                                                                                "deviceId": device_for_features.unique_id,
                                                "features": features,
                                                "status": status  // Use evaluated status instead of hardcoded "ready"
                            });
                            
                                                                            if let Err(e) = crate::commands::emit_or_queue_event(&app_for_features, "device:features-updated", features_payload).await {
                                println!("❌ Failed to emit/queue device:features-updated event: {}", e);
                            } else {
                                                                                println!("📡 Successfully emitted/queued device:features-updated for {}", device_for_features.unique_id);
                            }
                                        }
                                        Err(e) => {
                                            println!("❌ Failed to get features for {}: {}", device_for_features.unique_id, e);
                                            
                                            // Check for timeout errors specifically
                                            if e.contains("Timeout while fetching device features") {
                                                println!("⏱️ Device timeout detected - device may be in invalid state");
                                                println!("❌ OOPS this should never happen - device communication failed!");
                                                
                                                // Log detailed error for debugging
                                                eprintln!("ERROR: Device timeout indicates invalid state - this should be prevented!");
                                                eprintln!("Device ID: {}", device_for_features.unique_id);
                                                eprintln!("Error: {}", e);
                                                
                                                // Emit device invalid state event for UI to handle
                                                let invalid_state_payload = serde_json::json!({
                                                    "deviceId": device_for_features.unique_id,
                                                    "error": e,
                                                    "errorType": "DEVICE_TIMEOUT",
                                                    "status": "invalid_state"
                                                });
                                                let _ = app_for_features.emit("device:invalid-state", &invalid_state_payload);
                                                
                                                // Also emit status update
                                                let _ = app_for_features.emit("status:update", serde_json::json!({
                                                    "status": "Device timeout - please reconnect"
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
                        let mut known = known_devices.lock().unwrap();
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
                            let mut known = known_devices.lock().unwrap();
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

// Create and manage event controller with proper Arc<Mutex<>> wrapper
pub fn spawn_event_controller(app: &AppHandle) -> Arc<Mutex<EventController>> {
    let mut controller = EventController::new();
    controller.start(app);
    
    let controller_arc = Arc::new(Mutex::new(controller));
    
    // Store the controller in app state so it can be properly cleaned up
    app.manage(controller_arc.clone());
    
    controller_arc
}
