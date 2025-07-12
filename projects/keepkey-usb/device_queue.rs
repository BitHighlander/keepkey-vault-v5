use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{timeout, sleep};
use anyhow::{anyhow, Result};
use tracing::{info, warn, error, debug, instrument};

use crate::messages::{Message, GetFeatures, GetAddress, Features};
use crate::transport::ProtocolAdapter;
use crate::friendly_usb::FriendlyUsbDevice;

/// Transport type detection for different KeepKey device modes
#[derive(Debug, Clone, Copy)]
enum TransportType {
    /// Modern WebUSB devices (firmware 7.10.0+) with bulk endpoints
    WebUsb,
    /// Traditional USB devices with interrupt endpoints and HID-style protocol
    TraditionalUsb,
    /// Legacy devices or fallback mode that only work with HID API
    HidOnly,
}

// Default timeouts and limits
const DEVICE_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);
const QUEUE_CHANNEL_SIZE: usize = 100;
const CACHE_MAX_ENTRIES: usize = 256;
const CACHE_TTL: Duration = Duration::from_secs(30);

/// Unique key for caching device responses
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    device_id: String,
    operation: String,
    params_hash: u64,
}

impl CacheKey {
    fn new(device_id: String, operation: impl Into<String>, params: &[u8]) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        params.hash(&mut hasher);
        
        Self {
            device_id,
            operation: operation.into(),
            params_hash: hasher.finish(),
        }
    }
}

/// Cached response with timestamp
#[derive(Debug, Clone)]
pub struct CachedResponse {
    value: serde_json::Value,
    timestamp: Instant,
}

impl CachedResponse {
    fn new(value: serde_json::Value) -> Self {
        Self {
            value,
            timestamp: Instant::now(),
        }
    }
    
    fn is_fresh(&self) -> bool {
        self.timestamp.elapsed() < CACHE_TTL
    }
}

/// Commands that can be sent to the device worker
#[derive(Debug)]
pub enum DeviceCmd {
    GetFeatures {
        respond_to: oneshot::Sender<Result<Features>>,
        enqueued_at: Instant,
    },
    GetAddress {
        path: Vec<u32>,
        coin_name: String,
        script_type: Option<i32>,
        show_display: Option<bool>,
        respond_to: oneshot::Sender<Result<String>>,
        enqueued_at: Instant,
    },
    SendRaw {
        message: Message,
        respond_to: oneshot::Sender<Result<Message>>,
        enqueued_at: Instant,
        bypass_cache: bool,
    },
    UpdateBootloader {
        target_version: String,
        bootloader_bytes: Vec<u8>,
        respond_to: oneshot::Sender<Result<bool>>,
        enqueued_at: Instant,
    },
    UpdateFirmware {
        target_version: String,
        firmware_bytes: Vec<u8>,
        respond_to: oneshot::Sender<Result<bool>>,
        enqueued_at: Instant,
    },
    Shutdown {
        respond_to: oneshot::Sender<Result<()>>,
    },
}

impl DeviceCmd {
    fn enqueued_at(&self) -> Instant {
        match self {
            DeviceCmd::GetFeatures { enqueued_at, .. } => *enqueued_at,
            DeviceCmd::GetAddress { enqueued_at, .. } => *enqueued_at,
            DeviceCmd::SendRaw { enqueued_at, .. } => *enqueued_at,
            DeviceCmd::UpdateBootloader { enqueued_at, .. } => *enqueued_at,
            DeviceCmd::UpdateFirmware { enqueued_at, .. } => *enqueued_at,
            DeviceCmd::Shutdown { .. } => Instant::now(),
        }
    }
    
    fn operation_name(&self) -> &'static str {
        match self {
            DeviceCmd::GetFeatures { .. } => "get_features",
            DeviceCmd::GetAddress { .. } => "get_address", 
            DeviceCmd::SendRaw { .. } => "send_raw",
            DeviceCmd::UpdateBootloader { .. } => "update_bootloader",
            DeviceCmd::UpdateFirmware { .. } => "update_firmware",
            DeviceCmd::Shutdown { .. } => "shutdown",
        }
    }
    
    fn should_cache(&self) -> bool {
        match self {
            DeviceCmd::GetFeatures { .. } => true,
            DeviceCmd::GetAddress { .. } => true,
            DeviceCmd::SendRaw { bypass_cache, .. } => !*bypass_cache,
            DeviceCmd::UpdateBootloader { .. } => false,
            DeviceCmd::UpdateFirmware { .. } => false,
            DeviceCmd::Shutdown { .. } => false,
        }
    }
}

