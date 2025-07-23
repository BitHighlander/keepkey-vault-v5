use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use keepkey_rust::friendly_usb::FriendlyUsbDevice;

/// Service for tracking device identity across USB re-enumerations
pub struct DeviceIdentityService {
    // Map serial number to last known device info
    device_map: Arc<Mutex<HashMap<String, DeviceInfo>>>,
}

#[derive(Clone, Debug)]
struct DeviceInfo {
    last_seen: std::time::Instant,
    usb_addresses: Vec<String>,
    device_id: Option<String>,
    label: Option<String>,
}

impl DeviceIdentityService {
    pub fn new() -> Self {
        Self {
            device_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Check if two devices are actually the same physical device
    pub fn are_same_device(&self, old_id: &str, new_device: &FriendlyUsbDevice) -> bool {
        // First check serial number
        if let Some(serial) = &new_device.serial_number {
            let map = self.device_map.lock().unwrap();
            if let Some(info) = map.get(serial) {
                return info.usb_addresses.contains(&old_id.to_string());
            }
        }
        
        // Fallback to timing-based heuristic
        // If a device disconnects and a new one appears within 2 seconds,
        // they might be the same device
        false
    }
    
    /// Update device information
    pub fn update_device(&self, device: &FriendlyUsbDevice, device_id: Option<String>) {
        if let Some(serial) = &device.serial_number {
            let mut map = self.device_map.lock().unwrap();
            
            if let Some(info) = map.get_mut(serial) {
                info.last_seen = std::time::Instant::now();
                if !info.usb_addresses.contains(&device.unique_id) {
                    info.usb_addresses.push(device.unique_id.clone());
                }
                if device_id.is_some() {
                    info.device_id = device_id;
                }
            } else {
                map.insert(serial.clone(), DeviceInfo {
                    last_seen: std::time::Instant::now(),
                    usb_addresses: vec![device.unique_id.clone()],
                    device_id,
                    label: None,
                });
            }
        }
    }
}

lazy_static::lazy_static! {
    pub static ref DEVICE_IDENTITY_SERVICE: DeviceIdentityService = DeviceIdentityService::new();
}
