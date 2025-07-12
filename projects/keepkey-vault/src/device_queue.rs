use keepkey_rust::{friendly_usb::FriendlyUsbDevice, features::{get_device_features_with_fallback, DeviceFeatures}, list_devices};
use std::collections::HashMap;
use std::thread;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum DeviceEvent {
    Connected(FriendlyUsbDevice),
    Disconnected(String), // unique_id or serial
    StateChanged(FriendlyUsbDevice, DeviceFeatures),
    QueueStatus { pending: usize, active: usize },
}

pub struct DeviceQueue {
    devices: Arc<Mutex<HashMap<String, thread::JoinHandle<()>>>>,
    event_tx: broadcast::Sender<DeviceEvent>,
    // You can add a command queue here if needed
}

impl DeviceQueue {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            devices: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
        }
    }

    pub fn start(&self) {
        let event_tx = self.event_tx.clone();
        let devices = self.devices.clone();
        thread::spawn(move || {
            // Poll for device changes every second
            let mut known_devices = HashMap::new();
            loop {
                let detected = list_devices();
                let mut current_ids = vec![];
                for dev in detected.iter() {
                    let id = dev.unique_id.clone();
                    current_ids.push(id.clone());
                    if !known_devices.contains_key(&id) {
                        // New device
                        known_devices.insert(id.clone(), dev.clone());
                        let _ = event_tx.send(DeviceEvent::Connected(dev.clone()));
                        // Start device thread
                        let event_tx2 = event_tx.clone();
                        let dev2 = dev.clone();
                        let handle = thread::spawn(move || {
                            // Poll device state every 2s
                            loop {
                                match get_device_features_with_fallback(&dev2) {
                                    Ok(features) => {
                                        let _ = event_tx2.send(DeviceEvent::StateChanged(dev2.clone(), features));
                                    },
                                    Err(_) => {}
                                }
                                thread::sleep(Duration::from_secs(2));
                            }
                        });
                        devices.lock().unwrap().insert(id.clone(), handle);
                    }
                }
                // Check for disconnected devices
                let prev_ids: Vec<_> = known_devices.keys().cloned().collect();
                for id in prev_ids {
                    if !current_ids.contains(&id) {
                        known_devices.remove(&id);
                        let _ = event_tx.send(DeviceEvent::Disconnected(id.clone()));
                        if let Some(handle) = devices.lock().unwrap().remove(&id) {
                            // Optionally join the thread
                            // let _ = handle.join();
                        }
                    }
                }
                thread::sleep(Duration::from_secs(1));
            }
        });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DeviceEvent> {
        self.event_tx.subscribe()
    }

    // Add command APIs here (firmware_upload, wipe, audit, etc.)
}