/// Metrics for monitoring queue performance
#[derive(Debug, Default, Clone)]
pub struct DeviceQueueMetrics {
    pub queue_wait_ms: Vec<u64>,
    pub device_rtt_ms: Vec<u64>,
    pub total_ms: Vec<u64>,
    pub queue_depth: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

impl DeviceQueueMetrics {
    pub fn cache_hit_ratio(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }
    
    pub fn record_cache_hit(&mut self) {
        self.cache_hits += 1;
    }
    
    pub fn record_cache_miss(&mut self) {
        self.cache_misses += 1;
    }
    
    pub fn record_operation(&mut self, queue_wait: Duration, device_rtt: Duration, total: Duration) {
        self.queue_wait_ms.push(queue_wait.as_millis() as u64);
        self.device_rtt_ms.push(device_rtt.as_millis() as u64);
        self.total_ms.push(total.as_millis() as u64);
        
        // Keep only last 100 measurements
        if self.queue_wait_ms.len() > 100 {
            self.queue_wait_ms.remove(0);
            self.device_rtt_ms.remove(0);
            self.total_ms.remove(0);
        }
    }
}

/// Worker task that processes device commands sequentially
pub struct DeviceWorker {
    device_id: String,
    device_info: FriendlyUsbDevice,
    transport: Option<Box<dyn ProtocolAdapter + Send>>,
    cache: HashMap<CacheKey, CachedResponse>,
    metrics: DeviceQueueMetrics,
    cmd_rx: mpsc::Receiver<DeviceCmd>,
    /// Track if device is in PIN flow mode (ResetDevice, PIN setup, etc)
    is_pin_flow: bool,
}

impl DeviceWorker {
    fn new(
        device_id: String,
        device_info: FriendlyUsbDevice,
        cmd_rx: mpsc::Receiver<DeviceCmd>,
    ) -> Self {
        Self {
            device_id,
            device_info,
            transport: None,
            cache: HashMap::new(),
            metrics: DeviceQueueMetrics::default(),
            cmd_rx,
            is_pin_flow: false,
        }
    }
    
    /// Main worker loop - processes commands sequentially
    #[instrument(level = "info", skip(self))]
    pub async fn run(mut self) {
        info!("🚀 DeviceWorker starting for device {}", self.device_id);
        
        while let Some(cmd) = self.cmd_rx.recv().await {
            let start_time = Instant::now();
            let queue_wait = start_time.duration_since(cmd.enqueued_at());
            
            // Update queue depth metric
            self.metrics.queue_depth = self.cmd_rx.len();
            
            debug!("📝 Processing {} command (queue wait: {:?})", cmd.operation_name(), queue_wait);
            
            let result = self.process_command(cmd).await;
            
            if let Err(ref e) = result {
                error!("❌ Command failed: {}", e);
            }
        }
        
        info!("🛑 DeviceWorker shutting down for device {}", self.device_id);
    }
    
