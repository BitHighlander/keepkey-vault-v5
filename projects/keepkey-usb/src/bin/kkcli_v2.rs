use std::time::Duration;

use comfy_table::{presets::UTF8_FULL, Table};
use rusb::UsbContext;
use rusb::{Context, Device, DeviceDescriptor};
use tokio::time::sleep;

use keepkey_rust::friendly_usb::FriendlyUsbDevice;
use keepkey_rust::features::{get_device_features_with_fallback, DeviceFeatures};

const KEEPKEY_VID: u16 = 0x2b24; // KeepKey USB vendor ID

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let ctx = Context::new()?;

    loop {
        // Collect current devices
        let keepkeys: Vec<_> = ctx
            .devices()?
            .iter()
            .filter_map(|d| match d.device_descriptor() {
                Ok(desc) if desc.vendor_id() == KEEPKEY_VID => Some((d, desc)),
                _ => None,
            })
            .collect();

        render_table(&keepkeys)?;

        sleep(Duration::from_secs(2)).await;
    }
}

fn render_table(devs: &[(Device<Context>, DeviceDescriptor)]) -> anyhow::Result<()> {
    // Clear screen ANSI
    print!("\x1B[2J\x1B[H");

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(["Bus:Address", "Serial", "Product", "FW Version", "Bootloader", "State"]);

    for (dev, desc) in devs {
        let addr = format!("{}:{}", dev.bus_number(), dev.address());
        let handle = dev.open();
        let serial = handle
            .as_ref()
            .ok()
            .and_then(|h| h.read_serial_number_string_ascii(&desc).ok())
            .unwrap_or_else(|| "<unknown>".to_string());
        let product = handle
            .as_ref()
            .ok()
            .and_then(|h| h.read_product_string_ascii(&desc).ok())
            .unwrap_or_else(|| "KeepKey".to_string());

        // Build FriendlyUsbDevice for feature fetch
        let friendly = FriendlyUsbDevice::new(
            format!("bus{}_addr{}", dev.bus_number(), dev.address()),
            desc.vendor_id(),
            desc.product_id(),
            None, // manufacturer not needed for fetch
            Some(product.clone()),
            Some(serial.clone()),
        );

        // Try to fetch features (may fail if device locked)
        let (fw_version, bl_mode, state) = match get_device_features_with_fallback(&friendly) {
            Ok(feat) => (
                feat.version,
                if feat.bootloader_mode { "Yes" } else { "No" }.to_string(),
                "Ready".to_string(),
            ),
            Err(e) => ("<n/a>".to_string(), "?".to_string(), format!("Err: {}", e)),
        };

        table.add_row([addr, serial, product, fw_version, bl_mode, state]);
    }

    println!("{}", table);
    Ok(())
}
