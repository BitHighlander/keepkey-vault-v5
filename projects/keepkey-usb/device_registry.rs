use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;
use crate::features::DeviceFeatures;
use crate::usb_manager::FriendlyUsbDevice;
use crate::device_queue::DeviceQueueHandle;

// Define a struct to hold device data including features and queue handle
#[derive(Clone, Debug)]
pub struct DeviceEntry {
    pub device: FriendlyUsbDevice,
    pub features: Option<DeviceFeatures>,
    pub last_updated: u64, // Unix timestamp in seconds
    pub queue_handle: Option<DeviceQueueHandle>, // Handle for device communication
}

// Serializable version for sending to frontend (excluding queue handle)
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DeviceEntrySerializable {
    pub device: FriendlyUsbDevice,
    pub features: Option<DeviceFeatures>,
    pub last_updated: u64,
}

impl From<&DeviceEntry> for DeviceEntrySerializable {
    fn from(entry: &DeviceEntry) -> Self {
        Self {
            device: entry.device.clone(),
            features: entry.features.clone(),
            last_updated: entry.last_updated,
        }
    }
}

// Global device registry - this replaces the single DEVICE_FEATURES
pub static DEVICE_REGISTRY: Lazy<Arc<Mutex<HashMap<String, DeviceEntry>>>> = Lazy::new(|| {
    Arc::new(Mutex::new(HashMap::new()))
});

// Helper functions for working with the registry
pub fn add_or_update_device(device: FriendlyUsbDevice, features: Option<DeviceFeatures>) -> Result<(), String> {
    let mut registry = DEVICE_REGISTRY.lock().map_err(|e| e.to_string())?;
    
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    
    // Check if device already exists to preserve queue handle
    let queue_handle = registry.get(&device.unique_id)
        .and_then(|entry| entry.queue_handle.clone());
    
    registry.insert(device.unique_id.clone(), DeviceEntry {
        device,
        features,
        last_updated: timestamp,
        queue_handle,
    });
    
    Ok(())
}

// Add or update device with queue handle
pub fn add_or_update_device_with_queue(
    device: FriendlyUsbDevice, 
    features: Option<DeviceFeatures>,
    queue_handle: DeviceQueueHandle
) -> Result<(), String> {
    let mut registry = DEVICE_REGISTRY.lock().map_err(|e| e.to_string())?;
    
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    
    registry.insert(device.unique_id.clone(), DeviceEntry {
        device,
        features,
        last_updated: timestamp,
        queue_handle: Some(queue_handle),
    });
    
    Ok(())
}

pub fn remove_device(device_id: &str) -> Result<bool, String> {
    let mut registry = DEVICE_REGISTRY.lock().map_err(|e| e.to_string())?;
    
    // Shutdown the queue handle if it exists before removing
    if let Some(entry) = registry.get(device_id) {
        if let Some(ref queue_handle) = entry.queue_handle {
            // Attempt graceful shutdown, but don't block removal if it fails
            let handle = queue_handle.clone();
            tokio::spawn(async move {
                if let Err(e) = handle.shutdown().await {
                    log::warn!("Failed to shutdown device queue for {}: {}", handle.device_id(), e);
                }
            });
        }
    }
    
    Ok(registry.remove(device_id).is_some())
}

pub fn get_all_devices() -> Result<Vec<FriendlyUsbDevice>, String> {
    let registry = DEVICE_REGISTRY.lock().map_err(|e| e.to_string())?;
    Ok(registry.values().map(|entry| entry.device.clone()).collect())
}

pub fn get_device_features(device_id: &str) -> Result<Option<DeviceFeatures>, String> {
    let registry = DEVICE_REGISTRY.lock().map_err(|e| e.to_string())?;
    Ok(registry.get(device_id).and_then(|entry| entry.features.clone()))
}

pub fn get_all_device_entries() -> Result<Vec<DeviceEntry>, String> {
    let registry = DEVICE_REGISTRY.lock().map_err(|e| e.to_string())?;
    Ok(registry.values().cloned().collect())
}

// Get serializable device entries for frontend
pub fn get_all_device_entries_serializable() -> Result<Vec<DeviceEntrySerializable>, String> {
    let registry = DEVICE_REGISTRY.lock().map_err(|e| e.to_string())?;
    Ok(registry.values().map(|entry| entry.into()).collect())
}

// Get device queue handle
pub fn get_device_queue_handle(device_id: &str) -> Result<Option<DeviceQueueHandle>, String> {
    let registry = DEVICE_REGISTRY.lock().map_err(|e| e.to_string())?;
    Ok(registry.get(device_id).and_then(|entry| entry.queue_handle.clone()))
}

// Get the first available device queue handle (for backward compatibility)
pub fn get_first_device_queue_handle() -> Result<Option<DeviceQueueHandle>, String> {
    let registry = DEVICE_REGISTRY.lock().map_err(|e| e.to_string())?;
    Ok(registry.values()
        .find(|entry| entry.device.is_keepkey && entry.queue_handle.is_some())
        .and_then(|entry| entry.queue_handle.clone()))
}

// Get the first connected device's features (for backward compatibility)
pub fn get_first_device_features() -> Result<Option<DeviceFeatures>, String> {
    let registry = DEVICE_REGISTRY.lock().map_err(|e| e.to_string())?;
    Ok(registry.values()
        .find(|entry| entry.device.is_keepkey)
        .and_then(|entry| entry.features.clone()))
}

// Clear all devices from the registry
pub fn clear_registry() -> Result<(), String> {
    let mut registry = DEVICE_REGISTRY.lock().map_err(|e| e.to_string())?;
    registry.clear();
    Ok(())
} 