    /// Process a single command
    async fn process_command(&mut self, cmd: DeviceCmd) -> Result<()> {
        let device_start = Instant::now();
        let enqueued_at = cmd.enqueued_at();
        
        match cmd {
            DeviceCmd::GetFeatures { respond_to, .. } => {
                let result = self.handle_get_features().await;
                let _ = respond_to.send(result);
            }
            DeviceCmd::GetAddress { path, coin_name, script_type, show_display, respond_to, .. } => {
                let result = self.handle_get_address(path, coin_name, script_type, show_display).await;
                let _ = respond_to.send(result);
            }
            DeviceCmd::SendRaw { message, respond_to, bypass_cache, .. } => {
                let result = self.handle_send_raw(message, bypass_cache).await;
                let _ = respond_to.send(result);
            }
            DeviceCmd::UpdateBootloader { target_version, bootloader_bytes, respond_to, enqueued_at: _ } => {
                let result = self.handle_update_bootloader(target_version, bootloader_bytes).await;
                let _ = respond_to.send(result);
            }
            DeviceCmd::UpdateFirmware { target_version, firmware_bytes, respond_to, enqueued_at: _ } => {
                let result = self.handle_update_firmware(target_version, firmware_bytes).await;
                let _ = respond_to.send(result);
            }
            DeviceCmd::Shutdown { respond_to } => {
                let _ = respond_to.send(Ok(()));
                return Ok(());
            }
        }
        
        let device_rtt = device_start.elapsed();
        let total_time = enqueued_at.elapsed();
        let queue_wait = device_start.duration_since(enqueued_at);
        
        self.metrics.record_operation(queue_wait, device_rtt, total_time);
    
    // Always drop transport after each command to avoid exclusive handle issues,
    // it will be recreated lazily on the next command.
    if self.transport.is_some() {
        info!("🔌 Releasing transport handle for device {} after operation", self.device_id);
    }
    self.transport = None;
    
    Ok(())
    }
    

    
    /// Ensure transport is available, creating if necessary
    async fn ensure_transport(&mut self) -> Result<&mut (dyn ProtocolAdapter + Send)> {
        loop {
            if self.transport.is_none() {
                info!("🔗 Attempting to create transport for device {}", self.device_id);
                match DeviceQueueFactory::create_transport_for_device(&self.device_info) {
                    Ok(transport) => {
                        self.transport = Some(transport);
                        info!("✅ Transport ready for {}", self.device_id);
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        
                        // Check if this looks like a device power cycle issue
                        if error_msg.contains("timeout") || error_msg.contains("Communication Timeout") || 
                           error_msg.contains("No data received") {
                            warn!("🔄 Device {} appears to need power cycling - {}", self.device_id, e);
                        } else {
                            warn!("⚠️  Transport unavailable for {}: {} – waiting for reconnect", self.device_id, e);
                        }
                        
                        // Drop any stale transport reference just in case
                        self.transport = None;
                        // Wait a bit before retrying.  This keeps the queue worker alive
                        // and effectively makes the queue "just wait" for the device to return.
                        sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                }
            }

            // By this point transport is Some
            return Ok(self.transport.as_mut().unwrap().as_mut());
        }
    }
    
    /// Handle GetFeatures command with caching
    async fn handle_get_features(&mut self) -> Result<Features> {
        // NOTE: We purposely skip normal caching for GetFeatures because features are
        // lightweight and the user generally expects fresh information about the
        // device. We still record a miss so that cache-hit ratio maths stay sane.
        self.metrics.record_cache_miss();

        // First attempt the standard GetFeatures call.
        // For OOB bootloaders, we need to handle raw responses directly since
        // the standard handler throws an error on Failure messages
        let transport = self.ensure_transport().await?;
        let response = transport.handle(GetFeatures {}.into())?;

        match response {
            Message::Features(features) => {
                return Ok(features);
            }
            // Some very old bootloaders (so-called "OOB bootloader" devices) do not
            // recognise the GetFeatures message and will reply with a Failure/Unknown
            // message instead.  In that scenario the proven workaround is to send an
            // Initialize message which these legacy bootloaders *do* understand and
            // which returns a Features structure on success.
            Message::Failure(f) => {
                tracing::info!(
                    "GetFeatures returned Failure ({}), attempting legacy Initialize fallback for {}",
                    f.message(),
                    self.device_id
                );
                println!("🔧 GetFeatures returned Failure: {}, attempting OOB bootloader fallback with Initialize message", f.message());

                // Re-establish transport just in case previous attempt left it in an
                // undefined state.
                self.transport = None;
                let transport = self.ensure_transport().await?;

                use crate::messages::Initialize;
                let fallback_resp = transport
                    .with_standard_handler()
                    .handle(Initialize {}.into())?;

                if let Message::Features(features) = fallback_resp {
                    tracing::info!(
                        "Initialize fallback succeeded for device {} (OOB bootloader mode)",
                        self.device_id
                    );
                    println!("✅ OOB bootloader Initialize fallback successful for device {}", self.device_id);
                    return Ok(features);
                } else {
                    return Err(anyhow!("Unexpected response to Initialize fallback"));
                }
            }
            other => {
                tracing::warn!(
                    "GetFeatures returned unexpected response: {:?}, treating as failure",
                    other
                );
                println!("⚠️ GetFeatures returned unexpected response: {:?}", other);
                return Err(anyhow!("Unexpected response to GetFeatures: {:?}", other));
            }
        }
    }
    
    /// Handle GetAddress command with caching
    async fn handle_get_address(&mut self, path: Vec<u32>, coin_name: String, script_type: Option<i32>, show_display: Option<bool>) -> Result<String> {
        // Simple hash for parameters without bincode dependency
        let mut params = Vec::new();
        for &part in &path {
            params.extend_from_slice(&part.to_le_bytes());
        }
        params.extend_from_slice(coin_name.as_bytes());
        if let Some(st) = script_type {
            params.extend_from_slice(&st.to_le_bytes());
        }
        if let Some(sd) = show_display {
            params.extend_from_slice(&[sd as u8]);
        }
        
        let cache_key = CacheKey::new(self.device_id.clone(), "get_address", &params);
        
        // Check cache first
        if let Some(cached) = self.cache.get(&cache_key) {
            if cached.is_fresh() {
                self.metrics.record_cache_hit();
                debug!("💰 Cache hit for GetAddress");
                return Ok(cached.value.as_str().unwrap_or_default().to_string());
            }
        }
        
        self.metrics.record_cache_miss();
        
        // Execute on device
        let transport = self.ensure_transport().await?;
        let get_address = GetAddress {
            address_n: path,
            coin_name: Some(coin_name),
            script_type,
            show_display,
            ..Default::default()
        };
        
        let response = transport.with_pin_flow_handler().handle(get_address.into())?;
        
        match response {
            Message::Address(addr_response) => {
                let address = addr_response.address.clone(); // Use field directly not method
                
                // Cache the response
                if let Ok(json_value) = serde_json::to_value(&address) {
                    self.cache.insert(cache_key, CachedResponse::new(json_value));
                    self.cleanup_cache();
                }
                
                Ok(address)
            }
            _ => Err(anyhow!("Unexpected response to GetAddress")),
        }
    }
    
    /// Handle raw message sending 
    async fn handle_send_raw(&mut self, message: Message, bypass_cache: bool) -> Result<Message> {
        // Detect if this is a PIN flow related message
        let is_pin_flow_message = matches!(
            &message,
            Message::ResetDevice(_) | 
            Message::PinMatrixAck(_) | 
            Message::ChangePin(_) |
            Message::RecoveryDevice(_) |
            Message::GetAddress(_) |      // GetAddress can trigger PIN requests
            Message::GetPublicKey(_) |    // GetPublicKey can trigger PIN requests  
            Message::SignTx(_)            // SignTx can trigger PIN requests
        );
        
        // Update PIN flow state based on message type
        if matches!(&message, Message::ResetDevice(_) | Message::ChangePin(_) | Message::RecoveryDevice(_)) {
            info!("🔐 Entering PIN flow mode for device {} due to {:?}", self.device_id, message.message_type());
            self.is_pin_flow = true;
        }
        
        // Store PIN flow state before mutable borrow
        let use_pin_flow_handler = self.is_pin_flow || is_pin_flow_message;
        
        // For raw messages, we generally don't cache unless specifically allowed
        let transport = self.ensure_transport().await?;
        
        // Use appropriate handler based on current state and message type
        let response = if use_pin_flow_handler {
            info!("🔐 Using PIN flow handler for message {:?}", message.message_type());
            transport.with_pin_flow_handler().handle(message)?
        } else {
            transport.with_standard_handler().handle(message)?
        };
        
        // Update PIN flow state based on response
        match &response {
            Message::Success(_) | Message::Failure(_) => {
                // PIN flow completed (either success or failure)
                if self.is_pin_flow {
                    info!("🔓 Exiting PIN flow mode for device {} after {:?}", self.device_id, response.message_type());
                    self.is_pin_flow = false;
                }
            }
            Message::EntropyRequest(_) => {
                // Device is asking for entropy, usually means PIN flow is completing
                info!("🔐 Device requesting entropy, PIN flow continuing");
            }
            _ => {}
        }
        
        // If this was a mutable operation, purge cache
        if bypass_cache || self.is_mutable_operation(&response) {
            self.cache.clear();
            info!("🧹 Cache cleared due to mutable operation");
        }
        
        Ok(response)
    }
    
    /// Handle bootloader update command
    async fn handle_update_bootloader(&mut self, target_version: String, bootloader_bytes: Vec<u8>) -> Result<bool> {
        use crate::messages::{FirmwareErase, FirmwareUpload, Message};
        use sha2::{Digest, Sha256};
        
        info!("🔄 Starting bootloader update to version {} ({} bytes)", target_version, bootloader_bytes.len());
        
        // Clear cache for this potentially disruptive operation
        self.cache.clear();
        info!("🧹 Cache cleared for bootloader update");
        
        // Get transport
        let transport = self.ensure_transport().await?;
        let mut handler = transport.with_standard_handler();
        
        // First, send FirmwareErase command for v1.0.3 bootloader compatibility
        info!("🧹 Sending FirmwareErase command for bootloader compatibility...");
        match handler.handle(FirmwareErase::default().into()) {
            Ok(Message::Success(s)) => {
                info!("✅ FirmwareErase successful: {}", s.message());
            }
            Ok(Message::Failure(f)) => {
                error!("❌ FirmwareErase failed: {}", f.message());
                return Err(anyhow!("Bootloader erase failed: {}", f.message()));
            }
            Ok(other) => {
                warn!("⚠️ Unexpected response during erase: {:?}", other);
            }
            Err(e) => {
                error!("❌ Error during FirmwareErase: {}", e);
                return Err(anyhow!("Error during bootloader erase: {}", e));
            }
        }
        
        // Now send the actual bootloader upload
        info!("📤 Sending FirmwareUpload command...");
        let payload_hash = Sha256::digest(&bootloader_bytes).to_vec();
        
        match handler.handle(FirmwareUpload {
            payload_hash,
            payload: bootloader_bytes,
        }.into()) {
            Ok(Message::Success(s)) => {
                info!("✅ Bootloader update successful: {}", s.message());
                info!("🔄 Device may reboot. Please wait a moment.");
                Ok(true)
            }
            Ok(Message::Failure(f)) => {
                error!("❌ Bootloader update failed: {}", f.message());
                Err(anyhow!("Bootloader update failed: {}", f.message()))
            }  
            Ok(other) => {
                error!("❌ Unexpected response during bootloader upload: {:?}", other);
                Err(anyhow!("Unexpected response: {:?}", other))
            }
            Err(e) => {
                error!("❌ Error during bootloader upload: {}", e);
                Err(anyhow!("Error during bootloader upload: {}. Check device screen for prompts.", e))
            }
        }
    }
    
    /// Handle firmware update command
    async fn handle_update_firmware(&mut self, target_version: String, firmware_bytes: Vec<u8>) -> Result<bool> {
        use crate::messages::{FirmwareErase, FirmwareUpload, Message};
        use sha2::{Digest, Sha256};
        
        info!("🔄 Starting firmware update to version {} ({} bytes)", target_version, firmware_bytes.len());
        
        // Clear cache for this potentially disruptive operation
        self.cache.clear();
        info!("🧹 Cache cleared for firmware update");
        
        // Get transport
        let transport = self.ensure_transport().await?;
        let mut handler = transport.with_standard_handler();
        
        // First, send FirmwareErase command to prepare device for firmware update
        info!("🧹 Sending FirmwareErase command to prepare for firmware update...");
        match handler.handle(FirmwareErase::default().into()) {
            Ok(Message::Success(s)) => {
                info!("✅ FirmwareErase successful: {}", s.message());
            }
            Ok(Message::Failure(f)) => {
                error!("❌ FirmwareErase failed: {}", f.message());
                return Err(anyhow!("Firmware erase failed: {}", f.message()));
            }
            Ok(other) => {
                warn!("⚠️ Unexpected response during erase: {:?}", other);
            }
            Err(e) => {
                error!("❌ Error during FirmwareErase: {}", e);
                return Err(anyhow!("Error during firmware erase: {}", e));
            }
        }
        
        // Now send the actual firmware upload
        info!("📤 Sending FirmwareUpload command...");
        let payload_hash = Sha256::digest(&firmware_bytes).to_vec();
        
        match handler.handle(FirmwareUpload {
            payload_hash,
            payload: firmware_bytes,
        }.into()) {
            Ok(Message::Success(s)) => {
                info!("✅ Firmware update successful: {}", s.message());
                info!("🔄 Device may reboot. Please wait a moment.");
                Ok(true)
            }
            Ok(Message::Failure(f)) => {
                error!("❌ Firmware update failed: {}", f.message());
                Err(anyhow!("Firmware update failed: {}", f.message()))
            }  
            Ok(other) => {
                error!("❌ Unexpected response during firmware upload: {:?}", other);
                Err(anyhow!("Unexpected response: {:?}", other))
            }
            Err(e) => {
                error!("❌ Error during firmware upload: {}", e);
                Err(anyhow!("Error during firmware upload: {}. Check device screen for prompts.", e))
            }
        }
    }
    
    /// Check if an operation is mutable and should invalidate cache
    fn is_mutable_operation(&self, response: &Message) -> bool {
        matches!(response,
            Message::Success(_) | // Most success responses indicate state change
            Message::TxRequest(_) | // Transaction operations
            Message::PinMatrixRequest(_) | // PIN operations
            Message::PassphraseRequest(_) // Passphrase operations
        )
    }
    
    /// Clean up old cache entries
    fn cleanup_cache(&mut self) {
        // Remove expired entries
        self.cache.retain(|_, cached| cached.is_fresh());
        
        // If still too many entries, remove oldest
        if self.cache.len() > CACHE_MAX_ENTRIES {
            let keys_to_remove: Vec<_> = {
                let mut entries: Vec<_> = self.cache.iter().collect();
                entries.sort_by_key(|(_, cached)| cached.timestamp);
                
                let to_remove = entries.len() - CACHE_MAX_ENTRIES;
                entries.iter().take(to_remove).map(|(key, _)| (*key).clone()).collect()
            };
            
            for key in keys_to_remove {
                self.cache.remove(&key);
            }
        }
    }
}

/// Handle for communicating with a device worker
#[derive(Clone, Debug)]
pub struct DeviceQueueHandle {
    device_id: String,
    cmd_tx: mpsc::Sender<DeviceCmd>,
}

impl DeviceQueueHandle {
    pub fn new(device_id: String, cmd_tx: mpsc::Sender<DeviceCmd>) -> Self {
        Self { device_id, cmd_tx }
    }
    
