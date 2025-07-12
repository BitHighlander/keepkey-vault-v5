use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, broadcast};
use tokio::time::interval;
use tauri::{AppHandle, Emitter};
use crate::features::{DeviceFeatures, get_device_features_with_fallback};
use crate::usb_manager::FriendlyUsbDevice;
use crate::device_registry::{DeviceEntry, add_or_update_device, add_or_update_device_with_queue, remove_device};
use crate::device_queue::{DeviceQueueFactory};
use crate::index_db::IndexDb;

use crate::blocking_actions::{BlockingActionsState, BlockingAction};

const FEATURE_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const DEVICE_SCAN_INTERVAL: Duration = Duration::from_secs(2);
const FEATURE_FETCH_TIMEOUT: Duration = Duration::from_secs(10); // Increased for better device compatibility

#[derive(Debug, Clone)]
pub enum DeviceControllerEvent {
    DeviceConnected(FriendlyUsbDevice),
    DeviceDisconnected(String), // device_id
    DeviceUpdated(DeviceEntry),
    FeaturesFetched { device_id: String, features: DeviceFeatures, status: DeviceStatus },
    FeatureFetchFailed { device_id: String, error: String },
    FeatureFetchRetrying { device_id: String, attempt: u8, max: u8 }, // NEW: for frontend status
    /// Generic status message for UI/UX, logs, or debugging. Can be sent from anywhere.
    StatusMessage {
        device_id: Option<String>,
        message: String,
    },
}


/// Background service that manages device state and caching
pub struct DeviceController {
    /// Event broadcaster for real-time updates
    event_tx: broadcast::Sender<DeviceControllerEvent>,
    /// Channel for device updates from USB manager
    device_updates_rx: mpsc::Receiver<(FriendlyUsbDevice, bool)>, // (device, is_connected)
    device_updates_tx: mpsc::Sender<(FriendlyUsbDevice, bool)>,
    /// Devices pending feature fetch
    pending_features: Arc<RwLock<HashMap<String, Instant>>>,
    /// Feature fetch retry counts
    retry_counts: Arc<RwLock<HashMap<String, u8>>>,
    /// Devices currently being processed (to prevent overlapping attempts)
    active_fetches: Arc<RwLock<HashMap<String, Instant>>>,
    /// Blocking actions state
    pub blocking_actions: BlockingActionsState,
    /// App handle for emitting events
    app_handle: AppHandle,
}

impl DeviceController {
    /// Emit a generic status message to the frontend (or any subscriber) from anywhere in the backend.
    /// Usage: DeviceController::emit_status_message(&event_tx, Some(device_id), "Attempting USB");
    pub fn emit_status_message(event_tx: &broadcast::Sender<DeviceControllerEvent>, device_id: Option<String>, message: impl Into<String>) {
        let _ = event_tx.send(DeviceControllerEvent::StatusMessage {
            device_id,
            message: message.into(),
        });
    }

    pub fn new(blocking_actions: BlockingActionsState, app_handle: AppHandle) -> (Self, mpsc::Sender<(FriendlyUsbDevice, bool)>, broadcast::Receiver<DeviceControllerEvent>) {
        let (event_tx, event_rx) = broadcast::channel(100);
        let (device_updates_tx, device_updates_rx) = mpsc::channel(100);
        
        let controller = DeviceController {
            event_tx: event_tx.clone(),
            device_updates_rx,
            device_updates_tx: device_updates_tx.clone(),
            pending_features: Arc::new(RwLock::new(HashMap::new())),
            retry_counts: Arc::new(RwLock::new(HashMap::new())),
            active_fetches: Arc::new(RwLock::new(HashMap::new())),
            blocking_actions,
            app_handle,
        };
        
        (controller, device_updates_tx, event_rx)
    }
    
