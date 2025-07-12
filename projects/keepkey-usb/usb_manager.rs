use anyhow::Result;
use rusb::UsbContext;
use serde::{Serialize, Deserialize};
use std::sync::mpsc::{self, Receiver, Sender};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
#[cfg(target_os = "windows")]
use std::time::Duration; // Only used in Windows polling code
use tauri::{AppHandle, Emitter}; // Import Emitter trait for emit method

/// Event sent to the frontend when device state changes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceEvent {
    pub connected: bool,
    pub features: Option<crate::features::DeviceFeatures>,
}

// KeepKey Vendor ID
const KEEPKEY_VID: u16 = 0x2B24;
// For Windows polling when hotplug is not available
const WINDOWS_POLL_INTERVAL_MS: u64 = 1000; // 1 second interval

/// Basic information about a USB device, obtained directly from the system.
#[derive(Debug, Clone)]
struct RawUsbDevice {
    vid: u16,
    pid: u16,
    manufacturer_string: Option<String>,
    product_string: Option<String>,
    serial_number: Option<String>,
    // A system-specific unique identifier (e.g., bus/port path or OS-assigned ID)
    system_id: String,
}

/// User-friendly representation of a USB device, intended for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FriendlyUsbDevice {
    pub unique_id: String, // This will be our primary key for managing devices
    pub name: String,      // Combination of product and manufacturer, or a default
    pub vid: u16,
    pub pid: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    pub is_keepkey: bool,
    // Potentially add: pub features: Option<DeviceFeatures> for KeepKeys later
}

impl FriendlyUsbDevice {
    fn from_raw(raw: &RawUsbDevice) -> Self {
        let name = match (&raw.product_string, &raw.manufacturer_string) {
            (Some(p), Some(m)) => format!("{} - {}", m, p),
            (Some(p), None) => p.clone(),
            (None, Some(m)) => m.clone(),
            (None, None) => format!("USB Device (VID: {:04x}, PID: {:04x})", raw.vid, raw.pid),
        };
        FriendlyUsbDevice {
            unique_id: raw.system_id.clone(),
            name,
            vid: raw.vid,
            pid: raw.pid,
            manufacturer: raw.manufacturer_string.clone(),
            product: raw.product_string.clone(),
            serial_number: raw.serial_number.clone(),
            is_keepkey: raw.vid == KEEPKEY_VID,
        }
    }
}

/// Manages the state of connected USB devices and handles hotplug events.
pub struct DeviceManager {
    // app_handle to emit events to the frontend
    app_handle: AppHandle,
    // Thread-safe storage for connected devices, keyed by their unique_id
    devices: Arc<Mutex<HashMap<String, FriendlyUsbDevice>>>,
    // Flag to control the hotplug listening thread
    hotplug_running_status: Arc<Mutex<bool>>,
    // Channel for device events
    event_sender: Sender<DeviceEvent>,
    event_receiver: Arc<Mutex<Receiver<DeviceEvent>>>,
    // Channel to send device updates to DeviceController
    device_controller_tx: Option<tokio::sync::mpsc::Sender<(FriendlyUsbDevice, bool)>>,
}

impl DeviceManager {
    pub fn new(app_handle: AppHandle) -> Self {
        let (sender, receiver) = mpsc::channel();
        DeviceManager {
            app_handle,
            devices: Arc::new(Mutex::new(HashMap::new())),
            hotplug_running_status: Arc::new(Mutex::new(false)),
            event_sender: sender,
            event_receiver: Arc::new(Mutex::new(receiver)),
            device_controller_tx: None,
        }
    }

    /// Set the device controller channel
    pub fn set_device_controller_tx(&mut self, tx: tokio::sync::mpsc::Sender<(FriendlyUsbDevice, bool)>) {
        self.device_controller_tx = Some(tx);
    }

