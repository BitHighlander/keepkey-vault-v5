use keepkey_rust::features::list_connected_devices;

fn main() {
    println!("Testing device enumeration...");
    
    for i in 1..=5 {
        println!("\nScan #{}: ", i);
        let devices = list_connected_devices();
        
        for device in devices {
            println!("  Device: {} ({})", device.name, device.unique_id);
            println!("    VID:PID: {:04x}:{:04x}", device.vid, device.pid);
            println!("    Manufacturer: {:?}", device.manufacturer);
            println!("    Product: {:?}", device.product);
            println!("    Serial: {:?}", device.serial_number);
            println!("    Is KeepKey: {}", device.is_keepkey);
        }
        
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
} 