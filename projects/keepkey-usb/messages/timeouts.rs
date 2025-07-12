use super::Message;
use core::time::Duration;

const TIMEOUT: Duration = Duration::from_millis(5000); // Increased for better device compatibility
const LONG_TIMEOUT: Duration = Duration::from_millis(5 * 60 * 1000);
const LEGACY_DEVICE_TIMEOUT: Duration = Duration::from_millis(10000); // 10 seconds for older devices
const QUICK_TIMEOUT: Duration = Duration::from_millis(5000); // 5 seconds for device communication (was causing timeouts)

// Thread-local storage to track if we're communicating with an older device
thread_local! {
    static USING_LEGACY_DEVICE: std::cell::RefCell<bool> = std::cell::RefCell::new(false);
}

impl Message {
    /// Set whether we're communicating with a legacy device (PID 0x0001)
    /// This affects timeouts for all messages in the current thread
    pub fn set_legacy_device_mode(enabled: bool) {
        USING_LEGACY_DEVICE.with(|mode| {
            *mode.borrow_mut() = enabled;
        });
    }
    
    /// Get whether we're in legacy device mode
    pub fn is_legacy_device_mode() -> bool {
        USING_LEGACY_DEVICE.with(|mode| *mode.borrow())
    }
    
    pub fn read_timeout(&self) -> Duration {
        // For Initialize messages or when in legacy device mode, use appropriate timeout
        match self {
            Message::ButtonAck(_) => LONG_TIMEOUT,
            Message::FirmwareUpload(_) => LONG_TIMEOUT, // Bootloader updates need more time
            Message::Initialize(_) if Message::is_legacy_device_mode() => LEGACY_DEVICE_TIMEOUT,
            Message::Initialize(_) => QUICK_TIMEOUT, // Initialize is usually very fast
            Message::GetFeatures(_) => QUICK_TIMEOUT, // GetFeatures is also fast
            _ if Message::is_legacy_device_mode() => LEGACY_DEVICE_TIMEOUT,
            _ => TIMEOUT,
        }
    }

    pub fn write_timeout(&self) -> Duration {
        match self {
            Message::FirmwareUpload(_) => LONG_TIMEOUT,
            Message::Initialize(_) if Message::is_legacy_device_mode() => LEGACY_DEVICE_TIMEOUT,
            Message::Initialize(_) => QUICK_TIMEOUT,
            Message::GetFeatures(_) => QUICK_TIMEOUT,
            _ if Message::is_legacy_device_mode() => LEGACY_DEVICE_TIMEOUT,
            _ => TIMEOUT,
        }
    }
}
