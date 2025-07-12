# KeepKey Rust Library

A comprehensive Rust library for communicating with KeepKey hardware wallets. This crate provides high-level APIs for device communication, feature detection, and transaction operations while handling all low-level USB/HID transport details internally.

## 🏗️ **Architecture Overview**

This library follows a clean architectural pattern with clear abstraction boundaries:

```
┌─────────────────────────────────────┐
│     Application Layer               │
│   (vault-v2, CLI tools, etc.)      │
└─────────────────────────────────────┘
                  │
                  ▼ High-level API calls only
┌─────────────────────────────────────┐
│         KeepKey-Rust Crate          │
│  • Device discovery & management    │
│  • Feature detection & caching      │
│  • Transport abstraction (USB/HID)  │
│  • Message protocol handling        │
└─────────────────────────────────────┘
                  │
                  ▼ Internal implementation only
┌─────────────────────────────────────┐
│      Low-level Dependencies        │
│    (rusb, hidapi, protobuf)        │
└─────────────────────────────────────┘
```

## ✅ **Proper Usage Pattern**

Applications should **ONLY** use the high-level APIs exported by this crate:

```rust
use keepkey_rust::{
    features::{list_connected_devices, get_device_features_by_id},
    device_queue::{DeviceQueueFactory, DeviceQueueHandle},
    friendly_usb::FriendlyUsbDevice,
};

// ✅ CORRECT: Use high-level APIs
let devices = keepkey_rust::features::list_connected_devices();
for device in devices {
    match keepkey_rust::features::get_device_features_by_id(&device.unique_id) {
        Ok(features) => println!("Device: {} v{}", device.name, features.version),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## ❌ **Anti-Pattern: Direct Low-Level Usage**

Applications should **NEVER** directly import or use low-level dependencies:

```rust
// ❌ WRONG: Don't import low-level USB libraries
use rusb::{Device, GlobalContext};

// ❌ WRONG: Don't do direct USB operations
let devices = rusb::devices().unwrap();
for device in devices.iter() {
    let desc = device.device_descriptor().unwrap(); // This violates abstraction
}
```

## 📚 **High-Level API Reference**

### Device Discovery

```rust
use keepkey_rust::features::{list_connected_devices, get_device_features_by_id};

// List all connected KeepKey devices
let devices = list_connected_devices();

// Get features for a specific device
let features = get_device_features_by_id("bus1_addr4")?;
```

### Device Queue Operations

```rust
use keepkey_rust::device_queue::{DeviceQueueFactory, DeviceQueueHandle};

// Create a device worker for async operations
let queue_handle = DeviceQueueFactory::spawn_worker(
    device_id.clone(), 
    device_info.clone()
);

// Get device features asynchronously
let features = queue_handle.get_features().await?;

// Get address for a derivation path
let address = queue_handle.get_address(
    vec![44, 0, 0, 0, 0], // BIP44 path
    "Bitcoin".to_string(),
    None
).await?;
```

### Device Types

```rust
use keepkey_rust::friendly_usb::FriendlyUsbDevice;
use keepkey_rust::features::DeviceFeatures;

// FriendlyUsbDevice provides user-friendly device information
struct FriendlyUsbDevice {
    pub unique_id: String,
    pub name: String,
    pub vid: u16,
    pub pid: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    pub is_keepkey: bool,
}

