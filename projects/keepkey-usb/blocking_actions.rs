use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Types of blocking actions that must be completed before a device can be used
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BlockingActionType {
    #[serde(rename = "mandatory_bootloader_update")]
    MandatoryBootloaderUpdate,
    
    #[serde(rename = "firmware_update")]
    FirmwareUpdate,
    
    #[serde(rename = "device_initialization")]
    DeviceInitialization,
    
    #[serde(rename = "device_communication_failure")]
    DeviceCommunicationFailure,
    
    // Future blocking action types can be added here
    // #[serde(rename = "pin_required")]
    // PinRequired,
}

/// A blocking action for a specific device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockingAction {
    pub device_id: String,
    pub action_type: BlockingActionType,
    pub message: String,
    pub priority: u8, // Higher number = higher priority
    pub current_version: Option<String>,
    pub required_version: Option<String>,
    // Additional metadata as needed
}

impl BlockingAction {
    /// Create a new bootloader update blocking action
    pub fn new_bootloader_update(
        device_id: &str, 
        current_version: &str, 
        required_version: &str
    ) -> Self {
        Self {
            device_id: device_id.to_string(),
            action_type: BlockingActionType::MandatoryBootloaderUpdate,
            message: format!("Bootloader update from v{} to v{} required", current_version, required_version),
            priority: 100, // Highest priority (bootloader updates are critical)
            current_version: Some(current_version.to_string()),
            required_version: Some(required_version.to_string()),
        }
    }
    
    /// Create a new firmware update blocking action
    pub fn new_firmware_update(
        device_id: &str, 
        current_version: &str, 
        target_version: &str
    ) -> Self {
        Self {
            device_id: device_id.to_string(),
            action_type: BlockingActionType::FirmwareUpdate,
            message: format!("Firmware update from v{} to v{} available", current_version, target_version),
            priority: 50, // Medium priority (important but not as critical as bootloader)
            current_version: Some(current_version.to_string()),
            required_version: Some(target_version.to_string()),
        }
    }
    
    /// Create a new device initialization blocking action
    pub fn new_device_initialization(
        device_id: &str
    ) -> Self {
        Self {
            device_id: device_id.to_string(),
            action_type: BlockingActionType::DeviceInitialization,
            message: "Device needs to be initialized".to_string(),
            priority: 80, // High priority but not as critical as bootloader
            current_version: None,
            required_version: None,
        }
    }
    
    /// Create a new device communication failure blocking action
    pub fn new_communication_failure(
        device_id: &str, 
        error_details: &str
    ) -> Self {
        Self {
            device_id: device_id.to_string(),
            action_type: BlockingActionType::DeviceCommunicationFailure,
            message: format!("Device {} cannot communicate: {}", device_id, error_details),
            priority: 110, // Highest priority - above bootloader updates
            current_version: None,
            required_version: None,
        }
    }
}

/// Registry for tracking all blocking actions across devices
#[derive(Debug, Default)]
pub struct BlockingActionsRegistry {
    // Map of device_id -> list of blocking actions for that device
    actions: HashMap<String, Vec<BlockingAction>>,
}

impl BlockingActionsRegistry {
    pub fn new() -> Self {
        Self {
            actions: HashMap::new(),
        }
    }

    /// Add a new blocking action for a device
    pub fn add_action(&mut self, action: BlockingAction) {
        let device_actions = self.actions
            .entry(action.device_id.clone())
            .or_insert_with(Vec::new);
        
        // Check if this action type already exists for this device
        if let Some(existing_idx) = device_actions
            .iter()
            .position(|a| a.action_type == action.action_type) 
        {
            // Replace the existing action
            device_actions[existing_idx] = action;
        } else {
            // Add new action
            device_actions.push(action);
            // Sort by priority (descending)
            device_actions.sort_by(|a, b| b.priority.cmp(&a.priority));
        }
    }

    /// Remove a blocking action for a device
    pub fn remove_action(&mut self, device_id: &str, action_type: BlockingActionType) -> bool {
        if let Some(device_actions) = self.actions.get_mut(device_id) {
            let initial_len = device_actions.len();
            device_actions.retain(|action| action.action_type != action_type);
            
            // If we actually removed something
            if device_actions.len() < initial_len {
                // Remove empty entries
                if device_actions.is_empty() {
                    self.actions.remove(device_id);
                }
                return true;
            }
        }
        false
    }

    /// Get all blocking actions for a specific device, sorted by priority
    pub fn get_actions_for_device(&self, device_id: &str) -> Vec<BlockingAction> {
        self.actions
            .get(device_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get the highest priority blocking action for a device (if any)
    pub fn get_highest_priority_action(&self, device_id: &str) -> Option<BlockingAction> {
        self.actions
            .get(device_id)
            .and_then(|actions| actions.first().cloned())
    }

    /// Get all blocking actions across all devices
    pub fn get_all_actions(&self) -> Vec<BlockingAction> {
        self.actions
            .values()
            .flat_map(|actions| actions.clone())
            .collect()
    }

    /// Check if a device has any blocking actions
    pub fn has_blocking_actions(&self, device_id: &str) -> bool {
        self.actions
            .get(device_id)
            .map_or(false, |actions| !actions.is_empty())
    }

    /// Get count of devices with blocking actions
    pub fn device_count_with_actions(&self) -> usize {
        self.actions.len()
    }

    /// Get total count of blocking actions across all devices
    pub fn total_action_count(&self) -> usize {
        self.actions
            .values()
            .map(|actions| actions.len())
            .sum()
    }
}

/// Threadsafe wrapper for the BlockingActionsRegistry
#[derive(Debug, Default)]
#[derive(Clone)]
pub struct BlockingActionsState {
    registry: Arc<Mutex<BlockingActionsRegistry>>,
}

impl BlockingActionsState {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Mutex::new(BlockingActionsRegistry::new())),
        }
    }

    // Clone the internal Arc<Mutex<>> to share with other components
    pub fn registry(&self) -> Arc<Mutex<BlockingActionsRegistry>> {
        self.registry.clone()
    }
}