    /// Get device features
    #[instrument(level = "debug", skip(self))]
    pub async fn get_features(&self) -> Result<Features> {
        let (tx, rx) = oneshot::channel();
        let cmd = DeviceCmd::GetFeatures {
            respond_to: tx,
            enqueued_at: Instant::now(),
        };
        
        self.cmd_tx.send(cmd).await
            .map_err(|_| anyhow!("Device worker unavailable"))?;
            
        timeout(DEVICE_OPERATION_TIMEOUT, rx).await
            .map_err(|_| anyhow!("Device operation timed out"))?
            .map_err(|_| anyhow!("Device worker channel closed"))?
    }
    
    /// Get address for given path
    #[instrument(level = "debug", skip(self))]
    pub async fn get_address(&self, path: Vec<u32>, coin_name: String, script_type: Option<i32>, show_display: Option<bool>) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        let cmd = DeviceCmd::GetAddress {
            path,
            coin_name,
            script_type,
            show_display,
            respond_to: tx,
            enqueued_at: Instant::now(),
        };
        
        self.cmd_tx.send(cmd).await
            .map_err(|_| anyhow!("Device worker unavailable"))?;
            
        timeout(DEVICE_OPERATION_TIMEOUT, rx).await
            .map_err(|_| anyhow!("Device operation timed out"))?
            .map_err(|_| anyhow!("Device worker channel closed"))?
    }
    
