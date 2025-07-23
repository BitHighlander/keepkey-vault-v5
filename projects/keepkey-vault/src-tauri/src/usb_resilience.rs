use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbResilienceConfig {
    /// Grace period in milliseconds before showing disconnection
    pub disconnection_grace_period_ms: u64,
    
    /// Maximum time in milliseconds to wait for reconnection
    pub max_reconnection_wait_ms: u64,
    
    /// Operation retry configuration
    pub max_retry_attempts: u32,
    pub initial_retry_delay_ms: u64,
    pub max_retry_delay_ms: u64,
    
    /// UI feedback thresholds in milliseconds
    pub show_banner_after_ms: u64,
    pub show_dialog_after_ms: u64,
}

impl Default for UsbResilienceConfig {
    fn default() -> Self {
        Self {
            disconnection_grace_period_ms: 5000,
            max_reconnection_wait_ms: 30000,
            max_retry_attempts: 5,
            initial_retry_delay_ms: 100,
            max_retry_delay_ms: 5000,
            show_banner_after_ms: 5000,
            show_dialog_after_ms: 30000,
        }
    }
}

/// Check if an error is transient and should be retried
pub fn is_transient_error(error: &str) -> bool {
    error.contains("Device operation timed out") ||
    error.contains("Device not found") ||
    error.contains("Communication Timeout") ||
    error.contains("No data received") ||
    error.contains("USB error") ||
    error.contains("temporarily unavailable")
}

/// Calculate retry delay with exponential backoff
pub fn calculate_retry_delay(attempt: u32, config: &UsbResilienceConfig) -> std::time::Duration {
    let delay_ms = (config.initial_retry_delay_ms * 2u64.pow(attempt))
        .min(config.max_retry_delay_ms);
    std::time::Duration::from_millis(delay_ms)
}
