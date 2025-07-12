use anyhow::{anyhow, Result};
use hidapi::{HidApi, HidDevice};
use std::time::Duration;
use thiserror::Error;
use log::{debug, info, warn, error};

use super::Transport;

const KEEPKEY_VID: u16 = 0x2B24;
const KEEPKEY_PIDS: &[u16] = &[0x0001, 0x0002]; // Legacy and bootloader PIDs
const HID_REPORT_SIZE: usize = 64;
const REPORT_ID: u8 = 0;

#[derive(Debug, Error)]
pub enum HidError {
    #[error("HID API error: {0}")]
    HidApi(#[from] hidapi::HidError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Other error: {0}")]
    Other(String),
}

pub struct HidTransport {
    device: HidDevice,
}

impl HidTransport {
    /// Handle specific device open errors with helpful messages
    fn handle_device_open_error(error: &hidapi::HidError, serial: &str) -> Result<()> {
        let error_msg = error.to_string().to_lowercase();
        
        if error_msg.contains("access") || error_msg.contains("permission") || 
           error_msg.contains("in use") || error_msg.contains("busy") ||
           error_msg.contains("claimed") || error_msg.contains("cannot open") {
            
            error!("❌ Device already claimed: KeepKey device with serial {} is being used by another application", serial);
            
            return Err(anyhow!(
                "🔒 KeepKey Device Already In Use\n\n\
                The KeepKey device (serial: {}) is currently being used by another application.\n\n\
                Common causes:\n\
                • KeepKey Desktop app is running\n\
                • KeepKey Bridge is running\n\
                • Another wallet application is connected\n\
                • Previous connection wasn't properly closed\n\n\
                Solutions:\n\
                1. Close KeepKey Desktop app completely\n\
                2. Close any other wallet applications\n\
                3. Unplug and reconnect your KeepKey device\n\
                4. Try again\n\n\
                Technical details: {}", 
                serial, error
            ));
        }
        
        // For other errors, just log and continue trying other devices
        error!("Failed to open device with serial {}: {}", serial, error);
        Ok(())
    }
    
