use std::sync::Arc;
use anyhow::{Result, anyhow};
use keepkey_rust::device_queue::DeviceQueueHandle;
use super::{CacheManager, CacheMetadata};
use super::types::FrontloadStatus;
use crate::commands::{DeviceQueueManager, DeviceRequest, DeviceResponse};
use serde::{Deserialize, Serialize};
use serde_json;

/// Controller for frontloading device public keys and addresses
pub struct FrontloadController {
    cache: Arc<CacheManager>,
    queue_manager: DeviceQueueManager,
}

/// Derivation path from default-paths.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultPath {
    pub id: String,
    pub note: String,
    pub blockchain: String,
    pub symbol: String,
    pub networks: Vec<String>,
    pub script_type: String,
    #[serde(rename = "addressNList")]
    pub address_n_list: Vec<u32>,
    #[serde(rename = "addressNListMaster")]
    pub address_n_list_master: Vec<u32>,
    pub curve: String,
    #[serde(rename = "showDisplay")]
    pub show_display: bool,
}

/// Default paths configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultPathsConfig {
    pub version: String,
    pub description: String,
    pub paths: Vec<DefaultPath>,
}

/// Load default paths from JSON file
fn load_default_paths() -> Result<DefaultPathsConfig> {
    let json_content = include_str!("../../default-paths.json");
    let config: DefaultPathsConfig = serde_json::from_str(json_content)
        .map_err(|e| anyhow!("Failed to parse default-paths.json: {}", e))?;
    Ok(config)
}

impl FrontloadController {
    /// Create a new frontload controller
    pub fn new(cache: Arc<CacheManager>, queue_manager: DeviceQueueManager) -> Self {
        Self {
            cache,
            queue_manager,
        }
    }
    
    /// Start frontloading for a device using default paths from JSON
    pub async fn frontload_device(&self, device_id: &str) -> Result<()> {
        log::info!("🔄 Starting frontload for device: {}", device_id);
        
        // Load default paths from JSON
        let paths_config = load_default_paths()
            .map_err(|e| anyhow!("Failed to load default paths: {}", e))?;
        
        log::info!("📋 Loaded {} default paths from config", paths_config.paths.len());
        
        // Update metadata to mark as in progress
        let metadata = CacheMetadata {
            device_id: device_id.to_string(),
            label: None,
            firmware_version: None,
            initialized: true,
            frontload_status: FrontloadStatus::InProgress,
            frontload_progress: 0,
            last_frontload: None,
            error_message: None,
        };
        self.cache.update_cache_metadata(&metadata).await?;
        
        // Get device queue handle
        let queue_handle = self.get_or_create_queue_handle(device_id).await?;
        
        // Get device features first
        let features = queue_handle.get_features().await
            .map_err(|e| anyhow!("Failed to get device features: {}", e))?;
        
        // Update metadata with device info
        let mut metadata = metadata;
        metadata.label = features.label.clone();
        metadata.firmware_version = Some(format!("{}.{}.{}", 
            features.major_version.unwrap_or(0),
            features.minor_version.unwrap_or(0),
            features.patch_version.unwrap_or(0)
        ));
        metadata.initialized = features.initialized.unwrap_or(false);
        self.cache.update_cache_metadata(&metadata).await?;
        
        // Check if device needs to be cache-wiped (seed change detection)
        if !metadata.initialized {
            log::warn!("Device {} not initialized, clearing cache", device_id);
            self.cache.clear_device_cache(device_id).await?;
            return Ok(());
        }
        
        let start_time = std::time::Instant::now();
        let mut total_cached = 0;
        let mut progress = 0;
        let total_paths = paths_config.paths.len();
        let mut errors = Vec::new();
        
        // Process each path from default-paths.json
        for (i, path_config) in paths_config.paths.iter().enumerate() {
            log::debug!("🔄 Processing path {}/{}: {} ({})", 
                i + 1, total_paths, path_config.id, path_config.note);
            
            // Skip if already cached (check cache first)
            let derivation_path = self.address_n_list_to_string(&path_config.address_n_list);
            if self.is_already_cached(device_id, &derivation_path, &path_config.blockchain, &path_config.script_type).await? {
                log::debug!("⏭️ Skipping already cached path: {}", path_config.id);
                continue;
            }
            
            // Frontload both account-level xpub and individual addresses
            match self.frontload_path(&queue_handle, device_id, path_config).await {
                Ok(count) => {
                    total_cached += count;
                    log::debug!("✅ Cached {} items for path: {}", count, path_config.id);
                }
                Err(e) => {
                    log::warn!("⚠️ Failed to frontload path {}: {}", path_config.id, e);
                    errors.push(format!("{}: {}", path_config.id, e));
                }
            }
            
            // Update progress
            progress = ((i + 1) * 100) / total_paths;
            let mut progress_metadata = metadata.clone();
            progress_metadata.frontload_progress = progress as i32;
            self.cache.update_cache_metadata(&progress_metadata).await?;
        }
        
        // Update final metadata
        let final_metadata = CacheMetadata {
            device_id: device_id.to_string(),
            label: metadata.label.clone(),
            firmware_version: metadata.firmware_version.clone(),
            initialized: metadata.initialized,
            frontload_status: if errors.is_empty() { FrontloadStatus::Completed } else { FrontloadStatus::Failed },
            frontload_progress: 100,
            last_frontload: Some(chrono::Utc::now().timestamp()),
            error_message: if errors.is_empty() { None } else { Some(errors.join("; ")) },
        };
        self.cache.update_cache_metadata(&final_metadata).await?;
        
        let elapsed = start_time.elapsed();
        log::info!("✅ Frontload completed for device {}", device_id);
        log::info!("   📊 Processed {} paths, cached {} addresses/pubkeys in {:.2}s", 
            total_paths, total_cached, elapsed.as_secs_f64());
        if !errors.is_empty() {
            log::warn!("   ⚠️ {} errors occurred: {}", errors.len(), errors.join("; "));
        }
        log::info!("   💾 Data stored in SQLite cache for fast access");
        log::info!("   🏷️ Device: {}", metadata.label.as_deref().unwrap_or("Unnamed KeepKey"));
        
        Ok(())
    }
    
