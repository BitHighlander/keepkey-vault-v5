# KeepKey Vault v2

A modern Tauri-based GUI application for KeepKey hardware wallets, built with clean architectural boundaries and efficient hardware communication.

## 🏗️ **Architecture Overview**

Vault v2 follows a **clean architecture pattern** with clear separation between the application layer and hardware communication:

```
┌─────────────────────────────────────┐
│         Vault-v2 (Tauri App)       │
│   • React/TypeScript Frontend      │
│   • Rust Backend (Tauri commands)  │
│   • Application Logic & State      │
└─────────────────────────────────────┘
                  │
                  ▼ High-level API calls only
┌─────────────────────────────────────┐
│         KeepKey-Rust Crate          │
│  • Device discovery & management    │
│  • USB/HID transport handling       │
│  • Protocol implementation          │
│  • Hardware abstraction layer      │
└─────────────────────────────────────┘
```

## ✅ **Clean Integration Principles**

### **What Vault-v2 Does**
- User interface and experience
- Application state management
- Tauri command handling
- Database operations
- Error presentation to users

### **What KeepKey-Rust Does**
- Hardware device discovery
- USB/HID communication
- Protocol message handling
- Transport layer management
- Hardware error translation

### **Clear Boundaries**
- Vault-v2 **NEVER** imports `rusb`, `hidapi`, or other low-level USB libraries
- All hardware operations go through keepkey-rust high-level APIs
- No direct USB device manipulation in application code

## 📦 **Dependencies**

### ✅ **Allowed Dependencies**
```toml
[dependencies]
# High-level KeepKey integration (REQUIRED)
keepkey_rust = { path = "../../keepkey-rust" }

# Tauri framework
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
tauri-plugin-sql = { version = "2", features = ["sqlite"] }

# Application utilities
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
uuid = { version = "1.0", features = ["v4"] }

# Database
rusqlite = { version = "0.31", features = ["bundled"] }
```

### ❌ **Prohibited Dependencies**
```toml
# ❌ NEVER add these to vault-v2:
# rusb = "0.9"           # Hardware USB - handled by keepkey-rust
# hidapi = "1.4"         # Hardware HID - handled by keepkey-rust
# hex = "0.4"            # Only if needed for non-keepkey operations
```

## 🔧 **Usage Patterns**

### **Device Discovery**
```rust
use keepkey_rust::features::list_connected_devices;

#[tauri::command]
pub async fn list_connected_devices() -> Result<Vec<FriendlyUsbDevice>, String> {
    // ✅ CORRECT: Use high-level API
    let devices = keepkey_rust::features::list_connected_devices();
    Ok(devices)
}
```

### **Device Feature Retrieval**
```rust
use keepkey_rust::features::get_device_features_by_id;

#[tauri::command]
pub async fn get_device_info(device_id: String) -> Result<DeviceFeatures, String> {
    // ✅ CORRECT: Use high-level API with error translation
    keepkey_rust::features::get_device_features_by_id(&device_id)
        .map_err(|e| format!("Failed to get device features: {}", e))
}
```

### **Async Device Operations**
```rust
use keepkey_rust::device_queue::{DeviceQueueFactory, DeviceQueueHandle};

type DeviceQueueManager = Arc<tokio::sync::Mutex<HashMap<String, DeviceQueueHandle>>>;

#[tauri::command]
pub async fn get_device_address(
    device_id: String,
    path: Vec<u32>,
    coin_name: String,
    queue_manager: State<'_, DeviceQueueManager>,
) -> Result<String, String> {
    // Get or create device queue handle
    let queue_handle = get_or_create_queue(&device_id, &queue_manager).await?;
    
    // ✅ CORRECT: Use device queue for async operations
    queue_handle
        .get_address(path, coin_name, None)
        .await
        .map_err(|e| format!("Failed to get device address: {}", e))
}
```

## 🚫 **Anti-Patterns to Avoid**

### ❌ **Direct USB Operations**
```rust
// ❌ WRONG: Never do this in vault-v2
use rusb::{Device, GlobalContext};

fn bad_device_enumeration() {
    let devices = rusb::devices().unwrap();  // Violates abstraction
    for device in devices.iter() {
        let desc = device.device_descriptor().unwrap();  // Wrong layer
    }
}
```

### ❌ **Manual Device Conversion**
```rust
// ❌ WRONG: Don't manually convert USB devices
fn device_to_friendly(device: &rusb::Device<GlobalContext>) -> FriendlyUsbDevice {
    // This function should NEVER exist in vault-v2
    // It's handled internally by keepkey-rust
}
```

### ❌ **Low-Level Transport Creation**
```rust
// ❌ WRONG: Don't create transports directly
let transport = UsbTransport::new(&device, 0)?;  // Wrong abstraction level
```