    /// Send raw message to device
    #[instrument(level = "debug", skip(self, message))]
    pub async fn send_raw(&self, message: Message, bypass_cache: bool) -> Result<Message> {
        let (tx, rx) = oneshot::channel();
        let cmd = DeviceCmd::SendRaw {
            message,
            respond_to: tx,
            enqueued_at: Instant::now(),
            bypass_cache,
        };
        
        self.cmd_tx.send(cmd).await
            .map_err(|_| anyhow!("Device worker unavailable"))?;
            
        timeout(DEVICE_OPERATION_TIMEOUT, rx).await
            .map_err(|_| anyhow!("Device operation timed out"))?
            .map_err(|_| anyhow!("Device worker channel closed"))?
    }
    
    /// Update device bootloader
    #[instrument(level = "debug", skip(self, bootloader_bytes))]
    pub async fn update_bootloader(&self, target_version: String, bootloader_bytes: Vec<u8>) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        let cmd = DeviceCmd::UpdateBootloader {
            target_version,
            bootloader_bytes,
            respond_to: tx,
            enqueued_at: Instant::now(),
        };
        
        self.cmd_tx.send(cmd).await
            .map_err(|_| anyhow!("Device worker unavailable"))?;
            
        // Use longer timeout for firmware operations (2 minutes)
        timeout(Duration::from_secs(120), rx).await
            .map_err(|_| anyhow!("Bootloader update timed out"))?
            .map_err(|_| anyhow!("Device worker channel closed"))?
    }
    