    /// Create a new HID transport for a specific device
    /// 
    /// This function has been improved to handle device reconnection better,
    /// especially on Windows where serial numbers might be temporarily unavailable
    /// after a device reconnects.
    pub fn new_for_device(serial_number: Option<&str>) -> Result<Self> {
        let api = HidApi::new().map_err(|e| anyhow!("Failed to initialize HID API: {}", e))?;
        
        // Log all available HID devices for debugging
        debug!("Available HID devices:");
        let mut keepkey_devices = Vec::new();
        
        for device_info in api.device_list() {
            let is_keepkey = device_info.vendor_id() == KEEPKEY_VID && 
                             KEEPKEY_PIDS.contains(&device_info.product_id());
            
            debug!("  VID: {:04x}, PID: {:04x}, Serial: {:?}, Path: {}, KeepKey: {}",
                device_info.vendor_id(),
                device_info.product_id(),
                device_info.serial_number(),
                device_info.path().to_string_lossy(),
                is_keepkey
            );
            
            if is_keepkey {
                keepkey_devices.push(device_info);
            }
        }
        
        info!("Found {} KeepKey devices", keepkey_devices.len());
        
        if keepkey_devices.is_empty() {
            return Err(anyhow!("No KeepKey devices found"));
        }
        
        // Find the KeepKey device
        let device = if let Some(serial) = serial_number {
            info!("Attempting to find KeepKey device with serial number: {}", serial);
            
            // First try exact serial match
            let exact_match = keepkey_devices.iter()
                .find(|info| info.serial_number() == Some(serial));
                    
            if let Some(info) = exact_match {
                info!("Found KeepKey device with exact serial match: {}", serial);
                match info.open_device(&api) {
                    Ok(device) => Some(device),
                    Err(e) => {
                        // Log detailed reason, but do NOT abort creation – continue trying
                        // other devices or fallback strategies. This matches Vault v1 behavior
                        // and avoids terminating early on "already claimed" / exclusive-access
                        // errors that can be transient or OS-specific.
                        let _ = Self::handle_device_open_error(&e, serial);
                        None
                    }
                }
            } else {
                // If no exact match, provide helpful error about available devices
                let available_serials: Vec<String> = keepkey_devices.iter()
                    .filter_map(|info| info.serial_number())
                    .map(|s| s.to_string())
                    .collect();
                
                return Err(anyhow!(
                    "🔍 KeepKey Device Not Found\n\n\
                    Could not find KeepKey device with serial: {}\n\n\
                    Available KeepKey devices:\n{}\n\n\
                    Solutions:\n\
                    1. Check that your KeepKey is connected\n\
                    2. Try reconnecting your KeepKey\n\
                    3. Use one of the available serial numbers above",
                    serial,
                    if available_serials.is_empty() {
                        "  • No KeepKey devices found".to_string()
                    } else {
                        available_serials.iter()
                            .map(|s| format!("  • {}", s))
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                ));
            }
        } else {
            info!("No serial number provided, trying first available KeepKey device");
            
            // Try each device until one opens successfully
            for info in keepkey_devices.iter() {
                info!("Trying KeepKey device with serial: {:?}", info.serial_number());
                match info.open_device(&api) {
                    Ok(device) => {
                        info!("Successfully opened KeepKey device with serial: {:?}", info.serial_number());
                        return Ok(Self { device });
                    }
                    Err(e) => {
                        debug!("Failed to open device with serial {:?}: {}", info.serial_number(), e);
                    }
                }
            }
            None
        };
        
        let device = device.ok_or_else(|| {
            if keepkey_devices.len() == 1 {
                anyhow!(
                    "🔒 KeepKey Device Access Failed\n\n\
                    Found 1 KeepKey device but couldn't open it.\n\
                    This usually means another application is using your KeepKey.\n\n\
                    Solutions:\n\
                    1. Close KeepKey Desktop app completely\n\
                    2. Close any other wallet applications\n\
                    3. Unplug and reconnect your KeepKey device\n\
                    4. Try again"
                )
            } else {
                anyhow!(
                    "🔒 KeepKey Device Access Failed\n\n\
                    Found {} KeepKey devices but couldn't open any of them.\n\
                    This usually means other applications are using your KeepKey devices.\n\n\
                    Solutions:\n\
                    1. Close KeepKey Desktop app completely\n\
                    2. Close any other wallet applications\n\
                    3. Unplug and reconnect your KeepKey devices\n\
                    4. Try again",
                    keepkey_devices.len()
                )
            }
        })?;
        
        Ok(Self { device })
    }
}

impl Transport for HidTransport {
    type Error = HidError;
    
