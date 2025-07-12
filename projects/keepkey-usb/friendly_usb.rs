use serde::{Deserialize, Serialize};

/// Vendor ID for KeepKey devices
pub const KEEPKEY_VID: u16 = 0x2b24;

/// User-friendly representation of a USB device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FriendlyUsbDevice {
    pub unique_id: String,
    pub name: String,
    pub vid: u16,
    pub pid: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    pub is_keepkey: bool,
}

impl FriendlyUsbDevice {
    /// Build from basic USB details.
    pub fn new(
        unique_id: String,
        vid: u16,
        pid: u16,
        manufacturer: Option<String>,
        product: Option<String>,
        serial_number: Option<String>,
    ) -> Self {
        let name = match (&product, &manufacturer) {
            (Some(p), Some(m)) => format!("{} - {}", m, p),
            (Some(p), None) => p.clone(),
            (None, Some(m)) => m.clone(),
            (None, None) => format!("USB Device (VID: {:04x}, PID: {:04x})", vid, pid),
        };
        Self {
            unique_id,
            name,
            vid,
            pid,
            manufacturer,
            product,
            serial_number,
            is_keepkey: vid == KEEPKEY_VID,
        }
    }
}