    /// Starts listening for USB hotplug events.
    /// This will spawn a new thread for rusb hotplug on Linux/macOS,
    /// or a polling thread on Windows.
    pub fn start_listening(&mut self) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.spawn_rusb_hotplug_listener()?;
        }
        
        #[cfg(target_os = "windows")]
        {
            self.spawn_windows_poll_listener()?;
        }

        Ok(())
    }

    /// Stops listening for USB hotplug events.
    pub fn stop_listening(&self) -> Result<()> {
        let mut running = self.hotplug_running_status.lock().unwrap();
        *running = false;
        Ok(())
    }

    /// Returns a list of currently connected friendly USB devices.
    pub fn get_connected_devices(&self) -> Vec<FriendlyUsbDevice> {
        let devices_lock = self.devices.lock().unwrap();
        devices_lock.values().cloned().collect()
    }

    /// Waits for and returns the next device event, blocking until one is available
    pub fn next_event_blocking(&self) -> Option<DeviceEvent> {
        let receiver = self.event_receiver.lock().unwrap();
        match receiver.recv() {
            Ok(event) => Some(event),
            Err(_) => None,
        }
    }

    /// Check for device features and emit event if state changed
    pub fn check_features_and_emit(&self) {
        // This method is now deprecated - device feature fetching is handled by DeviceController
        log::debug!("check_features_and_emit called - now handled by DeviceController");
    }

    // --- Platform-specific hotplug implementations ---

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn spawn_rusb_hotplug_listener(&mut self) -> Result<()> {
        log::info!("Starting rusb hotplug listener");
        let app_handle_clone = self.app_handle.clone();
        let devices_clone = Arc::clone(&self.devices);
        let running_status_clone = Arc::clone(&self.hotplug_running_status);
        let device_controller_tx = self.device_controller_tx.clone();

        // Set running to true before spawning the thread
        {
            let mut running = running_status_clone.lock().unwrap();
            *running = true;
        }

        std::thread::spawn(move || {
            // libusb context must live on the same thread that calls handle_events()
            match rusb::Context::new() {
                Ok(mut context) => {
                    let _registration = rusb::HotplugBuilder::new()
                        .enumerate(true) // Enumerate devices already present
                        .register(&mut context, Box::new(RusbHotplugHandler {
                            app_handle: app_handle_clone.clone(),
                            devices: devices_clone.clone(),
                            device_controller_tx: device_controller_tx.clone(),
                        }));

                    match _registration {
                        Ok(_reg) => {
                            log::info!("rusb hotplug registration successful");
                            let mut running = running_status_clone.lock().unwrap();
                            while *running {
                                // Dereference running inside the loop condition
                                drop(running); // Release lock before blocking call
                                // The handle_events method expects Option<Duration>
                                if let Err(e) = context.handle_events(Some(std::time::Duration::from_millis(500))) {
                                    log::error!("rusb handle_events error: {}", e);
                                    // Decide if we need to break or attempt to recover
                                    // For now, we'll just log and continue, but a persistent error might require stopping.
                                }
                                // Re-acquire lock for the next check
                                running = running_status_clone.lock().unwrap();
                            }
                            log::info!("rusb hotplug listener stopping.");
                            // Registration is dropped here, unregistering the callback
                        }
                        Err(e) => {
                            log::error!("Failed to register rusb hotplug callback: {}", e);
                            // Set running to false if registration fails
                            let mut running = running_status_clone.lock().unwrap();
                            *running = false;
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to initialize libusb context: {}", e);
                    // Set running to false if context init fails
                    let mut running = running_status_clone.lock().unwrap();
                    *running = false;
                }
            }
        });
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn spawn_windows_poll_listener(&mut self) -> Result<()> {
        log::info!("Starting Windows USB polling listener");
        let app_handle_clone = self.app_handle.clone();
        let devices_clone = Arc::clone(&self.devices);
        let running_status_clone = Arc::clone(&self.hotplug_running_status);
        let device_controller_tx = self.device_controller_tx.clone();

        // Set running to true before spawning the thread
        {
            let mut running = running_status_clone.lock().unwrap();
            *running = true;
        }

        std::thread::spawn(move || {
            log::info!("Windows USB polling thread started");
            let mut running = running_status_clone.lock().unwrap();
            
            while *running {
                // Release the mutex during sleep
                drop(running);
                
                // Poll for USB device changes
                if let Err(e) = Self::scan_and_update_all_devices(
                    app_handle_clone.clone(),
                    devices_clone.clone(),
                    device_controller_tx.clone()
                ) {
                    log::error!("Error during Windows device poll: {}", e);
                }
                
                // Sleep for the poll interval
                std::thread::sleep(Duration::from_millis(WINDOWS_POLL_INTERVAL_MS));
                
                // Re-acquire the mutex for the next loop iteration
                running = running_status_clone.lock().unwrap();
            }
            
            log::info!("Windows USB polling thread stopped");
        });

        Ok(())
    }
    
    // This function works for all platforms, so we're removing the Windows-specific tag
    pub fn scan_and_update_all_devices(
        app_handle: AppHandle,
        devices_arc: Arc<Mutex<HashMap<String, FriendlyUsbDevice>>>,
        device_controller_tx: Option<tokio::sync::mpsc::Sender<(FriendlyUsbDevice, bool)>>
    ) -> Result<()> {
        let mut current_system_devices = HashMap::new();
        if let Ok(rusb_devices) = rusb::devices() {
            for device in rusb_devices.iter() {
                let device_desc = match device.device_descriptor() {
                    Ok(d) => d,
                    Err(_) => continue, // Skip devices we can't get a descriptor for
                };

                let vid = device_desc.vendor_id();
                let pid = device_desc.product_id();
                
                // Try to open the device to read strings
                let (manufacturer_string, product_string, serial_number) = match device.open() {
                    Ok(handle) => {
                        let timeout = std::time::Duration::from_millis(100);
                        let lang = match handle.read_languages(timeout) {
                            Ok(l) if !l.is_empty() => Some(l[0]),
                            _ => None,
                        };

                        let manufacturer = lang.and_then(|l| handle.read_manufacturer_string(l, &device_desc, timeout).ok());
                        let product = lang.and_then(|l| handle.read_product_string(l, &device_desc, timeout).ok());
                        let serial = lang.and_then(|l| handle.read_serial_number_string(l, &device_desc, timeout).ok());
                        
                        (manufacturer, product, serial)
                    }
                    Err(e) => {
                        // If it's a KeepKey device and we can't open it, still add it with default values
                        if vid == KEEPKEY_VID {
                            log::warn!("Could not open KeepKey device {:04x}:{:04x}: {}. Using default values.", vid, pid, e);
                            (Some("KeyHodlers, LLC".to_string()), Some("KeepKey".to_string()), None)
                        } else {
                            continue; // Skip non-KeepKey devices we can't open
                        }
                    }
                };
                
                // Create a unique system_id for the device
                let mut use_serial_as_id = false;
                if let Some(sn) = &serial_number {
                    if !sn.is_empty() {
                        use_serial_as_id = true;
                    }
                }

                let system_id = if use_serial_as_id {
                    serial_number.clone().unwrap() // Safe: checked is_some (implicitly by if let) and !is_empty
                } else {
                    // Serial is None or Some("")
                    if vid == KEEPKEY_VID {
                        if serial_number.is_some() { // Implies Some("") because use_serial_as_id is false
                            log::info!("KeepKey device (VID:{:04x}, PID:{:04x}) with empty serial. Using fallback ID in scan.", vid, pid);
                        } else { // Implies None
                            log::info!("KeepKey device (VID:{:04x}, PID:{:04x}) with no serial. Using fallback ID in scan.", vid, pid);
                        }
                        format!("keepkey_{:04x}_{:04x}_bus{}_addr{}", 
                                vid, pid, 
                                device.bus_number(), 
                                device.address())
                    } else {
                        // Non-KeepKey device, and serial is None or Some("")
                        if serial_number.is_some() { // Implies Some("")
                             log::info!("Non-KeepKey device (VID:{:04x}, PID:{:04x}) with empty serial. Using bus/address fallback ID in scan.", vid, pid);
                        } else { // Implies None
                             log::info!("Non-KeepKey device (VID:{:04x}, PID:{:04x}) with no serial. Using bus/address fallback ID in scan.", vid, pid);
                        }
                        format!("bus{}_addr{}", device.bus_number(), device.address())
                    }
                };
                
                // Create the raw device structure
                let raw_device = RawUsbDevice {
                    vid,
                    pid,
                    manufacturer_string,
                    product_string,
                    serial_number,
                    system_id: system_id.clone(),
                };
                current_system_devices.insert(system_id, FriendlyUsbDevice::from_raw(&raw_device));
            }
        }

        let mut managed_devices = devices_arc.lock().unwrap();
        let old_keys: std::collections::HashSet<_> = managed_devices.keys().cloned().collect();
        let new_keys: std::collections::HashSet<_> = current_system_devices.keys().cloned().collect();

        // Removed devices
        for unique_id in old_keys.difference(&new_keys) {
            if let Some(removed_device) = managed_devices.remove(unique_id) {
                log::info!("Device disconnected (scan): {} ({:04x}:{:04x})", removed_device.name, removed_device.vid, removed_device.pid);
                if let Err(e) = app_handle.emit("usb-device-disconnected", removed_device.unique_id.clone()) {
                    log::error!("Failed to emit usb-device-disconnected event: {}", e);
                }
                
                // Remove from global registry
                if removed_device.is_keepkey {
                    if let Some(controller_tx) = &device_controller_tx {
                        // Send disconnect event to controller
                        let tx = controller_tx.clone();
                        let device_clone = removed_device.clone();
                        
                        std::thread::spawn(move || {
                            if let Err(e) = futures::executor::block_on(tx.send((device_clone, false))) {
                                log::error!("Failed to send device disconnect to controller: {}", e);
                            }
                        });
                    } else {
                        // Fallback: remove from registry directly
                        if let Err(e) = crate::device_registry::remove_device(&removed_device.unique_id) {
                            log::error!("Failed to remove device from registry: {}", e);
                        }
                    }
                }
            }
        }

        // Added or changed devices
        for unique_id in new_keys {
            let system_device = current_system_devices.get(&unique_id).unwrap(); // Should exist
            if !managed_devices.contains_key(&unique_id) {
                log::info!("Device connected (scan): {} ({:04x}:{:04x})", system_device.name, system_device.vid, system_device.pid);
                managed_devices.insert(unique_id.clone(), system_device.clone());
                if let Err(e) = app_handle.emit("usb-device-connected", system_device.clone()) {
                    log::error!("Failed to emit usb-device-connected event: {}", e);
                }
                
                // Send to device controller for async processing
                if let Some(controller_tx) = &device_controller_tx {
                    // Clone the sender for async operation
                    let tx = controller_tx.clone();
                    let device_clone = system_device.clone();
                    
                    // Use a separate thread to avoid blocking the USB callback
                    std::thread::spawn(move || {
                        // Convert std mpsc to tokio mpsc using blocking send
                        if let Err(e) = futures::executor::block_on(tx.send((device_clone, true))) {
                            log::error!("Failed to send device to controller: {}", e);
                        }
                    });
                } else {
                    // Fallback: add to registry without features if no controller
                    if system_device.is_keepkey {
                        if let Err(e) = crate::device_registry::add_or_update_device(system_device.clone(), None) {
                            log::error!("Failed to add device to registry: {}", e);
                        }
                    }
                }
            }
            // We could also check if device details changed for existing unique_ids, but that's more complex.
        }
        Ok(())
    }

    /// Manually trigger a full USB device scan and update the device list
    pub fn refresh_devices(&self) -> Result<()> {
        Self::scan_and_update_all_devices(
            self.app_handle.clone(),
            Arc::clone(&self.devices),
            self.device_controller_tx.clone()
        )
    }
}

// --- rusb Hotplug Callback Handler (Linux/macOS) ---
#[cfg(any(target_os = "linux", target_os = "macos"))]
struct RusbHotplugHandler {
    app_handle: AppHandle,
    devices: Arc<Mutex<HashMap<String, FriendlyUsbDevice>>>,
    device_controller_tx: Option<tokio::sync::mpsc::Sender<(FriendlyUsbDevice, bool)>>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl rusb::Hotplug<rusb::Context> for RusbHotplugHandler {
    fn device_arrived(&mut self, device: rusb::Device<rusb::Context>) {
        let device_desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Failed to get device descriptor on arrival: {}", e);
                return;
            }
        };

        // Try to open the device to read string descriptors
        let raw_device_result: Result<RawUsbDevice, anyhow::Error> = match device.open() {
            Ok(handle) => {
                let timeout = std::time::Duration::from_millis(50);
                // Attempt to get the first language
                let lang = match handle.read_languages(timeout) {
                    Ok(langs) if !langs.is_empty() => Some(langs[0]),
                    Ok(_) => { log::debug!("Device {:04x}:{:04x} has no languages.", device_desc.vendor_id(), device_desc.product_id()); None },
                    Err(e) => { log::warn!("Failed to read languages for {:04x}:{:04x}: {}", device_desc.vendor_id(), device_desc.product_id(), e); None },
                };

                let manufacturer_string = lang.and_then(|l| handle.read_manufacturer_string(l, &device_desc, timeout).ok());
                let product_string = lang.and_then(|l| handle.read_product_string(l, &device_desc, timeout).ok());
                let serial_number = lang.and_then(|l| handle.read_serial_number_string(l, &device_desc, timeout).ok());
                
                // Unique ID: Serial number is best. Fallback to bus/address.
                // Note: bus/address can change if the device is plugged into a different port.
                // A more robust system might involve udev properties on Linux or IOKit properties on macOS.
                // Create a unique ID. Bus/address can change, so serial is best if available.
                // For KeepKey devices, we need to be more careful with the ID generation
                // since Windows may not consistently provide serial numbers on reconnects
                let system_id = if device_desc.vendor_id() == KEEPKEY_VID {
                    // For KeepKey devices, we'll use a more stable ID format
                    // If serial is available, use it as the primary identifier
                    // Otherwise, use a combination of VID/PID and bus/address
                    serial_number.clone().unwrap_or_else(|| {
                        log::info!("KeepKey device without serial number detected, using fallback ID");
                        format!("keepkey_{:04x}_{:04x}_bus{}_addr{}", 
                                device_desc.vendor_id(), 
                                device_desc.product_id(), 
                                device.bus_number(), 
                                device.address())
                    })
                } else {
                    // For non-KeepKey devices, use the standard approach
                    serial_number.clone().unwrap_or_else(|| format!("bus{}_addr{}", device.bus_number(), device.address()))
                };

                Ok(RawUsbDevice {
                    vid: device_desc.vendor_id(),
                    pid: device_desc.product_id(),
                    manufacturer_string,
                    product_string,
                    serial_number,
                    system_id,
                })
            }
            Err(e) => {
                log::warn!("Could not open device {:04x}:{:04x} to get details: {}. Using VID/PID only.", device_desc.vendor_id(), device_desc.product_id(), e);
                // Fallback if we can't open the device (e.g., permissions, or already exclusively opened)
                Ok(RawUsbDevice {
                    vid: device_desc.vendor_id(),
                    pid: device_desc.product_id(),
                    manufacturer_string: None,
                    product_string: None,
                    serial_number: None,
                    system_id: format!("bus{}_addr{}", device.bus_number(), device.address()), // Fallback ID
                })
            }
        };

        if let Ok(raw_device) = raw_device_result {
            let friendly_device = FriendlyUsbDevice::from_raw(&raw_device);
            log::info!("Device arrived: {} ({:04x}:{:04x}) ID: {}", friendly_device.name, friendly_device.vid, friendly_device.pid, friendly_device.unique_id);
            let mut devices_lock = self.devices.lock().unwrap();
            devices_lock.insert(friendly_device.unique_id.clone(), friendly_device.clone());
            if let Err(e) = self.app_handle.emit("usb-device-connected", friendly_device.clone()) {
                log::error!("Failed to emit usb-device-connected event: {}", e);
            }
            
            // Send to device controller for async processing
            if let Some(controller_tx) = &self.device_controller_tx {
                // Clone the sender for async operation
                let tx = controller_tx.clone();
                let device_clone = friendly_device.clone();
                
                // Use a separate thread to avoid blocking the USB callback
                std::thread::spawn(move || {
                    // Convert std mpsc to tokio mpsc using blocking send
                    if let Err(e) = futures::executor::block_on(tx.send((device_clone, true))) {
                        log::error!("Failed to send device to controller: {}", e);
                    }
                });
            } else {
                // Fallback: add to registry without features if no controller
                if friendly_device.is_keepkey {
                    if let Err(e) = crate::device_registry::add_or_update_device(friendly_device.clone(), None) {
                        log::error!("Failed to add device to registry: {}", e);
                    }
                }
            }
        }
    }

    fn device_left(&mut self, device: rusb::Device<rusb::Context>) {
        // Construct a potential unique_id like we do on arrival for fallback.
        // Ideally, we'd have stored the exact ID when it arrived.
        // This part is tricky because the device is gone, so getting serial or even bus/addr might fail or be inaccurate.
        // The `enumerate(true)` on HotplugBuilder helps populate initial devices with full details.
        // For devices that were present and then left, we rely on the ID stored in our map.
        
        // A simple way: iterate our known devices and see if VID/PID and bus/address match.
        // This is not perfectly robust if multiple identical devices are on the same bus/address sequentially.
        // A better approach for `device_left` might involve the OS providing a persistent ID if rusb allows it.
        // For now, we'll try to find it by a generated ID if serial was not available.
        let bus = device.bus_number();
        let address = device.address();
        let potential_id_bus_addr = format!("bus{}_addr{}", bus, address);

        let mut devices_lock = self.devices.lock().unwrap();
        let mut _found_id: Option<String> = None;

        // Try to find by VID/PID and bus/address (less reliable but a fallback)
        for (id, dev) in devices_lock.iter() {
            // This check is weak. If serial numbers were consistently available and used as unique_id, it would be better.
            if dev.vid == device.device_descriptor().map_or(0, |d| d.vendor_id()) &&
               dev.pid == device.device_descriptor().map_or(0, |d| d.product_id()) &&
               id.ends_with(&potential_id_bus_addr) && dev.serial_number.is_none() {
                _found_id = Some(id.clone());
                break;
            }
        }
        
        // If not found by bus/addr, it implies it might have had a serial number.
        // The `device` object passed to `device_left` might not have full details anymore.
        // This is a known challenge with USB hotplug; often you only get a minimal identifier for removal.
        // The best is to rely on the `system_id` that was created on arrival.
        // However, rusb's `Device` object here might be a new instance representing the detached device.
        // We need a way to correlate it. If `device.sysfs_path()` or similar were available and stable, that could work.
        // For now, we assume the `enumerate(true)` and initial scan populate our map correctly,
        // and `device_left` is more of a signal that *something* left, prompting a re-check or using a simple identifier.

        // Let's try to find it primarily by iterating what we have and matching basic properties if serial was not the key.
        // This part needs robust testing. The `unique_id` should be the key.
        // If `rusb` gives us enough info on `device_left` to reconstruct the *exact same* `unique_id` as on `device_arrived`,
        // then we can just use that. Serial number is the best candidate for this.

        let device_desc_opt = device.device_descriptor().ok();
        let serial_handle_opt = device.open().ok().and_then(|handle| {
            device_desc_opt.as_ref().and_then(|desc| {
                handle.read_languages(std::time::Duration::from_millis(20)).ok().and_then(|langs| {
                    if langs.is_empty() { None } else { Some((handle, desc, langs[0])) }
                })
            })
        });
        let serial_opt = serial_handle_opt.and_then(|(handle, desc, lang)| {
            handle.read_serial_number_string(lang, desc, std::time::Duration::from_millis(20)).ok()
        });

        let id_to_remove = serial_opt.unwrap_or(potential_id_bus_addr);

        if let Some(removed_device) = devices_lock.remove(&id_to_remove) {
            log::info!("Device left: {} ({:04x}:{:04x}) ID: {}", removed_device.name, removed_device.vid, removed_device.pid, removed_device.unique_id);
            if let Err(e) = self.app_handle.emit("usb-device-disconnected", removed_device.unique_id.clone()) {
                log::error!("Failed to emit usb-device-disconnected event: {}", e);
            }
            
            // Remove from global registry
            if removed_device.is_keepkey {
                if let Some(controller_tx) = &self.device_controller_tx {
                    // Send disconnect event to controller
                    let tx = controller_tx.clone();
                    let device_clone = removed_device.clone();
                    
                    std::thread::spawn(move || {
                        if let Err(e) = futures::executor::block_on(tx.send((device_clone, false))) {
                            log::error!("Failed to send device disconnect to controller: {}", e);
                        }
                    });
                } else {
                    // Fallback: remove from registry directly
                    if let Err(e) = crate::device_registry::remove_device(&removed_device.unique_id) {
                        log::error!("Failed to remove device from registry: {}", e);
                    }
                }
            }
        } else {
            // If not found by direct ID, it means our ID generation/matching on disconnect needs improvement.
            // This could happen if the serial number was the ID, but we couldn't retrieve it on disconnect.
            // Or if the bus/address changed or was not the key.
            log::warn!("Device left event for an unknown or already removed device (ID attempted: {}). This might indicate an issue with unique ID generation on disconnect or a race condition.", id_to_remove);
            // As a fallback, we might need to re-scan or notify the UI that *a* device left without specifics.
            // For now, we log. A full re-scan on every disconnect if ID is uncertain might be too heavy for Linux/macOS.
        }
    }
}



// Tauri command to list devices
#[tauri::command]
pub fn list_usb_devices(manager_state: tauri::State<'_, Arc<Mutex<DeviceManager>>>) -> Vec<FriendlyUsbDevice> {
    match manager_state.lock() {
        Ok(manager) => manager.get_connected_devices(),
        Err(e) => {
            log::error!("Failed to lock device manager: {}", e);
            // Return empty vec instead of panicking - keep the app running
            Vec::new()
        }
    }
}

// Function to initialize and start the device manager, to be called from main.rs or lib.rs setup
pub fn init_device_manager(app_handle: AppHandle) -> Arc<Mutex<DeviceManager>> {
    let mut manager = DeviceManager::new(app_handle.clone());
    if let Err(e) = manager.start_listening() {
        log::error!("Failed to start USB device listener: {}", e);
        // Handle error appropriately, maybe notify frontend or panic if critical
    }
        
    Arc::new(Mutex::new(manager))
}

// Make sure to add logging, e.g., using the `log` crate and `env_logger` or `tauri-plugin-log`.
// Example: `log::info!("Device connected: {:?}", friendly_device);`
//          `log::error!("Some error: {}", e);`