// DeviceFeatures contains all device capabilities and status
struct DeviceFeatures {
    pub label: Option<String>,
    pub version: String,
    pub initialized: bool,
    pub pin_protection: bool,
    // ... many more fields
}
```

## 🔧 **Integration Guidelines**

### For Application Developers

1. **Add to Cargo.toml**: Only include keepkey-rust, never low-level USB crates
   ```toml
   [dependencies]
   keepkey_rust = { path = "../keepkey-rust" }
   # ❌ DON'T add: rusb = "0.9"
   # ❌ DON'T add: hidapi = "1.4"
   ```

2. **Use High-Level APIs**: Always use the exported functions from keepkey_rust modules

3. **Handle Errors Gracefully**: The library provides detailed error messages for debugging

4. **Async Support**: Use the device queue system for non-blocking operations

### For Library Maintainers

1. **Maintain Abstraction**: All USB/HID details should remain internal to this crate

2. **Export Clean APIs**: Only export functions that applications actually need

3. **Handle Transport Fallback**: USB→HID fallback should be automatic and transparent

4. **Provide Good Error Messages**: Help developers understand what went wrong

## 🚀 **Features**

- **Device Discovery**: Automatic detection of all connected KeepKey devices
- **Transport Abstraction**: Automatic USB/HID fallback for maximum compatibility
- **Async Operations**: Non-blocking device communication with queuing
- **Feature Detection**: Comprehensive device capability reporting
- **Error Handling**: Detailed error messages for debugging
- **Cross-Platform**: Works on Windows, macOS, and Linux

## 🔗 **Transport Layer**

The library automatically handles:
- **USB Transport**: Primary communication method for modern systems
- **HID Fallback**: Automatic fallback for permission issues or older devices
- **Device State Detection**: Bootloader vs wallet mode detection
- **Connection Recovery**: Automatic reconnection on temporary disconnects

## 📝 **Examples**

### Basic Device Listing
```rust
use keepkey_rust::features::list_connected_devices;

fn main() -> anyhow::Result<()> {
    let devices = list_connected_devices();
    
    if devices.is_empty() {
        println!("No KeepKey devices found");
        return Ok(());
    }
    
    for device in devices {
        println!("Found: {} ({})", device.name, device.unique_id);
    }
    
    Ok(())
}
```

### Async Device Communication
```rust
use keepkey_rust::{
    features::list_connected_devices,
    device_queue::DeviceQueueFactory,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let devices = list_connected_devices();
    let device = devices.first().unwrap();
    
    let queue = DeviceQueueFactory::spawn_worker(
        device.unique_id.clone(),
        device.clone()
    );
    
    let features = queue.get_features().await?;
    println!("Device version: {}", features.version);
    
    let address = queue.get_address(
        vec![44, 0, 0, 0, 0],
        "Bitcoin".to_string(),
        None
    ).await?;
    println!("Bitcoin address: {}", address);
    
    Ok(())
}
```

## 🛡️ **Error Handling**

The library provides comprehensive error handling:

```rust
use keepkey_rust::features::get_device_features_by_id;

match get_device_features_by_id("invalid_device_id") {
    Ok(features) => println!("Success: {}", features.version),
    Err(e) => {
        if e.to_string().contains("not found") {
            println!("Device not connected");
        } else if e.to_string().contains("transport") {
            println!("Communication error - check permissions");
        } else {
            println!("Unknown error: {}", e);
        }
    }
}
```

## 📋 **Cargo.toml Dependencies**

The library manages all low-level dependencies internally:

```toml
[dependencies]
# USB communication
rusb = "0.9"
hidapi = "1.4"

# Protocol handling  
prost = "0.11"
hex = "0.4"

# Async support
tokio = { version = "1", features = ["full"] }

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

Applications consuming this library should **never** need to add these dependencies directly.

## 🧪 **Testing**

```bash
# Test with a connected KeepKey device
cargo test --features integration-tests

# Test only the API without hardware
cargo test --lib
```

## 📜 **License**

This project is licensed under [LICENSE] - see the LICENSE file for details.

## 🤝 **Contributing**

1. Maintain the high-level API abstraction
2. Add tests for new functionality
3. Update documentation for API changes
4. Ensure cross-platform compatibility

## 🔗 **Related Projects**

- [vault-v2](../vault-v2/) - Tauri-based GUI application using this library
- [KeepKey Firmware](https://github.com/keepkey/keepkey-firmware) - Device firmware 