    /// Start the background device monitoring service
    pub async fn run(mut self) {
        log::info!("DeviceController starting...");
        // Log number of connected devices at startup
        if let Ok(entries) = crate::device_registry::get_all_device_entries() {
            let count = entries.len();
            log::info!("{} device(s) connected", count);
        }
        
        // Spawn feature refresh task
        let pending_features = Arc::clone(&self.pending_features);
        let retry_counts = Arc::clone(&self.retry_counts);
        let active_fetches = Arc::clone(&self.active_fetches);
        let event_tx = self.event_tx.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1));
            
            loop {
                interval.tick().await;
                
                // Get devices that need feature updates
                let devices_to_update = {
                    let pending = pending_features.read().unwrap();
                    let now = Instant::now();
                    
                    pending.iter()
                        .filter(|(_, last_attempt)| now.duration_since(**last_attempt) > Duration::from_secs(5))
                        .map(|(id, _)| id.clone())
                        .collect::<Vec<_>>()
                };
                
                for device_id in devices_to_update {
                    // Check if device is already being processed
                    {
                        let active = active_fetches.read().unwrap();
                        if let Some(fetch_time) = active.get(&device_id) {
                            // If the fetch has been running for more than 15 seconds, consider it stuck and remove it
                            if Instant::now().duration_since(*fetch_time) < Duration::from_secs(15) {
                                log::debug!("Device {} is already being processed, skipping", device_id);
                                continue;
                            } else {
                                log::warn!("Device {} fetch appears stuck, removing from active list", device_id);
                                drop(active);
                                active_fetches.write().unwrap().remove(&device_id);
                            }
                        }
                    }
                    
                    // Check retry count
                    let retry_count = {
                        let counts = retry_counts.read().unwrap();
                        counts.get(&device_id).copied().unwrap_or(0)
                    };
                    // Broadcast retry attempt to frontend
                    let _ = event_tx.send(DeviceControllerEvent::FeatureFetchRetrying {
                        device_id: device_id.clone(),
                        attempt: retry_count + 1,
                        max: 3,
                    });
                    log::info!("Attempting to connect to device {} ({}/{})", device_id, retry_count + 1, 3);
                    
                    if retry_count >= 3 {
                        log::warn!("Giving up after {} attempts for device {} – please reconnect KeepKey", 3, device_id);
                        let _ = event_tx.send(DeviceControllerEvent::FeatureFetchRetrying {
                            device_id: device_id.clone(),
                            attempt: 3,
                            max: 3,
                        });
                        // Create communication failure blocking action
                        let action = BlockingAction::new_communication_failure(
                            &device_id,
                            "Failed to fetch device features after multiple attempts. Please reconnect KeepKey."
                        );
                        // Add the blocking action
                        if let Ok(mut registry) = self.blocking_actions.registry().lock() {
                            registry.add_action(action);
                        }
                        // Emit blocking actions update event to frontend
                        let action_count = self.blocking_actions.registry()
                            .lock()
                            .map(|registry| registry.total_action_count())
                            .unwrap_or(0);
                        log::info!("Emitting blocking actions update: {} actions", action_count);
                        let _ = self.app_handle.emit("blocking:actions_updated", action_count);
                        pending_features.write().unwrap().remove(&device_id);
                        retry_counts.write().unwrap().remove(&device_id);
                        active_fetches.write().unwrap().remove(&device_id);
                        continue;
                    }
                    
                    // Get device info from registry
                    if let Ok(entries) = crate::device_registry::get_all_device_entries() {
                        if let Some(entry) = entries.iter().find(|e| e.device.unique_id == device_id) {
                            let device = entry.device.clone();
                            let device_for_update = device.clone(); // Clone for the update after features are fetched
                            let device_id_clone = device_id.clone();
                            let event_tx_clone = event_tx.clone();
                            let pending_features_clone = Arc::clone(&pending_features);
                            let retry_counts_clone = Arc::clone(&retry_counts);
                            let active_fetches_clone = Arc::clone(&active_fetches);
                            
                            // Mark device as actively being processed
                            active_fetches.write().unwrap().insert(device_id.clone(), Instant::now());
                            
                            // Spawn async task to fetch features
                            tokio::spawn(async move {
                                log::info!("Fetching features for device {} (attempt {})", device_id_clone, retry_count + 1);
                                
                                // Update last attempt time
                                pending_features_clone.write().unwrap().insert(device_id_clone.clone(), Instant::now());
                                
                                // Use tokio::task::spawn_blocking for the USB operation
                                let result = tokio::time::timeout(
                                    FEATURE_FETCH_TIMEOUT,
                                    tokio::task::spawn_blocking(move || {
                                        get_device_features_with_fallback(&device)
                                    })
                                ).await;
                                
                                match result {
                                    Ok(Ok(Ok(features))) => {
                                        log::info!("Successfully fetched features for device {}", device_id_clone);
                                        
                                        // Create device queue worker
                                        log::info!("🚀 Creating device queue worker for {}", device_id_clone);
                                        let queue_handle = DeviceQueueFactory::spawn_worker(
                                            device_id_clone.clone(),
                                            device_for_update.clone()
                                        );
                                        
                                        // Update registry with queue handle
                                        if let Err(e) = add_or_update_device_with_queue(
                                            device_for_update, 
                                            Some(features.clone()),
                                            queue_handle
                                        ) {
                                            log::error!("Failed to update device registry with queue: {}", e);
                                        }
                                        
                                        // Update database with features
                                        if let Ok(db) = IndexDb::open() {
                                            if let Err(e) = db.device_connected(&device_id_clone, Some(&features)) {
                                                log::error!("Failed to update device in database: {}", e);
                                            }
                                        }
                                        
                                        // Remove from pending and retry counts
                                        pending_features_clone.write().unwrap().remove(&device_id_clone);
                                        retry_counts_clone.write().unwrap().remove(&device_id_clone);
                                        
                                        // Evaluate device status
                                        let status = evaluate_device_status(device_id_clone.clone(), &features);
                                        
                                        // Emit event
                                        let _ = event_tx_clone.send(DeviceControllerEvent::FeaturesFetched {
                                            device_id: device_id_clone.clone(),
                                            features,
                                            status,
                                        });
                                        
                                        // Clean up active fetch
                                        active_fetches_clone.write().unwrap().remove(&device_id_clone);
                                    }
                                    Ok(Ok(Err(e))) => {
                                        log::warn!("Failed to fetch features for device {}: {}", device_id_clone, e);
                                        retry_counts_clone.write().unwrap().insert(device_id_clone.clone(), retry_count + 1);
                                        
                                        let _ = event_tx_clone.send(DeviceControllerEvent::FeatureFetchFailed {
                                            device_id: device_id_clone.clone(),
                                            error: e.to_string(),
                                        });
                                        DeviceController::emit_status_message(&event_tx_clone, Some(device_id_clone.clone()), "Please reconnect your keepkey");
                                        // Clean up active fetch
                                        active_fetches_clone.write().unwrap().remove(&device_id_clone);
                                    }
                                    Ok(Err(_)) => {
                                        log::error!("Task panicked while fetching features for device {}", device_id_clone);
                                        retry_counts_clone.write().unwrap().insert(device_id_clone.clone(), retry_count + 1);
                                        
                                        // Clean up active fetch
                                        active_fetches_clone.write().unwrap().remove(&device_id_clone);
                                    }
                                    Err(_) => {
                                        log::warn!("Timeout fetching features for device {}", device_id_clone);
                                        retry_counts_clone.write().unwrap().insert(device_id_clone.clone(), retry_count + 1);
                                        
                                        let _ = event_tx_clone.send(DeviceControllerEvent::FeatureFetchFailed {
                                            device_id: device_id_clone.clone(),
                                            error: "Operation timed out".to_string(),
                                        });
                                        DeviceController::emit_status_message(&event_tx_clone, Some(device_id_clone.clone()), "Please reconnect your keepkey");
                                        // Clean up active fetch
                                        active_fetches_clone.write().unwrap().remove(&device_id_clone);
                                    }
                                }
                            });
                        }
                    }
                }
            }
        });
        
        // Main event loop
        while let Some((device, is_connected)) = self.device_updates_rx.recv().await {
            if is_connected {
                log::info!("Device connected: {} - {} ({})", 
                    device.manufacturer.as_deref().unwrap_or("Unknown"), 
                    device.name, 
                    device.unique_id
                );
                
                // Add to registry immediately with no features
                if let Err(e) = add_or_update_device(device.clone(), None) {
                    log::error!("Failed to add device to registry: {}", e);
                }
                
                // Record device connection in database (without features for now)
                if device.is_keepkey {
                    if let Ok(db) = IndexDb::open() {
                        if let Err(e) = db.device_connected(&device.unique_id, None) {
                            log::error!("Failed to record device connection in database: {}", e);
                        }
                    }
                }
                
                // Mark for feature fetch if it's a KeepKey
                if device.is_keepkey {
                    self.pending_features.write().unwrap().insert(device.unique_id.clone(), Instant::now());
                    self.retry_counts.write().unwrap().insert(device.unique_id.clone(), 0);
                }
                
                // Emit event
                let _ = self.event_tx.send(DeviceControllerEvent::DeviceConnected(device));
            } else {
                log::info!("Device disconnected: {} - {} ({})", 
                    device.manufacturer.as_deref().unwrap_or("Unknown"), 
                    device.name, 
                    device.unique_id
                );
                
                // Remove from registry
                if let Err(e) = remove_device(&device.unique_id) {
                    log::error!("Failed to remove device from registry: {}", e);
                }
                
                // Record device disconnection in database
                if device.is_keepkey {
                    if let Ok(db) = IndexDb::open() {
                        if let Err(e) = db.device_disconnected(&device.unique_id) {
                            log::error!("Failed to record device disconnection in database: {}", e);
                        }
                    }
                }
                
                // Remove from pending and active
                self.pending_features.write().unwrap().remove(&device.unique_id);
                self.retry_counts.write().unwrap().remove(&device.unique_id);
                self.active_fetches.write().unwrap().remove(&device.unique_id);
                
                // Emit event
                let _ = self.event_tx.send(DeviceControllerEvent::DeviceDisconnected(device.unique_id));
            }
        }
        
        log::info!("DeviceController shutting down");
    }
    
    /// Subscribe to device controller events
    pub fn subscribe(&self) -> broadcast::Receiver<DeviceControllerEvent> {
        self.event_tx.subscribe()
    }
} 