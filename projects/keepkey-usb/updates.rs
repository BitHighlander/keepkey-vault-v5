use semver::Version;
use tauri::{AppHandle, State};
use crate::blocking_actions::{BlockingActionType, BlockingActionsState};
use crate::device_registry::DEVICE_REGISTRY;
use crate::transport::{ProtocolAdapter, UsbTransport, hid::HidTransport};
use crate::messages;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use rusb::{Context, Device, UsbContext};

fn find_usb_device(_device_id: &str) -> Option<Device<Context>> {
    // Get the USB context
    let context = Context::new().ok()?;
    
    // Known KeepKey VID/PIDs  
    const KEEPKEY_VID: u16 = 0x2B24;
    const KEEPKEY_PID_MAIN: u16 = 0x0001;
    const KEEPKEY_PID_BOOTLOADER: u16 = 0x0002;
    
    // List all devices and find the matching one
    let devices = context.devices().ok()?;
    
    for device in devices.iter() {
        let descriptor = device.device_descriptor().ok()?;
        
        // Check if it's a KeepKey device
        if descriptor.vendor_id() == KEEPKEY_VID &&
           (descriptor.product_id() == KEEPKEY_PID_MAIN || 
            descriptor.product_id() == KEEPKEY_PID_BOOTLOADER) {
            
            // Return the device - bootloader mode can be on either PID
            // The device features already confirmed it's in bootloader mode
            return Some(device);
        }
    }
    
    None
}