    fn write(&mut self, msg: &[u8], _timeout: Duration) -> Result<usize, Self::Error> {
        // The incoming message already has the protocol header from Message::encode
        // Format: [#][#][msg_type(2)][length(4)][data...]
        
        // For v4 compatibility, we need to send with the old format:
        // [Report ID][0x3f][0x23][0x23][msg_type(2)][length(4)][data...]
        
        if msg.len() < 8 {
            return Err(HidError::Other("Message too short".to_string()));
        }
        
        // Extract message components (skip the ## header)
        let msg_type = &msg[2..4];
        let msg_length = &msg[4..8];
        let msg_data = &msg[8..];
        
        let msg_type_val = u16::from_be_bytes([msg_type[0], msg_type[1]]);
        let msg_length_val = u32::from_be_bytes([msg_length[0], msg_length[1], msg_length[2], msg_length[3]]);
        
        info!("HID Write: Sending message type {} (0x{:04x}), length: {}", 
              msg_type_val, msg_type_val, msg_length_val);
        
        // Log first few bytes of data for debugging
        if msg_data.len() > 0 {
            let preview_len = msg_data.len().min(32);
            let preview: Vec<String> = msg_data[..preview_len].iter()
                .map(|b| format!("{:02x}", b))
                .collect();
            debug!("HID Write: First {} data bytes: {}", preview_len, preview.join(" "));
        }
        
        // Prepare first packet with v4 format (EXACT MATCH TO WORKING VAULT V1)
        let mut first_packet = vec![0u8; HID_REPORT_SIZE];
        first_packet[0] = REPORT_ID;  // 0x00 - REQUIRED BY VAULT V1!
        first_packet[1] = 0x3f;
        first_packet[2] = 0x23;
        first_packet[3] = 0x23;
        first_packet[4..6].copy_from_slice(msg_type);
        first_packet[6..10].copy_from_slice(msg_length);
        
        // Copy as much data as fits in first packet
        let first_chunk_size = (HID_REPORT_SIZE - 10).min(msg_data.len());
        if first_chunk_size > 0 {
            first_packet[10..10 + first_chunk_size].copy_from_slice(&msg_data[..first_chunk_size]);
        }
        
        debug!("HID Write: Sending first packet (64 bytes), data chunk size: {}", first_chunk_size);
        
        // Log the actual bytes being sent for debugging
        let preview: Vec<String> = first_packet[..32].iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        info!("HID Write: First 32 bytes of packet: {}", preview.join(" "));
        
        // Log the FULL packet for detailed debugging
        let full_packet_hex: Vec<String> = first_packet.iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        info!("🔍 HID Write: FULL PACKET (64 bytes): {}", full_packet_hex.join(" "));
        
        // Send first packet
        self.device
            .write(&first_packet)
            .map_err(|e| HidError::Other(format!("HID write failed: {}", e)))?;
        
        // Send continuation packets if needed
        let mut sent = first_chunk_size;
        let mut packet_count = 1;
        while sent < msg_data.len() {
            let mut cont_packet = vec![0u8; HID_REPORT_SIZE];
            cont_packet[0] = b'?'; // Continuation packet marker
            
            let chunk_size = (HID_REPORT_SIZE - 1).min(msg_data.len() - sent);
            cont_packet[1..1 + chunk_size].copy_from_slice(&msg_data[sent..sent + chunk_size]);
            
            debug!("HID Write: Sending continuation packet {} (64 bytes), data chunk size: {}", 
                   packet_count + 1, chunk_size);
            
            self.device
                .write(&cont_packet)
                .map_err(|e| HidError::Other(format!("HID continuation write failed: {}", e)))?;
            
            sent += chunk_size;
            packet_count += 1;
        }
        
        info!("HID Write: Complete. Sent {} bytes in {} packets", msg.len(), packet_count);
        
        Ok(msg.len())
    }
    