    /// Update device firmware
    #[instrument(level = "debug", skip(self, firmware_bytes))]
    pub async fn update_firmware(&self, target_version: String, firmware_bytes: Vec<u8>) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        let cmd = DeviceCmd::UpdateFirmware {
            target_version,
            firmware_bytes,
            respond_to: tx,
            enqueued_at: Instant::now(),
        };
        
        self.cmd_tx.send(cmd).await
            .map_err(|_| anyhow!("Device worker unavailable"))?;
            
        // Use longer timeout for firmware operations (2 minutes)
        timeout(Duration::from_secs(120), rx).await
            .map_err(|_| anyhow!("Firmware update timed out"))?
            .map_err(|_| anyhow!("Device worker channel closed"))?
    }
    
    /// Shutdown the device worker
    pub async fn shutdown(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let cmd = DeviceCmd::Shutdown { respond_to: tx };
        
        self.cmd_tx.send(cmd).await
            .map_err(|_| anyhow!("Device worker unavailable"))?;
            
        timeout(Duration::from_secs(5), rx).await
            .map_err(|_| anyhow!("Shutdown timed out"))?
            .map_err(|_| anyhow!("Device worker channel closed"))?
    }
    
    pub fn device_id(&self) -> &str {
        &self.device_id
    }
}