#[tauri::command]
pub async fn update_device_bootloader(
    device_id: String,
    target_version: String,
    _app_handle: AppHandle,
    blocking_actions: State<'_, BlockingActionsState>,
) -> Result<bool, String> {
    
    log::info!("Starting bootloader update for device {}: target version {}", device_id, target_version);
    
    // Debug: Log current working directory and environment
    let cwd = std::env::current_dir().unwrap_or_default();
    log::info!("Current working directory: {:?}", cwd);
    
    // Debug: List contents of current directory
    if let Ok(entries) = fs::read_dir(&cwd) {
        let contents: Vec<String> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        log::info!("Current directory contents: {:?}", contents);
    }
    
    // Debug: Check for parent directories
    if let Some(parent) = cwd.parent() {
        log::info!("Parent directory: {:?}", parent);
        if let Ok(entries) = fs::read_dir(parent) {
            let contents: Vec<String> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            log::info!("Parent directory contents: {:?}", contents);
        }
    }
    
    // Validate target version
    let _target_semver = Version::parse(&target_version)
        .map_err(|e| format!("Invalid target bootloader version: {}", e))?;
    
    // Check if device is connected
    let device = DEVICE_REGISTRY.lock()
        .map_err(|e| format!("Failed to access device registry: {}", e))?
        .get(&device_id)
        .cloned()
        .ok_or_else(|| format!("Device {} not found", device_id))?;
    
    log::info!("Found device: {:?}", device.device.product);
    
    // Load the bootloader binary from the firmware directory
    let bootloader_filename = format!("bl_v{}", target_version);
    
    // Get executable location for bundled app paths
    let exe_path = std::env::current_exe().ok();
    let exe_dir = exe_path.as_ref().and_then(|p| p.parent());
    
    if let Some(exe_dir) = &exe_dir {
        log::info!("Executable directory: {:?}", exe_dir);
    }
    
    // Build a comprehensive list of possible paths
    let mut possible_firmware_paths = vec![
        // Development paths
        PathBuf::from("../../keepkey-rust/firmware").join(&bootloader_filename).join("blupdater.bin"), // From vault-v2/src-tauri
        PathBuf::from("../firmware").join(&bootloader_filename).join("blupdater.bin"), // From vault/src-tauri
        PathBuf::from("firmware").join(&bootloader_filename).join("blupdater.bin"), // From keepkey-rust root
        PathBuf::from("./firmware").join(&bootloader_filename).join("blupdater.bin"), // Explicit current dir
        cwd.join("firmware").join(&bootloader_filename).join("blupdater.bin"), // Absolute from cwd
    ];
    
    // Add executable-relative paths for bundled apps
    if let Some(exe_dir) = exe_dir {
        // Common bundled app locations
        possible_firmware_paths.push(exe_dir.join("firmware").join(&bootloader_filename).join("blupdater.bin"));
        possible_firmware_paths.push(exe_dir.join("../Resources/firmware").join(&bootloader_filename).join("blupdater.bin")); // macOS
        possible_firmware_paths.push(exe_dir.join("../firmware").join(&bootloader_filename).join("blupdater.bin"));
        possible_firmware_paths.push(exe_dir.join("../../firmware").join(&bootloader_filename).join("blupdater.bin"));
        
        // Try to find keepkey-rust relative to executable
        possible_firmware_paths.push(exe_dir.join("../../keepkey-rust/firmware").join(&bootloader_filename).join("blupdater.bin"));
        possible_firmware_paths.push(exe_dir.join("../../../keepkey-rust/firmware").join(&bootloader_filename).join("blupdater.bin"));
    }
    
    // Debug: Log all paths being tried
    log::info!("Trying to find bootloader file. Checking paths:");
    for (i, path) in possible_firmware_paths.iter().enumerate() {
        log::info!("  Path {}: {:?} - exists: {}", i + 1, path, path.exists());
        
        // Also check if the parent firmware directory exists
        if let Some(firmware_dir) = path.parent() {
            if let Some(parent) = firmware_dir.parent() {
                log::info!("    Parent dir: {:?} - exists: {}", parent, parent.exists());
            }
        }
    }
    
    let firmware_path = possible_firmware_paths.iter().find(|path| path.exists()).cloned();
    
    let bootloader_bytes = if let Some(path) = firmware_path {
        log::info!("Loading bootloader from: {}", path.display());
        fs::read(&path)
            .map_err(|e| format!("Failed to read bootloader file {}: {}", path.display(), e))?
    } else {
        // Check available bootloader versions from all possible firmware directories
        let possible_firmware_dirs = [
            PathBuf::from("../../keepkey-rust/firmware"), // From vault-v2/src-tauri
            PathBuf::from("../firmware"), // From vault/src-tauri  
            PathBuf::from("firmware"), // From keepkey-rust root
        ];
        
        let mut available_versions = String::from("No firmware directory found");
        for firmware_dir in &possible_firmware_dirs {
            if firmware_dir.exists() {
                match fs::read_dir(firmware_dir) {
                    Ok(entries) => {
                        let versions: Vec<String> = entries
                            .filter_map(|e| e.ok())
                            .map(|e| e.file_name().to_string_lossy().to_string())
                            .filter(|name| name.starts_with("bl_v"))
                            .collect();
                        available_versions = format!("Available bootloader versions in {}: {}", 
                            firmware_dir.display(), versions.join(", "));
                        break;
                    }
                    Err(_) => continue,
                }
            }
        }
        
        return Err(format!(
            "Bootloader file not found: bl_v{}/blupdater.bin in any firmware directory. {}. Target version was: {}",
            target_version,
            available_versions,
            target_version
        ));
    };
    
    log::info!("Loaded bootloader binary: {} bytes", bootloader_bytes.len());
    
    // Check if we have features to determine if device is in bootloader mode
    if let Some(features) = &device.features {
        if !features.bootloader_mode {
            return Err("Device is not in bootloader mode. Current firmware version indicates normal mode.".to_string());
        }
        log::info!("Device confirmed in bootloader mode, firmware version: {}", features.version);
    }
    
    log::info!("Device is in bootloader mode. Attempting bootloader update...");
    
    // Try USB transport first (preferred for updates when permissions allow)
    let upload_result = match find_usb_device(&device_id) {
        Some(usb_device) => {
            log::info!("Found USB device, attempting USB transport...");
            match UsbTransport::new(&usb_device, 0) {
                Ok((mut transport, _, _)) => {
                    log::info!("USB transport created successfully");
                    log::info!("Uploading bootloader via USB ({} bytes)...", bootloader_bytes.len());
                    
                    let mut handler = transport.with_standard_handler();
                    
                    log::info!("⚠️  IMPORTANT: Check your KeepKey device screen!");
                    log::info!("    You may need to press the button to confirm the update.");
                    log::info!("    The v1.0.3 bootloader requires manual confirmation.");
                    log::info!("    If you see 'Upload' on the device screen, press and hold the button.");
                    
                    // For very old bootloaders, add a small delay to ensure device is ready
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    
                    // For v1.0.3 bootloader, we need to send FirmwareErase first
                    log::info!("Sending FirmwareErase command for v1.0.3 bootloader compatibility...");
                    match handler.handle(messages::FirmwareErase::default().into()) {
                        Ok(messages::Message::Success(s)) => {
                            log::info!("FirmwareErase successful: {}", s.message());
                        }
                        Ok(messages::Message::Failure(f)) => {
                            log::error!("FirmwareErase failed: {}", f.message());
                            return Err(format!("Bootloader erase failed: {}", f.message()));
                        }
                        Ok(other) => {
                            log::warn!("Unexpected response during erase: {:?}", other);
                        }
                        Err(e) => {
                            log::error!("Error during FirmwareErase: {}", e);
                            return Err(format!("Error during bootloader erase: {}", e));
                        }
                    }
                    
                    // Now send the actual firmware upload
                    log::info!("Sending FirmwareUpload command...");
                    Some(handler.handle(
                        messages::FirmwareUpload {
                            payload_hash: Sha256::digest(&bootloader_bytes).to_vec(),
                            payload: bootloader_bytes.clone(),
                        }
                        .into()
                    ))
                }
                Err(e) => {
                    let error_str = e.to_string();
                    if error_str.contains("Access denied") || 
                       error_str.contains("insufficient permissions") ||
                       error_str.contains("LIBUSB_ERROR_ACCESS") {
                        log::warn!("USB permission denied: {}, will try HID fallback", error_str);
                        None
                    } else {
                        return Err(format!("Failed to create USB transport: {}", e));
                    }
                }
            }
        }
        None => {
            log::warn!("USB device not found, will try HID");
            None
        }
    };
    
    // If USB failed or wasn't available, try HID as fallback
    let final_result = match upload_result {
        Some(result) => result,
        None => {
            log::info!("Attempting HID transport as fallback...");
            
            // Create HID transport
            let mut transport = HidTransport::new_for_device(Some(&device_id))
                .map_err(|e| format!("Failed to create HID transport: {}", e))?;
            
            log::info!("HID transport created successfully");
            log::info!("Uploading bootloader via HID ({} bytes)...", bootloader_bytes.len());
            
            let adapter = &mut transport as &mut dyn ProtocolAdapter;
            
            log::info!("⚠️  IMPORTANT: Check your KeepKey device screen!");
            log::info!("    You may need to press the button to confirm the update.");
            log::info!("    The v1.0.3 bootloader requires manual confirmation.");
            log::info!("    If you see 'Upload' on the device screen, press and hold the button.");
            
            // For very old bootloaders, add a small delay to ensure device is ready
            std::thread::sleep(std::time::Duration::from_millis(500));
            
            // Use with_standard_handler() to automatically handle ButtonRequest messages
            let mut handler = adapter.with_standard_handler();
            
            // For v1.0.3 bootloader, we need to send FirmwareErase first
            log::info!("Sending FirmwareErase command for v1.0.3 bootloader compatibility...");
            match handler.handle(messages::FirmwareErase::default().into()) {
                Ok(messages::Message::Success(s)) => {
                    log::info!("FirmwareErase successful: {}", s.message());
                }
                Ok(messages::Message::Failure(f)) => {
                    log::error!("FirmwareErase failed: {}", f.message());
                    return Err(format!("Bootloader erase failed: {}", f.message()));
                }
                Ok(other) => {
                    log::warn!("Unexpected response during erase: {:?}", other);
                }
                Err(e) => {
                    log::error!("Error during FirmwareErase: {}", e);
                    return Err(format!("Error during bootloader erase: {}", e));
                }
            }
            
            // Now send the actual firmware upload
            log::info!("Sending FirmwareUpload command...");
            handler.handle(
                messages::FirmwareUpload {
                    payload_hash: Sha256::digest(&bootloader_bytes).to_vec(),
                    payload: bootloader_bytes,
                }
                .into()
            )
        }
    };
    
    match final_result {
        Ok(messages::Message::Success(s)) => {
            log::info!("✅ Bootloader update successful: {}", s.message());
            log::info!("Device may reboot. Please wait a moment.");
            let success = true;
            
            if success {
        // Update device bootloader version in registry
        if let Ok(mut registry) = DEVICE_REGISTRY.lock() {
            if let Some(device_entry) = registry.get_mut(&device_id) {
                if let Some(ref mut features) = device_entry.features {
                    features.bootloader_version = Some(target_version.clone());
                }
            }
        }
        
        // Resolve the blocking action
        let blocking_actions_registry = blocking_actions.registry();
        blocking_actions_registry.lock().unwrap()
            .remove_action(&device_id, BlockingActionType::MandatoryBootloaderUpdate);
        
        log::info!("Bootloader update completed successfully for device {}", device_id);
        
                // Real implementation would trigger a device reconnect or re-enumerate here
                Ok(true)
            } else {
                log::error!("Bootloader update failed for device {}", device_id);
                Err("Bootloader update failed".to_string())
            }
        }
        Ok(messages::Message::Failure(f)) => {
            log::error!("Bootloader update failed: {}", f.message());
            Err(format!("Bootloader update failed: {}", f.message()))
        }
        Ok(other) => {
            log::error!("Unexpected response during bootloader upload: {:?}", other);
            Err(format!("Unexpected response: {:?}", other))
        }
        Err(e) => {
            log::error!("Error during bootloader upload: {}", e);
            log::error!("This might mean:");
            log::error!("  1. The device needs button confirmation - check the device screen");
            log::error!("  2. The device isn't in the correct bootloader mode");
            log::error!("  3. The bootloader binary format isn't compatible");
            Err(format!("Error during bootloader upload: {}. Check device screen for prompts.", e))
        }
    }
}

