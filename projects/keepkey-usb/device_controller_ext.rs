use crate::device_controller::DeviceController;
use crate::blocking_actions::BlockingActionType;
use crate::device_registry::DEVICE_REGISTRY;

impl DeviceController {
    /// Get a device by ID
    pub fn get_device(&self, device_id: &str) -> Option<crate::device_registry::DeviceEntry> {
        DEVICE_REGISTRY.lock().ok()
            .and_then(|registry| registry.get(device_id).cloned())
    }
    
    /// Update device bootloader version
    pub fn update_device_bootloader_version(&mut self, device_id: &str, version: &str) {
        if let Ok(mut registry) = DEVICE_REGISTRY.lock() {
            if let Some(device_entry) = registry.get_mut(device_id) {
                if let Some(ref mut features) = device_entry.features {
                    features.bootloader_version = Some(version.to_string());
                }
            }
        }
    }
    
    /// Update device firmware version
    pub fn update_device_firmware_version(&mut self, device_id: &str, version: &str) {
        if let Ok(mut registry) = DEVICE_REGISTRY.lock() {
            if let Some(device_entry) = registry.get_mut(device_id) {
                if let Some(ref mut features) = device_entry.features {
                    features.version = version.to_string();
                }
            }
        }
    }
    
    /// Resolve a blocking action for a device
    pub async fn resolve_blocking_action(
        &mut self,
        device_id: &str,
        action_type: &BlockingActionType
    ) -> Result<bool, String> {
        let blocking_actions = self.blocking_actions.registry();
        if blocking_actions.lock().unwrap().remove_action(device_id, action_type.clone()) {
            // Action was removed successfully
            // Emit updated counts
            let count = blocking_actions.lock().unwrap().total_action_count();
            // Emit updated count through tauri events
            // We'll use the tauri event system directly from the controller
            log::info!("Updated blocking actions count: {}", count);
            Ok(true)
        } else {
            log::warn!("No action of type {:?} to remove for device {}", action_type, device_id);
            Ok(false)
        }
    }
    
    /// Check if a device has a specific blocking action
    pub async fn device_has_blocking_action(
        &self,
        device_id: &str,
        action_type: &BlockingActionType
    ) -> bool {
        let blocking_actions = self.blocking_actions.registry();
        let actions = blocking_actions.lock().unwrap().get_actions_for_device(device_id);
        
        actions.iter().any(|a| &a.action_type == action_type)
    }
}

// Add this to lib.rs to import this extension module
// mod device_controller_ext;