## 🗂️ **Project Structure**

```
vault-v2/
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── lib.rs          # Main library entry point
│   │   ├── commands.rs     # Tauri commands (cleaned)
│   │   ├── event_controller.rs  # Device event handling (cleaned)
│   │   ├── db.rs           # Database operations
│   │   └── main.rs         # Application entry point
│   ├── Cargo.toml          # Clean dependencies (no rusb/hidapi)
│   └── tauri.conf.json     # Tauri configuration
├── src/                    # Frontend (React/TypeScript)
├── package.json            # Node.js dependencies
└── README.md               # This file
```

## 🔄 **Event System**

The application uses a clean event-driven architecture:

```rust
// event_controller.rs - cleaned implementation
use keepkey_rust::features::{list_connected_devices, get_device_features_with_fallback};

pub fn spawn_event_controller(app: &AppHandle) {
    tokio::spawn(async move {
        let mut last_devices = Vec::new();
        
        loop {
            // ✅ CORRECT: Use high-level API
            let current_devices = list_connected_devices();
            
            // Check for device changes and emit events
            for device in &current_devices {
                if !last_devices.iter().any(|d| d.unique_id == device.unique_id) {
                    // ✅ CORRECT: Use high-level feature detection
                    if let Ok(features) = get_device_features_with_fallback(device) {
                        app.emit("device:connected", &device);
                        app.emit("device:features-updated", &features);
                    }
                }
            }
            
            last_devices = current_devices;
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    });
}
```

## 🔧 **Device Queue Management**

For async operations, the application maintains device queues:

```rust
// Proper device queue management
type DeviceQueueManager = Arc<tokio::sync::Mutex<HashMap<String, DeviceQueueHandle>>>;

async fn get_or_create_queue(
    device_id: &str,
    manager: &DeviceQueueManager,
) -> Result<DeviceQueueHandle, String> {
    let mut manager_lock = manager.lock().await;
    
    if let Some(handle) = manager_lock.get(device_id) {
        Ok(handle.clone())
    } else {
        // ✅ CORRECT: Find device using high-level API
        let devices = keepkey_rust::features::list_connected_devices();
        let device_info = devices
            .iter()
            .find(|d| d.unique_id == device_id)
            .ok_or_else(|| format!("Device {} not found", device_id))?;
        
        // ✅ CORRECT: Create queue using factory pattern
        let handle = DeviceQueueFactory::spawn_worker(
            device_id.to_string(),
            device_info.clone()
        );
        
        manager_lock.insert(device_id.to_string(), handle.clone());
        Ok(handle)
    }
}
```

## 🧪 **Testing**

### **Unit Tests**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_device_listing() {
        // Test high-level API integration
        let devices = keepkey_rust::features::list_connected_devices();
        // Assertions on FriendlyUsbDevice objects
    }
}
```

### **Integration Tests**
```bash
# Test with real hardware
cargo test --features integration-tests

# Test without hardware (mocked)
cargo test --lib
```

## 🚀 **Getting Started**

### **Prerequisites**
- Rust 1.70+ with Cargo
- Node.js 18+ with npm/yarn
- KeepKey device (for hardware testing)

### **Development Setup**
```bash
# Clone the repository
git clone <repository-url>
cd keepkey-bitcoin-only/projects/vault-v2

# Install frontend dependencies
npm install

# Run in development mode
npm run tauri dev
```

### **Building**
```bash
# Build for production
npm run tauri build
```

## 🔍 **Debugging**

### **Device Communication Issues**
1. Check device connection: Use high-level `list_connected_devices()`
2. Verify permissions: USB/HID access issues are handled by keepkey-rust
3. Check logs: Application logs show high-level operations, keepkey-rust logs show transport details

### **Common Issues**
- **"Device not found"**: Device disconnected or permissions issue
- **"Transport failed"**: USB/HID communication problem (handled by keepkey-rust)
- **"Queue timeout"**: Device operation took too long (check device state)

## 📋 **Architecture Checklist**

Before adding new features, verify:

- [ ] **No Low-Level Imports**: Don't import `rusb`, `hidapi`, etc.
- [ ] **High-Level APIs Only**: Use keepkey-rust exported functions
- [ ] **Proper Error Handling**: Translate errors for user display
- [ ] **Async Patterns**: Use device queues for non-blocking operations
- [ ] **Clean Dependencies**: Don't add hardware libraries to Cargo.toml

## 🔗 **References**

- [KeepKey-Rust Library](../keepkey-rust/README.md)
- [Architecture Documentation](../../docs/architecture/keepkey-rust-integration.md)
- [Tauri Documentation](https://tauri.app/v1/guides/)

## 📝 **License**

This project is licensed under [LICENSE] - see the LICENSE file for details.