    fn read(&mut self, buf: &mut Vec<u8>, timeout: Duration) -> Result<(), Self::Error> {
        let timeout_ms = timeout.as_millis() as i32;
        
        info!("HID Read: Waiting for response (timeout: {} ms)...", timeout_ms);
        
        // Read first packet
        let mut packet = vec![0u8; HID_REPORT_SIZE];
        let size = self.device
            .read_timeout(&mut packet, timeout_ms)
            .map_err(|e| HidError::Other(format!("HID read failed: {}", e)))?;
        
        if size == 0 {
            error!("HID Read: No data received from device after {} ms timeout", timeout_ms);
            
            // Provide helpful error message suggesting device power cycle
            let helpful_message = format!(
                "🔄 KeepKey Device Communication Timeout\n\n\
                Your KeepKey device is not responding to requests.\n\n\
                This usually indicates the device needs to be power cycled:\n\
                1. Unplug your KeepKey device from USB\n\
                2. Wait 3-5 seconds\n\
                3. Plug it back in\n\
                4. Try again\n\n\
                If the issue persists:\n\
                • Make sure no other applications are using the device\n\
                • Try a different USB port or cable\n\
                • Check if the device needs a firmware update\n\n\
                Technical details: No data received after {} ms timeout",
                timeout_ms
            );
            
            return Err(HidError::Other(helpful_message));
        }
        
        info!("HID Read: Received first packet ({} bytes)", size);
        
        // Log first few bytes for debugging
        let preview: Vec<String> = packet[..size.min(16)].iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        debug!("HID Read: First 16 bytes: {}", preview.join(" "));
        
        // v4 response format: [0x3f][0x23][0x23][msg_type(2)][length(4)][data...]
        if size < 9 || packet[0] != 0x3f || packet[1] != 0x23 || packet[2] != 0x23 {
            error!("HID Read: Invalid response header");
            return Err(HidError::Other(format!(
                "Invalid response header: {:02x} {:02x} {:02x}",
                packet[0], packet[1], packet[2]
            )));
        }
        
        // Extract message type and length from header  
        let msg_type = u16::from_be_bytes([packet[3], packet[4]]);
        let msg_length = u32::from_be_bytes([packet[5], packet[6], packet[7], packet[8]]) as usize;
        
        info!("HID Read: Message type {} (0x{:04x}), length: {} bytes", 
              msg_type, msg_type, msg_length);
        
        // Convert to v5 format for the protocol adapter
        // Start with ## header that Message::decode expects
        buf.clear();
        buf.push(b'#');
        buf.push(b'#');
        buf.extend_from_slice(&packet[3..9]); // msg_type(2) + length(4)
        
        // Copy initial data from first packet
        let first_chunk_size = (size - 9).min(msg_length);
        buf.extend_from_slice(&packet[9..9 + first_chunk_size]);
        
        let mut bytes_received = first_chunk_size;
        debug!("HID Read: First packet contained {} bytes of message data", first_chunk_size);
        
        // Read continuation packets if needed
        let mut packet_count = 1;
        while bytes_received < msg_length {
            packet.fill(0);
            
            debug!("HID Read: Reading continuation packet {} (need {} more bytes)...", 
                   packet_count + 1, msg_length - bytes_received);
                   
            let cont_size = self.device
                .read_timeout(&mut packet, 100)
                .map_err(|e| HidError::Other(format!("HID continuation read failed: {}", e)))?;
            
            if cont_size > 0 {
                // Skip the '?' prefix byte in continuation packets
                let data_start = if packet[0] == b'?' { 1 } else { 0 };
                let remaining = msg_length - bytes_received;
                let to_copy = (cont_size - data_start).min(remaining);
                
                debug!("HID Read: Continuation packet {} has {} bytes, copying {} bytes", 
                       packet_count + 1, cont_size, to_copy);
                
                buf.extend_from_slice(&packet[data_start..data_start + to_copy]);
                bytes_received += to_copy;
                packet_count += 1;
            } else if bytes_received < msg_length {
                // Still need more data but didn't get any
                error!("HID Read: Incomplete message after {} packets", packet_count);
                return Err(HidError::Other(format!(
                    "Incomplete message: expected {} bytes, got {}",
                    msg_length, bytes_received
                )));
            }
        }
        
        info!("HID Read: Complete. Received {} bytes in {} packets", buf.len() - 2, packet_count);
        
        Ok(())
    }
    
    fn reset(&mut self) -> Result<(), Self::Error> {
        info!("HID Reset: Flushing any pending data from device buffer...");
        
        // HID doesn't have a direct reset like USB
        // Flush any pending data by reading with a short timeout
        let mut dummy = vec![0u8; HID_REPORT_SIZE];
        let mut packets_flushed = 0;
        
        while let Ok(size) = self.device.read_timeout(&mut dummy, 10) {
            if size == 0 {
                break;
            }
            packets_flushed += 1;
            debug!("HID Reset: Flushed packet {} ({} bytes)", packets_flushed, size);
        }
        
        if packets_flushed > 0 {
            info!("HID Reset: Flushed {} packets from device buffer", packets_flushed);
        } else {
            info!("HID Reset: Device buffer was already empty");
        }
        
        Ok(())
    }
} 