    /// Get or create device queue handle
    async fn get_or_create_queue_handle(&self, device_id: &str) -> Result<DeviceQueueHandle> {
        let mut manager = self.queue_manager.lock().await;
        
        if let Some(handle) = manager.get(device_id) {
            Ok(handle.clone())
        } else {
            // Find the device
            let devices = keepkey_rust::features::list_connected_devices();
            let device = devices
                .iter()
                .find(|d| d.unique_id == device_id)
                .ok_or_else(|| anyhow!("Device {} not found", device_id))?;
            
            // Create new queue handle
            let handle = keepkey_rust::device_queue::DeviceQueueFactory::spawn_worker(
                device_id.to_string(),
                device.clone()
            );
            manager.insert(device_id.to_string(), handle.clone());
            
            Ok(handle)
        }
    }
    
    /// Convert address_n_list to string format (m/44'/0'/0')
    fn address_n_list_to_string(&self, address_n_list: &[u32]) -> String {
        format!("m/{}", address_n_list.iter()
            .map(|&n| if n & 0x80000000 != 0 {
                format!("{}'", n & 0x7FFFFFFF)
            } else {
                n.to_string()
            })
            .collect::<Vec<_>>()
            .join("/"))
    }
    
    /// Check if a path is already cached
    async fn is_already_cached(
        &self, 
        device_id: &str, 
        derivation_path: &str, 
        coin_name: &str, 
        script_type: &str
    ) -> Result<bool> {
        // Check if we already have this exact path cached
        match self.cache.get_cached_pubkey(device_id, derivation_path, coin_name, Some(script_type)).await {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }
    
    /// Frontload a single path configuration
    async fn frontload_path(
        &self,
        queue_handle: &DeviceQueueHandle,
        device_id: &str,
        path_config: &DefaultPath,
    ) -> Result<usize> {
        let mut count = 0;
        
        // Convert the path to string format
        let account_path_str = self.address_n_list_to_string(&path_config.address_n_list);
        let master_path_str = self.address_n_list_to_string(&path_config.address_n_list_master);
        
        // For Bitcoin-like coins, get both XPUB (account level) and addresses (master level)
        if matches!(path_config.blockchain.as_str(), "bitcoin" | "bitcoincash" | "litecoin" | "dogecoin" | "dash") {
            // 1. Get XPUB at account level (m/44'/0'/0')
            let xpub_request = DeviceRequest::GetPublicKey {
                path: account_path_str.clone(),
                coin_name: Some(path_config.blockchain.clone()),
                script_type: Some(path_config.script_type.clone()),
                ecdsa_curve_name: Some("secp256k1".to_string()),
                show_display: Some(false),
            };
            
            match self.send_device_request(queue_handle, xpub_request).await {
                Ok(response) => {
                    if let Some(cached) = super::types::CachedPubkey::from_device_response(
                        device_id,
                        &account_path_str,
                        &path_config.blockchain,
                        Some(&path_config.script_type),
                        &response,
                    ) {
                        if let Err(e) = self.cache.save_pubkey(&cached).await {
                            log::warn!("Failed to cache XPUB for {}: {}", path_config.id, e);
                        } else {
                            count += 1;
                            log::debug!("💰 Cached XPUB for {}: {}", path_config.id, account_path_str);
                        }
                    }
                }
                Err(e) => {
                    log::debug!("Failed to get XPUB for {}: {}", path_config.id, e);
                }
            }
            
            // 2. Get address at master level (m/44'/0'/0'/0/0)
            let address_request = DeviceRequest::GetAddress {
                path: master_path_str.clone(),
                coin_name: path_config.blockchain.clone(),
                script_type: Some(path_config.script_type.clone()),
                show_display: Some(path_config.show_display),
            };
            
            match self.send_device_request(queue_handle, address_request).await {
                Ok(response) => {
                    if let Some(cached) = super::types::CachedPubkey::from_device_response(
                        device_id,
                        &master_path_str,
                        &path_config.blockchain,
                        Some(&path_config.script_type),
                        &response,
                    ) {
                        if let Err(e) = self.cache.save_pubkey(&cached).await {
                            log::warn!("Failed to cache address for {}: {}", path_config.id, e);
                        } else {
                            count += 1;
                            log::debug!("🏠 Cached address for {}: {}", path_config.id, master_path_str);
                        }
                    }
                }
                Err(e) => {
                    log::debug!("Failed to get address for {}: {}", path_config.id, e);
                }
            }
        } else {
            // For other blockchains, use appropriate address request
            let request = match path_config.blockchain.as_str() {
                "ethereum" | "arbitrum" | "optimism" | "polygon" | "avalanche" | "base" | "bsc" => {
                    DeviceRequest::EthereumGetAddress {
                        path: master_path_str.clone(),
                        show_display: Some(path_config.show_display),
                    }
                },
                "cosmos" => DeviceRequest::CosmosGetAddress {
                    path: master_path_str.clone(),
                    hrp: "cosmos".to_string(),
                    show_display: Some(path_config.show_display),
                },
                "osmosis" => DeviceRequest::OsmosisGetAddress {
                    path: master_path_str.clone(),
                    show_display: Some(path_config.show_display),
                },
                "thorchain" => DeviceRequest::ThorchainGetAddress {
                    path: master_path_str.clone(),
                    testnet: false,
                    show_display: Some(path_config.show_display),
                },
                "mayachain" => DeviceRequest::MayachainGetAddress {
                    path: master_path_str.clone(),
                    show_display: Some(path_config.show_display),
                },
                "ripple" => DeviceRequest::XrpGetAddress {
                    path: master_path_str.clone(),
                    show_display: Some(path_config.show_display),
                },
                _ => {
                    log::debug!("Unsupported blockchain for frontload: {}", path_config.blockchain);
                    return Ok(0);
                }
            };
            
            match self.send_device_request(queue_handle, request).await {
                Ok(response) => {
                    if let Some(cached) = super::types::CachedPubkey::from_device_response(
                        device_id,
                        &master_path_str,
                        &path_config.blockchain,
                        Some(&path_config.script_type),
                        &response,
                    ) {
                        if let Err(e) = self.cache.save_pubkey(&cached).await {
                            log::warn!("Failed to cache {} address for {}: {}", path_config.blockchain, path_config.id, e);
                        } else {
                            count += 1;
                            log::debug!("🏠 Cached {} address for {}: {}", path_config.blockchain, path_config.id, master_path_str);
                        }
                    }
                }
                Err(e) => {
                    log::debug!("Failed to get {} address for {}: {}", path_config.blockchain, path_config.id, e);
                }
            }
        }
        
        Ok(count)
    }
    
    /// Send a device request through the queue
    async fn send_device_request(
        &self,
        queue_handle: &DeviceQueueHandle,
        request: DeviceRequest,
    ) -> Result<DeviceResponse> {
        // Generate a unique request ID
        let request_id = uuid::Uuid::new_v4().to_string();
        
        // Process the request through the appropriate handler
        let response = match &request {
            DeviceRequest::GetAddress { .. } | 
            DeviceRequest::GetPublicKey { .. } |
            DeviceRequest::GetFeatures => {
                crate::device::system_operations::process_system_request(
                    queue_handle,
                    &request,
                    &request_id,
                    &queue_handle.device_id(),
                ).await
                .map_err(|e| anyhow!("System operation failed: {}", e))
            }
            DeviceRequest::EthereumGetAddress { .. } |
            DeviceRequest::CosmosGetAddress { .. } |
            DeviceRequest::OsmosisGetAddress { .. } |
            DeviceRequest::ThorchainGetAddress { .. } |
            DeviceRequest::MayachainGetAddress { .. } |
            DeviceRequest::XrpGetAddress { .. } => {
                crate::device::address_operations::process_address_request(
                    queue_handle,
                    &request,
                    &request_id,
                    &queue_handle.device_id(),
                ).await
                .map_err(|e| anyhow!("Address operation failed: {}", e))
            }
            _ => Err(anyhow!("Unsupported request type for frontload")),
        }?;
        
        Ok(response)
    }
} 