/// Factory for creating device workers and handles
pub struct DeviceQueueFactory;

impl DeviceQueueFactory {
    /// Spawn a new device worker and return a handle to it
    pub fn spawn_worker(device_id: String, device_info: FriendlyUsbDevice) -> DeviceQueueHandle {
        let (cmd_tx, cmd_rx) = mpsc::channel(QUEUE_CHANNEL_SIZE);
        
        let worker = DeviceWorker::new(device_id.clone(), device_info, cmd_rx);
        
        // Spawn the worker task
        tokio::spawn(worker.run());
        
        DeviceQueueHandle::new(device_id, cmd_tx)
    }
    
    /// Create transport with WebUSB/USB/HID auto-detection
    pub fn create_transport_for_device(device_info: &FriendlyUsbDevice) -> Result<Box<dyn ProtocolAdapter + Send>> {
        // Find physical device for transport
        let devices = crate::features::list_devices();
        let physical_device = Self::find_physical_device_by_info(device_info, &devices)?;
        
        // Detect transport type based on device endpoints
        let transport_type = Self::detect_transport_type(&physical_device, device_info)?;
        
        match transport_type {
            TransportType::WebUsb => {
                info!("🌐 Detected WebUSB device, using WebUSB transport for {}", device_info.unique_id);
                info!("🔧 Attempting to create WebUSB transport...");
                match crate::transport::WebUsbTransport::new(&physical_device, 0) {
                    Ok((transport, _, _)) => {
                        info!("✅ Successfully created WebUSB transport for device {}", device_info.unique_id);
                        Ok(Box::new(transport))
                    }
                    Err(webusb_err) => {
                        error!("❌ WebUSB transport creation failed for device {}: {}", device_info.unique_id, webusb_err);
                        warn!("⚠️ WebUSB transport failed for device {}: {}, trying HID fallback", device_info.unique_id, webusb_err);
                        Self::try_hid_fallback(device_info, webusb_err.to_string())
                    }
                }
            }
            TransportType::TraditionalUsb => {
                info!("🔌 Detected traditional USB device, using interrupt transport for {}", device_info.unique_id);
                match crate::transport::UsbTransport::new(&physical_device, 0) {
                    Ok((transport, _, _)) => {
                        info!("✅ Created USB transport for device {}", device_info.unique_id);
                        Ok(Box::new(transport))
                    }
                    Err(usb_err) => {
                        warn!("⚠️ USB transport failed for device {}: {}, trying HID fallback", device_info.unique_id, usb_err);
                        Self::try_hid_fallback(device_info, usb_err.to_string())
                    }
                }
            }
            TransportType::HidOnly => {
                info!("🎛️ Device requires HID transport, using HID for {}", device_info.unique_id);
                Self::try_hid_fallback(device_info, "Device requires HID transport".to_string())
            }
        }
    }
    