#[tauri::command]
pub async fn update_device_firmware(
    device_id: String,
    target_version: String,
    _app_handle: AppHandle,
    blocking_actions: State<'_, BlockingActionsState>,
) -> Result<bool, String> {
    
    log::info!("Starting firmware update for device {}: target version {}", device_id, target_version);
    
    // Validate firmware version
    let _firmware_semver = Version::parse(&target_version)
        .map_err(|e| format!("Invalid firmware version: {}", e))?;
    
    // Check if device is connected
    let device = DEVICE_REGISTRY.lock()
        .map_err(|e| format!("Failed to access device registry: {}", e))?
        .get(&device_id)
        .cloned()
        .ok_or_else(|| format!("Device {} not found", device_id))?;
    
    log::info!("Found device: {:?}", device.device.product);
    
    // Confirm no bootloader update is pending
    let blocking_actions_registry = blocking_actions.registry();
    let actions = blocking_actions_registry.lock().unwrap()
        .get_actions_for_device(&device_id);
    let has_bootloader_update = actions.iter()
        .any(|a| a.action_type == BlockingActionType::MandatoryBootloaderUpdate);
    
    if has_bootloader_update {
        return Err("Bootloader update required before firmware update".to_string());
    }
    
    // Load the firmware binary from the firmware directory
    let firmware_filename = format!("v{}", target_version);
    
    // Try multiple possible paths for the firmware directory
    let possible_firmware_paths = [
        PathBuf::from("../../keepkey-rust/firmware").join(&firmware_filename).join("firmware.keepkey.bin"), // From vault-v2/src-tauri
        PathBuf::from("../firmware").join(&firmware_filename).join("firmware.keepkey.bin"), // From vault/src-tauri
        PathBuf::from("firmware").join(&firmware_filename).join("firmware.keepkey.bin"), // From keepkey-rust root
    ];
    
    let firmware_path = possible_firmware_paths.iter().find(|path| path.exists()).cloned();
    
    let firmware_bytes = if let Some(path) = firmware_path {
        log::info!("Loading firmware from: {}", path.display());
        fs::read(&path)
            .map_err(|e| format!("Failed to read firmware file {}: {}", path.display(), e))?
    } else {
        return Err(format!("Firmware file not found: v{}/firmware.keepkey.bin in any firmware directory", target_version));
    };
    
    log::info!("Loaded firmware binary: {} bytes", firmware_bytes.len());
    
    // Check if device is in bootloader mode (required for firmware update)
    if let Some(features) = &device.features {
        if !features.bootloader_mode {
            return Err("Device must be in bootloader mode for firmware update. Please hold the button while reconnecting.".to_string());
        }
    }
    
    // Try USB transport first, then fall back to HID if permissions fail
    let mut usb_transport_opt = None;
    let mut hid_transport_opt = None;
    
    // Try USB first
    if let Some(usb_device) = find_usb_device(&device_id) {
        match UsbTransport::new(&usb_device, 0) {
            Ok((transport, _, _)) => {
                log::info!("Using USB transport for firmware update");
                usb_transport_opt = Some(transport);
            }
            Err(e) => {
                let error_str = e.to_string();
                if error_str.contains("Access denied") || 
                   error_str.contains("insufficient permissions") ||
                   error_str.contains("LIBUSB_ERROR_ACCESS") {
                    log::warn!("USB permission denied: {}, will try HID fallback", error_str);
                } else {
                    return Err(format!("Failed to create USB transport: {}", e));
                }
            }
        }
    }
    
    // If USB failed or wasn't available, try HID
    if usb_transport_opt.is_none() {
        log::info!("Attempting HID transport...");
        let hid_transport = HidTransport::new_for_device(Some(&device_id))
            .map_err(|e| format!("Failed to create HID transport: {}", e))?;
        log::info!("Using HID transport for firmware update");
        hid_transport_opt = Some(hid_transport);
    }
    
    // Perform the firmware update with whichever transport is available
    if let Some(mut usb_transport) = usb_transport_opt {
        // USB transport path
        log::info!("Erasing firmware sectors via USB...");
        {
            let mut erase_handler = usb_transport.with_standard_handler();
            match erase_handler.handle(messages::FirmwareErase::default().into()) {
                Ok(messages::Message::Success(s)) => {
                    log::info!("Firmware erase successful: {}", s.message());
                }
                Ok(messages::Message::Failure(f)) => {
                    return Err(format!("Firmware erase failed: {}. Aborting update.", f.message()));
                }
                Ok(other) => {
                    return Err(format!("Unexpected response during firmware erase: {:?}. Aborting update.", other));
                }
                Err(e) => return Err(format!("Error during firmware erase: {}. Aborting update.", e)),
            }
        } // erase_handler is dropped here
        
        // Then upload firmware via USB
        log::info!("Uploading firmware via USB ({} bytes)...", firmware_bytes.len());
        let mut upload_handler = usb_transport.with_standard_handler();
        match upload_handler.handle(
        messages::FirmwareUpload {
            payload_hash: Sha256::digest(&firmware_bytes).to_vec(),
            payload: firmware_bytes,
        }
        .into()
    ) {
        Ok(messages::Message::Success(s)) => {
            log::info!("✅ Firmware update successful: {}", s.message());
            log::info!("Device may reboot. Please wait a moment.");
            let success = true;
            
            if success {
                // Update device firmware version in registry
                if let Ok(mut registry) = DEVICE_REGISTRY.lock() {
                    if let Some(device_entry) = registry.get_mut(&device_id) {
                        if let Some(ref mut features) = device_entry.features {
                            features.version = target_version.clone();
                        }
                    }
                }
                
                // Resolve the blocking action
                let blocking_actions_registry = blocking_actions.registry();
                blocking_actions_registry.lock().unwrap()
                    .remove_action(&device_id, BlockingActionType::FirmwareUpdate);
                
                log::info!("Firmware update completed successfully for device {}", device_id);
                
                Ok(true)
            } else {
                log::error!("Firmware update failed for device {}", device_id);
                Err("Firmware update failed".to_string())
            }
        }
        Ok(messages::Message::Failure(f)) => {
            log::error!("Firmware update failed: {}", f.message());
            Err(format!("Firmware update failed: {}", f.message()))
        }
        Ok(other) => {
            log::error!("Unexpected response during firmware upload: {:?}", other);
            Err(format!("Unexpected response: {:?}", other))
        }
        Err(e) => Err(format!("Error during firmware upload: {}", e)),
    }
    } else if let Some(mut hid_transport) = hid_transport_opt {
        // HID transport path
        log::info!("Erasing firmware sectors via HID...");
        let adapter = &mut hid_transport as &mut dyn ProtocolAdapter;
        {
            let mut erase_handler = adapter.with_standard_handler();
            match erase_handler.handle(messages::FirmwareErase::default().into()) {
                Ok(messages::Message::Success(s)) => {
                    log::info!("Firmware erase successful: {}", s.message());
                }
                Ok(messages::Message::Failure(f)) => {
                    return Err(format!("Firmware erase failed: {}. Aborting update.", f.message()));
                }
                Ok(other) => {
                    return Err(format!("Unexpected response during firmware erase: {:?}. Aborting update.", other));
                }
                Err(e) => return Err(format!("Error during firmware erase: {}. Aborting update.", e)),
            }
        } // erase_handler is dropped here
        
        // Then upload firmware via HID
        log::info!("Uploading firmware via HID ({} bytes)...", firmware_bytes.len());
        let mut upload_handler = adapter.with_standard_handler();
        match upload_handler.handle(
            messages::FirmwareUpload {
                payload_hash: Sha256::digest(&firmware_bytes).to_vec(),
                payload: firmware_bytes,
            }
            .into()
        ) {
            Ok(messages::Message::Success(s)) => {
                log::info!("✅ Firmware update successful: {}", s.message());
                log::info!("Device may reboot. Please wait a moment.");
                let success = true;
                
                if success {
                    // Update device firmware version in registry
                    if let Ok(mut registry) = DEVICE_REGISTRY.lock() {
                                            if let Some(device_entry) = registry.get_mut(&device_id) {
                        if let Some(ref mut features) = device_entry.features {
                            features.version = target_version.clone();
                        }
                        }
                    }
                    
                    // Resolve the blocking action
                    let blocking_actions_registry = blocking_actions.registry();
                    blocking_actions_registry.lock().unwrap()
                        .remove_action(&device_id, BlockingActionType::FirmwareUpdate);
                    
                    log::info!("Firmware update completed successfully for device {}", device_id);
                    
                    Ok(true)
                } else {
                    log::error!("Firmware update failed for device {}", device_id);
                    Err("Firmware update failed".to_string())
                }
            }
            Ok(messages::Message::Failure(f)) => {
                log::error!("Firmware update failed: {}", f.message());
                Err(format!("Firmware update failed: {}", f.message()))
            }
            Ok(other) => {
                log::error!("Unexpected response during firmware upload: {:?}", other);
                Err(format!("Unexpected response: {:?}", other))
            }
            Err(e) => Err(format!("Error during firmware upload: {}", e)),
        }
    } else {
        Err("Failed to create any transport (USB or HID)".to_string())
    }
}

#[tauri::command]
pub async fn update_resolve_blocking_action(
    device_id: String,
    action_type: BlockingActionType,
    blocking_actions: State<'_, BlockingActionsState>,
) -> Result<bool, String> {
    log::info!("Resolving blocking action: {:?} for device {}", action_type, device_id);
    
    let blocking_actions_registry = blocking_actions.registry();
    let result = blocking_actions_registry.lock().unwrap()
        .remove_action(&device_id, action_type);
    
    if result {
        log::info!("Successfully resolved blocking action for device {}", device_id);
    } else {
        log::warn!("No blocking action to resolve for device {}", device_id);
    }
    
    Ok(result)
}