    /// Detect the appropriate transport type for a device
    fn detect_transport_type(device: &rusb::Device<rusb::GlobalContext>, device_info: &FriendlyUsbDevice) -> Result<TransportType> {
        info!("🔍 Detecting transport type for device {} (VID: {:04x}, PID: {:04x})", 
              device_info.unique_id, device_info.vid, device_info.pid);
        
        // Legacy devices (PID 0x0001) must use HID
        if device_info.pid == 0x0001 {
            info!("🎛️ Legacy device (PID 0x0001) detected - using HID transport");
            return Ok(TransportType::HidOnly);
        }
        
        // Modern devices (PID 0x0002 and newer) should use USB transport, NOT HID
        // PID 0x0002 devices have interrupt endpoints and use USB transport (not WebUSB with bulk endpoints)
        if device_info.pid == 0x0002 {
            info!("🔌 Modern KeepKey device (PID 0x0002) detected - using USB transport");
            info!("   📡 Firmware 7.10.0+ devices use USB transport with interrupt endpoints");
            return Ok(TransportType::TraditionalUsb);
        }
        
        // For other newer device PIDs, inspect the endpoints
        match device.active_config_descriptor() {
            Ok(config_desc) => {
                info!("📋 Successfully read device config descriptor");
                
                // Look at the first interface (index 0) which is what we use
                if let Some(interface) = config_desc.interfaces().nth(0) {
                    info!("📋 Found interface 0");
                    if let Some(interface_desc) = interface.descriptors().next() {
                        let endpoints: Vec<_> = interface_desc.endpoint_descriptors().collect();
                        info!("📋 Found {} endpoints", endpoints.len());
                        
                        // Log all endpoints for debugging
                        for (i, ep) in endpoints.iter().enumerate() {
                            info!("   Endpoint {}: addr=0x{:02x}, type={:?}, dir={:?}, max_packet={}",
                                  i, ep.address(), ep.transfer_type(), ep.direction(), ep.max_packet_size());
                        }
                        
                        // Check if we have bulk endpoints (WebUSB) or interrupt endpoints (USB)
                        let has_bulk = endpoints.iter().any(|ep| ep.transfer_type() == rusb::TransferType::Bulk);
                        let has_interrupt = endpoints.iter().any(|ep| ep.transfer_type() == rusb::TransferType::Interrupt);
                        
                        info!("📋 Endpoint analysis: has_bulk={}, has_interrupt={}", has_bulk, has_interrupt);
                        
                        if has_bulk {
                            info!("🌐 Device {} has bulk endpoints - using WebUSB", device_info.unique_id);
                            return Ok(TransportType::WebUsb);
                        } else if has_interrupt {
                            info!("🔌 Device {} has interrupt endpoints - using USB transport", device_info.unique_id);
                            return Ok(TransportType::TraditionalUsb);
                        } else {
                            warn!("⚠️ Device {} has no recognizable endpoints - defaulting to WebUSB", device_info.unique_id);
                            return Ok(TransportType::WebUsb);
                        }
                    } else {
                        warn!("⚠️ Could not get interface descriptor for device {}", device_info.unique_id);
                    }
                } else {
                    warn!("⚠️ Could not find interface 0 for device {}", device_info.unique_id);
                }
            }
            Err(e) => {
                warn!("⚠️ Could not read device descriptors for {}: {}, defaulting to WebUSB", device_info.unique_id, e);
            }
        }
        
        // Default to WebUSB for modern devices, not HID
        info!("🌐 Defaulting to WebUSB transport for device {}", device_info.unique_id);
        Ok(TransportType::WebUsb)
    }
    
    /// Try HID transport as fallback
    fn try_hid_fallback(device_info: &FriendlyUsbDevice, previous_error: String) -> Result<Box<dyn ProtocolAdapter + Send>> {
        match crate::transport::HidTransport::new_for_device(device_info.serial_number.as_deref()) {
            Ok(hid_transport) => {
                info!("✅ Created HID transport for device {}", device_info.unique_id);
                Ok(Box::new(hid_transport))
            }
            Err(hid_err) => {
                Err(anyhow!("Failed with both primary transport ({}) and HID fallback ({})", previous_error, hid_err))
            }
        }
    }
    
    /// Find the physical device matching device info (static method)
    fn find_physical_device_by_info(device_info: &FriendlyUsbDevice, devices: &[rusb::Device<rusb::GlobalContext>]) -> Result<rusb::Device<rusb::GlobalContext>> {
        if let Some(serial) = &device_info.serial_number {
            // Match by serial number
            for device in devices {
                if let Ok(handle) = device.open() {
                    let timeout = std::time::Duration::from_millis(100);
                    if let Ok(langs) = handle.read_languages(timeout) {
                        if let Some(lang) = langs.first() {
                            if let Ok(desc) = device.device_descriptor() {
                                if let Ok(device_serial) = handle.read_serial_number_string(*lang, &desc, timeout) {
                                    if device_serial == *serial {
                                        return Ok(device.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Try to parse bus and address from unique_id
        let parts: Vec<&str> = device_info.unique_id.split('_').collect();
        if parts.len() >= 2 {
            let bus_str = parts[0].strip_prefix("bus").unwrap_or("");
            let addr_str = parts[1].strip_prefix("addr").unwrap_or("");
            
            if let (Ok(bus), Ok(addr)) = (bus_str.parse::<u8>(), addr_str.parse::<u8>()) {
                for device in devices {
                    if device.bus_number() == bus && device.address() == addr {
                        return Ok(device.clone());
                    }
                }
            }
        }
        
        Err(anyhow!("Physical device not found for {}", device_info.unique_id))
    }
} 