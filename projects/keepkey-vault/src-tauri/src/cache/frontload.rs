use std::sync::Arc;
use anyhow::{Result, anyhow};
use keepkey_rust::device_queue::DeviceQueueHandle;
use super::{CacheManager, CacheMetadata};
use super::types::FrontloadStatus;
use crate::commands::{DeviceQueueManager, DeviceRequest, DeviceResponse};
use crate::pioneer_api::PioneerClient;
use uuid;

/// Controller for frontloading device public keys and addresses
pub struct FrontloadController {
    cache: Arc<CacheManager>,
    queue_manager: DeviceQueueManager,
    pioneer_client: Option<PioneerClient>,
}



impl FrontloadController {
    /// Create a new frontload controller with Pioneer API integration
    pub fn new(cache: Arc<CacheManager>, queue_manager: DeviceQueueManager) -> Self {
        // 🔧 HARDCODED API KEY - User's own free service, any string works
        let api_key = std::env::var("PIONEER_API_KEY").unwrap_or_else(|_| "1234".to_string());
        
        let pioneer_client = match PioneerClient::new(Some(api_key.clone())) {
            Ok(client) => {
                log::info!("✅ Pioneer API client initialized with key: {}", &api_key[0..4]);
                Some(client)
            }
            Err(e) => {
                log::error!("❌ Failed to initialize Pioneer API client: {}", e);
                log::info!("🔧 Trying with hardcoded fallback key...");
                // Last resort - use simple hardcoded key
                match PioneerClient::new(Some("1234".to_string())) {
                    Ok(client) => {
                        log::info!("✅ Pioneer API client initialized with hardcoded key");
                        Some(client)
                    }
                    Err(e2) => {
                        log::error!("❌ Even hardcoded key failed: {}", e2);
                        None
                    }
                }
            }
        };

        Self {
            cache,
            queue_manager,
            pioneer_client,
        }
    }
    
    /// Start frontloading for a device using cached asset and path data
    pub async fn frontload_device(&self, device_id: &str) -> Result<()> {
        log::info!("🔄 Starting cache-first frontload for device: {}", device_id);
        println!("🔍 DEBUG: Frontload entry point reached for device: {}", device_id);
        
        // Initialize cache if not already done
        println!("🔍 DEBUG: Checking cache initialization...");
        if !self.cache.is_cache_initialized().await.unwrap_or(false) {
            log::info!("🌱 Initializing asset cache during frontload...");
            self.cache.init_from_json_data().await
                .map_err(|e| anyhow!("Failed to initialize cache: {}", e))?;
        }
        println!("🔍 DEBUG: Cache initialization completed");
        
        // Get cached derivation paths, fallback to empty list if unavailable
        log::info!("🔍 DEBUG: Loading cached derivation paths...");
        let cached_paths = self.cache.get_all_paths().await.unwrap_or_else(|e| {
            log::warn!("⚠️ Failed to load cached paths, using empty list: {}", e);
            Vec::new()
        });
        log::info!("🔍 DEBUG: Loaded {} cached paths", cached_paths.len());
        
        if cached_paths.is_empty() {
            log::warn!("📋 No cached derivation paths available, skipping frontload");
            // Update metadata to mark as completed (but with warning)
            let metadata = CacheMetadata {
                device_id: device_id.to_string(),
                label: None,
                firmware_version: None,
                initialized: true,
                frontload_status: FrontloadStatus::Completed,
                frontload_progress: 100,
                last_frontload: Some(chrono::Utc::now().timestamp()),
                error_message: Some("No derivation paths available".to_string()),
            };
            self.cache.update_cache_metadata(&metadata).await?;
            return Ok(());
        }
        
        log::info!("📋 Using {} cached derivation paths from asset data", cached_paths.len());
        
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
        
        // Try to get device features, but don't fail frontload if it doesn't work
        log::info!("🔍 DEBUG: About to attempt device features...");
        let features = match self.get_or_create_queue_handle(device_id).await {
            Ok(queue_handle) => {
                log::info!("🔗 Device queue handle obtained, trying to get features...");
                match tokio::time::timeout(
                    std::time::Duration::from_secs(10), // Shorter timeout for frontload
                    queue_handle.get_features()
                ).await {
                    Ok(Ok(features)) => {
                        log::info!("✅ Device features retrieved successfully");
                        Some(features)
                    }
                    Ok(Err(e)) => {
                        log::warn!("⚠️ Failed to get device features: {} - continuing with cached data", e);
                        None
                    }
                    Err(_) => {
                        log::warn!("⚠️ Device features timeout - continuing with cached data");
                        println!("🔍 DEBUG: Device features timeout occurred");
                        None
                    }
                }
            }
            Err(e) => {
                log::warn!("⚠️ Failed to create device queue handle: {} - continuing with cached data", e);
                None
            }
        };
        
        // Update metadata with device info if available, otherwise use defaults
        let mut metadata = metadata;
        if let Some(ref features) = features {
            metadata.label = features.label.clone();
            metadata.firmware_version = Some(format!("{}.{}.{}", 
                features.major_version.unwrap_or(0),
                features.minor_version.unwrap_or(0),
                features.patch_version.unwrap_or(0)
            ));
            metadata.initialized = features.initialized.unwrap_or(false);
            
            // Check if device needs to be cache-wiped (seed change detection)
            if !metadata.initialized {
                log::warn!("Device {} not initialized, clearing cache", device_id);
                self.cache.clear_device_cache(device_id).await?;
                return Ok(());
            }
        } else {
            // Use cached metadata or defaults when device features unavailable
            log::info!("📋 Device features unavailable, using cached metadata or defaults");
            metadata.label = Some("KeepKey Device".to_string());
            metadata.firmware_version = Some("Unknown".to_string());
            metadata.initialized = true; // Assume initialized for frontload to proceed
        }
        self.cache.update_cache_metadata(&metadata).await?;
        
        let start_time = std::time::Instant::now();
        let mut total_cached = 0;
        let mut progress;
        let total_paths = cached_paths.len();
        let mut errors = Vec::new();
        
        // Process each cached derivation path - only if device features are available
        if features.is_some() {
            log::info!("🔑 Device responsive - deriving fresh addresses for {} paths", cached_paths.len());
            
            // Get queue handle for derivation (we know it should work since features worked)
            match self.get_or_create_queue_handle(device_id).await {
                Ok(queue_handle) => {
                    // Device communication is working, proceed with fresh derivation
                                        for (i, cached_path) in cached_paths.iter().enumerate() {
                        log::debug!("🔄 Processing cached path {}/{}: {} ({})", 
                            i + 1, total_paths, cached_path.path_id, cached_path.blockchain);
                        
                        // Check if we need to skip based on what we'll actually request
                        let mut skip_entirely = false;
                    
                        // For Bitcoin-like coins, check both account and master paths
                        if matches!(cached_path.blockchain.as_str(), "bitcoin" | "bitcoincash" | "litecoin" | "dogecoin" | "dash") {
                            let account_path = self.address_n_list_to_string(&cached_path.address_n_list);
                            let master_path = self.address_n_list_to_string(&cached_path.address_n_list_master);
                            
                            let account_cached = self.is_already_cached(device_id, &account_path, &cached_path.blockchain, cached_path.script_type.as_deref().unwrap_or("")).await?;
                            let master_cached = self.is_already_cached(device_id, &master_path, &cached_path.blockchain, cached_path.script_type.as_deref().unwrap_or("")).await?;
                            
                            if account_cached && master_cached {
                                log::debug!("⏭️ Skipping already cached Bitcoin-like path: {} (both account and master cached)", cached_path.path_id);
                                skip_entirely = true;
                            }
                                } else {
            // For other coins, only check master path since that's what we'll request
            let master_path = self.address_n_list_to_string(&cached_path.address_n_list_master);
            
            // 🔥 For EVM chains, check cache using "ethereum" to avoid duplicates
            let coin_name_for_check = match cached_path.blockchain.as_str() {
                "base" | "arbitrum" | "optimism" | "polygon" | "avalanche" | "bsc" => "ethereum",
                _ => &cached_path.blockchain,
            };
            
            if self.is_already_cached(device_id, &master_path, coin_name_for_check, cached_path.script_type.as_deref().unwrap_or("")).await? {
                log::debug!("⏭️ Skipping already cached path: {} (master, checking as {})", cached_path.path_id, coin_name_for_check);
                skip_entirely = true;
            }
        }
                        
                        if skip_entirely {
                            continue;
                        }
                        
                        // Frontload both account-level xpub and individual addresses
                        match self.frontload_cached_path(&queue_handle, device_id, cached_path).await {
                            Ok(count) => {
                                total_cached += count;
                                log::debug!("✅ Cached {} items for path: {}", count, cached_path.path_id);
                            }
                            Err(e) => {
                                log::warn!("⚠️ Failed to frontload path {}: {}", cached_path.path_id, e);
                                errors.push(format!("{}: {}", cached_path.path_id, e));
                            }
                        }
                        
                        // Update progress for xpub/address collection (0-70%)
                        progress = ((i + 1) * 70) / total_paths;
                        let mut progress_metadata = metadata.clone();
                        progress_metadata.frontload_progress = progress as i32;
                        self.cache.update_cache_metadata(&progress_metadata).await?;
                    }
                }
                Err(e) => {
                    log::warn!("⚠️ Failed to get queue handle for derivation: {}", e);
                    log::info!("📋 Skipping fresh derivation, will use cached data only");
                    // Continue without fresh derivation
                    total_cached = self.cache.get_device_pubkey_count(device_id).await.unwrap_or(0);
                    progress = 75;
                }
            }
        } else {
            log::info!("🔄 Device not responsive - using cached address data for {} paths", cached_paths.len());
            log::info!("🔍 DEBUG: Entering non-responsive device path");
            
            // Check if we have cached pubkeys/addresses for this device already
            let cached_count = self.cache.get_device_pubkey_count(device_id).await.unwrap_or(0);
            log::info!("🔍 DEBUG: Found {} cached pubkeys for device", cached_count);
            
            if cached_count > 0 {
                log::info!("📋 Found {} cached pubkeys for device, using existing data", cached_count);
                total_cached = cached_count;
            } else {
                log::warn!("📋 No cached data found for device - portfolio will be empty until device communication is restored");
                total_cached = 0;
            }
            
            progress = 75;
            if let Some(mut metadata) = self.cache.get_cache_metadata(device_id).await {
                metadata.frontload_progress = progress as i32;
                let _ = self.cache.update_cache_metadata(&metadata).await;
            }
            log::info!("🔍 DEBUG: Completed non-responsive device setup");
        }

        // Phase 2: 🚨 COMPREHENSIVE BLOCKCHAIN AUDIT & POPULATION
        println!("🔍 DEBUG: About to start blockchain audit phase...");
        log::info!("🔍 Starting comprehensive blockchain coverage audit...");
        match self.audit_and_ensure_blockchain_coverage(device_id).await {
            Ok(()) => {
                log::info!("✅ Blockchain coverage audit passed - all blockchains have pubkeys");
            }
            Err(e) => {
                log::error!("🚨 FATAL: Blockchain coverage audit failed: {}", e);
                
                // Update metadata to reflect audit failure
                let failed_metadata = CacheMetadata {
                    device_id: device_id.to_string(),
                    label: metadata.label.clone(),
                    firmware_version: metadata.firmware_version.clone(),
                    initialized: metadata.initialized,
                    frontload_status: FrontloadStatus::Failed,
                    frontload_progress: 50,
                    last_frontload: Some(chrono::Utc::now().timestamp()),
                    error_message: Some(format!("Blockchain coverage audit failed: {}", e)),
                };
                let _ = self.cache.update_cache_metadata(&failed_metadata).await;
                
                return Err(anyhow!("🚨 FRONTLOAD FAILED: {}", e));
            }
        }

        // Phase 3: Fetch portfolio data using collected xpubs (70-100%)
        log::info!("💰 Starting portfolio data collection phase...");
        match self.frontload_portfolio_data(device_id).await {
            Ok(portfolio_count) => {
                log::info!("✅ Cached portfolio data for {} assets", portfolio_count);
                
                // Clean up any duplicate portfolio balances
                match self.cache.clean_duplicate_portfolio_balances().await {
                    Ok(cleaned) => {
                        if cleaned > 0 {
                            log::info!("🧹 Cleaned {} duplicate portfolio balances", cleaned);
                        }
                    }
                    Err(e) => {
                        log::warn!("⚠️ Failed to clean duplicate portfolio balances: {}", e);
                    }
                }
            }
            Err(e) => {
                log::error!("🚨 CRITICAL: Failed to fetch portfolio data: {}", e);
                errors.push(format!("Portfolio fetch: {}", e));
                
                // Portfolio fetch failure is also critical - fail the frontload
                return Err(anyhow!("🚨 FRONTLOAD FAILED: Portfolio data fetch failed: {}", e));
            }
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
        
        // 💰 GET AND LOG PORTFOLIO USD VALUE 
        let portfolio_value = self.get_device_portfolio_total_usd(device_id).await.unwrap_or(0.0);
        let device_label = metadata.label.as_deref().unwrap_or("Unnamed KeepKey");
        
        let elapsed = start_time.elapsed();
        log::info!("✅ Frontload completed for device {}", device_id);
        log::info!("   📊 Processed {} paths, cached {} addresses/pubkeys in {:.2}s", 
            total_paths, total_cached, elapsed.as_secs_f64());
            
        // 🎯 THE NUMBER THAT MATTERS - TOTAL USD VALUE
        if portfolio_value > 0.0 {
            log::info!("   💰 PORTFOLIO VALUE: ${:.2} USD", portfolio_value);
            log::info!("   🏷️ Device: {} ({})", device_label, &device_id[device_id.len().saturating_sub(8)..]);
        } else {
            log::info!("   💰 PORTFOLIO VALUE: $0.00 USD (no balances or API unavailable)");
            log::info!("   🏷️ Device: {} ({})", device_label, &device_id[device_id.len().saturating_sub(8)..]);
        }
        
        if !errors.is_empty() {
            log::warn!("   ⚠️ {} errors occurred: {}", errors.len(), errors.join("; "));
        }
        log::info!("   💾 Data stored in SQLite cache for fast access");
        
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
    
    /// Frontload a single cached path configuration
    async fn frontload_cached_path(
        &self,
        queue_handle: &DeviceQueueHandle,
        device_id: &str,
        cached_path: &super::CachedPath,
    ) -> Result<usize> {
        let mut count = 0;
        
        // Convert the path to string format
        let account_path_str = self.address_n_list_to_string(&cached_path.address_n_list);
        let master_path_str = self.address_n_list_to_string(&cached_path.address_n_list_master);
        
        // For Bitcoin-like coins, get both XPUB (account level) and addresses (master level)
        if matches!(cached_path.blockchain.as_str(), "bitcoin" | "bitcoincash" | "litecoin" | "dogecoin" | "dash") {
            // 1. Get XPUB at account level (m/44'/0'/0') - but check cache first
            if !self.is_already_cached(device_id, &account_path_str, &cached_path.blockchain, cached_path.script_type.as_deref().unwrap_or("")).await? {
                let xpub_request = DeviceRequest::GetPublicKey {
                    path: account_path_str.clone(),
                    coin_name: Some(cached_path.blockchain.clone()),
                    script_type: cached_path.script_type.clone(),
                    ecdsa_curve_name: Some(cached_path.curve.clone()),
                    show_display: Some(cached_path.show_display),
                };
                
                match self.send_device_request(queue_handle, xpub_request).await {
                Ok(response) => {
                    if let Some(cached) = super::types::CachedPubkey::from_device_response(
                        device_id,
                        &account_path_str,
                        &cached_path.blockchain,
                        cached_path.script_type.as_deref(),
                        &response,
                    ) {
                        if let Err(e) = self.cache.save_pubkey(&cached).await {
                            log::warn!("Failed to cache XPUB for {}: {}", cached_path.path_id, e);
                        } else {
                            count += 1;
                            log::debug!("💰 Cached XPUB for {}: {}", cached_path.path_id, account_path_str);
                        }
                    }
                }
                    Err(e) => {
                        log::debug!("Failed to get XPUB for {}: {}", cached_path.path_id, e);
                    }
                }
            } else {
                log::debug!("⏭️ XPUB already cached for {}: {}", cached_path.path_id, account_path_str);
                count += 1;
            }
            
            // 2. Get address at master level (m/44'/0'/0'/0/0) - but check cache first
            if !self.is_already_cached(device_id, &master_path_str, &cached_path.blockchain, cached_path.script_type.as_deref().unwrap_or("")).await? {
                let address_request = DeviceRequest::GetAddress {
                    path: master_path_str.clone(),
                    coin_name: cached_path.blockchain.clone(),
                    script_type: cached_path.script_type.clone(),
                    show_display: Some(cached_path.show_display),
                };
                
                match self.send_device_request(queue_handle, address_request).await {
                Ok(response) => {
                    if let Some(cached) = super::types::CachedPubkey::from_device_response(
                        device_id,
                        &master_path_str,
                        &cached_path.blockchain,
                        cached_path.script_type.as_deref(),
                        &response,
                    ) {
                        if let Err(e) = self.cache.save_pubkey(&cached).await {
                            log::warn!("Failed to cache address for {}: {}", cached_path.path_id, e);
                        } else {
                            count += 1;
                            log::debug!("🏠 Cached address for {}: {}", cached_path.path_id, master_path_str);
                        }
                    }
                }
                    Err(e) => {
                        log::debug!("Failed to get address for {}: {}", cached_path.path_id, e);
                    }
                }
            } else {
                log::debug!("⏭️ Address already cached for {}: {}", cached_path.path_id, master_path_str);
                count += 1;
            }
        } else {
            // For other blockchains, use appropriate address request
            let request = match cached_path.blockchain.as_str() {
                "ethereum" | "arbitrum" | "optimism" | "polygon" | "avalanche" | "base" | "bsc" => {
                    DeviceRequest::EthereumGetAddress {
                        path: master_path_str.clone(),
                        show_display: Some(cached_path.show_display),
                    }
                },
                "cosmos" => DeviceRequest::CosmosGetAddress {
                    path: master_path_str.clone(),
                    hrp: "cosmos".to_string(),
                    show_display: Some(cached_path.show_display),
                },
                "osmosis" => DeviceRequest::OsmosisGetAddress {
                    path: master_path_str.clone(),
                    show_display: Some(cached_path.show_display),
                },
                "thorchain" => DeviceRequest::ThorchainGetAddress {
                    path: master_path_str.clone(),
                    testnet: false,
                    show_display: Some(cached_path.show_display),
                },
                "mayachain" => DeviceRequest::MayachainGetAddress {
                    path: master_path_str.clone(),
                    show_display: Some(cached_path.show_display),
                },
                "ripple" => DeviceRequest::XrpGetAddress {
                    path: master_path_str.clone(),
                    show_display: Some(cached_path.show_display),
                },
                _ => {
                    log::debug!("Unsupported blockchain for frontload: {}", cached_path.blockchain);
                    return Ok(0);
                }
            };
            
            match self.send_device_request(queue_handle, request).await {
                Ok(response) => {
                    // 🔥 For EVM chains, always save as "ethereum" to enable expansion
                    let coin_name_for_cache = match cached_path.blockchain.as_str() {
                        "base" | "arbitrum" | "optimism" | "polygon" | "avalanche" | "bsc" => "ethereum",
                        _ => &cached_path.blockchain,
                    };
                    
                    if let Some(cached) = super::types::CachedPubkey::from_device_response(
                        device_id,
                        &master_path_str,
                        coin_name_for_cache,
                        cached_path.script_type.as_deref(),
                        &response,
                    ) {
                        if let Err(e) = self.cache.save_pubkey(&cached).await {
                            log::warn!("Failed to cache {} address for {}: {}", coin_name_for_cache, cached_path.path_id, e);
                        } else {
                            count += 1;
                            log::debug!("🏠 Cached {} address for {} (original: {}): {}", 
                                coin_name_for_cache, cached_path.path_id, cached_path.blockchain, master_path_str);
                        }
                    }
                }
                Err(e) => {
                    log::debug!("Failed to get {} address for {}: {}", cached_path.blockchain, cached_path.path_id, e);
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

    /// Fetch and cache portfolio data using Pioneer API
    async fn frontload_portfolio_data(&self, device_id: &str) -> Result<usize> {
        // Check if Pioneer API client is available
        let pioneer_client = match &self.pioneer_client {
            Some(client) => client,
            None => {
                log::info!("📊 Skipping portfolio fetch - Pioneer API not configured");
                return Ok(0);
            }
        };

        // Collect all xpubs for this device from cached pubkeys
        let xpubs = self.collect_device_xpubs(device_id).await?;
        if xpubs.is_empty() {
            log::warn!("No xpubs found for device {}, skipping portfolio fetch", device_id);
            return Ok(0);
        }

        log::info!("💰 Fetching real portfolio data from Pioneer API for {} xpubs...", xpubs.len());

        // Update progress to 75%
        if let Some(mut metadata) = self.cache.get_cache_metadata(device_id).await {
            metadata.frontload_progress = 75;
            let _ = self.cache.update_cache_metadata(&metadata).await;
        }

        // Convert xpubs to PubkeyInfo for Pioneer API
        let pubkey_infos: Vec<crate::pioneer_api::PubkeyInfo> = xpubs.iter().map(|(xpub, blockchain)| {
            crate::pioneer_api::PubkeyInfo {
                pubkey: xpub.clone(),
                networks: vec![blockchain.clone()], // Use actual blockchain from derivation
                path: None, // Could be enhanced with actual derivation path
                address: None, // Address will be derived by Pioneer API
            }
        }).collect();

        // Fetch real portfolio balances from Pioneer API
        match pioneer_client.get_portfolio_balances(pubkey_infos).await {
            Ok(balances) => {
                log::info!("✅ Received {} real portfolio balances from Pioneer API", balances.len());
                
                // Cache the real portfolio data
                for balance in &balances {
                    if let Err(e) = self.cache.save_portfolio_balance(balance, device_id).await {
                        let ticker = balance.ticker.as_deref().unwrap_or("Unknown");
                        log::warn!("⚠️ Failed to cache balance for {}: {}", ticker, e);
                    }
                }
                
                log::info!("📊 Successfully cached {} real portfolio balances", balances.len());
                Ok(balances.len())
            }
            Err(e) => {
                log::warn!("⚠️ Pioneer API request failed: {}", e);
                log::info!("📊 No portfolio data cached due to API error");
                Ok(0)
            }
        }
    }

    /// Collect all xpubs for a device from the cache with blockchain info
    async fn collect_device_xpubs(&self, device_id: &str) -> Result<Vec<(String, String)>> {
        // 🌐 Pre-load EVM networks BEFORE SQLite to avoid Send issues
        let evm_networks = self.get_active_evm_networks().await;
        
        // 🔧 Load enabled blockchains for validation
        let enabled_blockchains = self.cache.load_enabled_blockchains().await?;
        
        let db = self.cache.db.lock().await;
        
        // 🔧 DEBUG: Check what device IDs exist in cache
        let device_debug: Vec<String> = db.prepare("SELECT DISTINCT device_id FROM cached_pubkeys")?
            .query_map([], |row| Ok(row.get::<_, String>(0)?))?
            .collect::<Result<Vec<_>, _>>()?;
        log::info!("🔍 DEBUG: All device_ids in cache: {:?}", device_debug);
        log::info!("🔍 DEBUG: Looking for device_id: {}", device_id);
        
        // 🔧 FIXED: Query both xpubs and addresses from cached_pubkeys with device ID fallback
        let mut stmt = db.prepare("
            SELECT DISTINCT 
                coin_name,
                xpub,
                address,
                derivation_path
            FROM cached_pubkeys 
            WHERE (device_id = ?1 OR device_id LIKE ?2)
            AND (xpub IS NOT NULL OR address IS NOT NULL)
            GROUP BY coin_name, COALESCE(xpub, address)
            ORDER BY last_used DESC
        ")?;
        
        let device_pattern = format!("%{}%", device_id.chars().take(8).collect::<String>());
        let rows = stmt.query_map([device_id, &device_pattern], |row| {
            Ok((
                row.get::<_, String>(0)?,  // coin_name
                row.get::<_, Option<String>>(1)?,  // xpub
                row.get::<_, Option<String>>(2)?,  // address
                row.get::<_, String>(3)?,  // derivation_path
            ))
        })?;
        
        let pubkey_data = rows.collect::<Result<Vec<_>, _>>()?;
        
        // 🔍 DEBUG: Log what coin_name values we found
        let coin_names: std::collections::HashSet<String> = pubkey_data.iter()
            .map(|(coin_name, _, _, _)| coin_name.clone())
            .collect();
        log::info!("🔍 DEBUG: Found cached pubkeys for coin_names: {:?}", coin_names);
        log::info!("🔍 DEBUG: Total cached pubkey entries: {}", pubkey_data.len());
        
        // Convert to (pubkey, caip) tuples - use addresses for Cosmos chains, xpubs for others
        let mut result = Vec::new();
        let mut seen_pubkeys = std::collections::HashSet::new();
        
        for (coin_name, xpub, address, _derivation_path) in pubkey_data {
            let (pubkey, caip) = match coin_name.to_lowercase().as_str() {
                // Cosmos chains need addresses, not xpubs
                "cosmos" => {
                    if let Some(addr) = address {
                        (addr, "cosmos:cosmoshub-4/slip44:118".to_string())
                    } else {
                        log::debug!("No address for cosmos");
                        continue;
                    }
                },
                "osmosis" => {
                    if let Some(addr) = address {
                        (addr, "cosmos:osmosis-1/slip44:118".to_string())
                    } else {
                        log::debug!("No address for osmosis");
                        continue;
                    }
                },
                "thorchain" => {
                    if let Some(addr) = address {
                        (addr, "cosmos:thorchain-mainnet-v1/slip44:931".to_string())
                    } else {
                        log::debug!("No address for thorchain");
                        continue;
                    }
                },
                "mayachain" => {
                    if let Some(addr) = address {
                        (addr, "cosmos:mayachain-mainnet-v1/slip44:931".to_string())
                    } else {
                        log::debug!("No address for mayachain");
                        continue;
                    }
                },
                // Bitcoin-like chains use xpubs
                "bitcoin" => {
                    if let Some(xpub_val) = xpub {
                        (xpub_val, "bip122:000000000019d6689c085ae165831e93/slip44:0".to_string())
                    } else {
                        log::debug!("No xpub for bitcoin pubkey");
                        continue;
                    }
                },
                // 🔥 EVM CHAINS: Use ONLY "ethereum" coin_name, expand to ALL EVM networks
                "ethereum" => {
                    // Try xpub first, but fall back to address (Ethereum uses addresses as pubkeys)
                    if let Some(xpub_val) = xpub {
                        log::info!("🚀 [EVM EXPANSION] Found ethereum xpub, expanding to {} EVM networks", evm_networks.len());
                        // 🌐 MULTI-CHAIN: Send same ethereum xpub to ALL EVM networks
                        for network_caip in &evm_networks {
                            if !seen_pubkeys.contains(&format!("{}:{}", xpub_val, network_caip)) {
                                log::info!("📊 EVM EXPANSION: ethereum xpub -> {}", network_caip);
                                result.push((xpub_val.clone(), network_caip.clone()));
                                seen_pubkeys.insert(format!("{}:{}", xpub_val, network_caip));
                            }
                        }
                        continue;
                    } else if let Some(addr) = address {
                        log::info!("🚀 [EVM EXPANSION] Found ethereum address (no xpub), using address as pubkey for {} EVM networks", evm_networks.len());
                        // 🌐 MULTI-CHAIN: For Ethereum, address IS the pubkey - expand to ALL EVM networks
                        for network_caip in &evm_networks {
                            if !seen_pubkeys.contains(&format!("{}:{}", addr, network_caip)) {
                                log::info!("📊 EVM EXPANSION: ethereum address -> {}", network_caip);
                                result.push((addr.clone(), network_caip.clone()));
                                seen_pubkeys.insert(format!("{}:{}", addr, network_caip));
                            }
                        }
                        continue;
                    } else {
                        log::warn!("⚠️ Found ethereum coin_name but no xpub OR address!");
                        continue;
                    }
                },
                // 🚫 Other EVM chains should be saved as "ethereum" - warn if found
                "base" | "arbitrum" | "optimism" | "polygon" | "avalanche" | "bsc" => {
                    log::warn!("⚠️ Found EVM chain {} in cache - should be saved as 'ethereum'", coin_name);
                    continue;
                },
                "litecoin" => {
                    if let Some(xpub_val) = xpub {
                        (xpub_val, "bip122:12a765e31ffd4059bada1e25190f6e98/slip44:2".to_string())
                    } else {
                        log::debug!("No xpub for litecoin");
                        continue;
                    }
                },
                "dogecoin" => {
                    if let Some(xpub_val) = xpub {
                        (xpub_val, "bip122:1a91e3dace36e2be3bf030a65679fe82/slip44:3".to_string())
                    } else {
                        log::debug!("No xpub for dogecoin");
                        continue;
                    }
                },
                "bitcoincash" => {
                    if let Some(xpub_val) = xpub {
                        (xpub_val, "bip122:000000000000000000651ef99cb9fcbe/slip44:145".to_string())
                    } else {
                        log::debug!("No xpub for bitcoincash");
                        continue;
                    }
                },
                "dash" => {
                    if let Some(xpub_val) = xpub {
                        (xpub_val, "bip122:0000ffd590b1485b3caadc19b22e637/slip44:5".to_string())
                    } else {
                        log::debug!("No xpub for dash");
                        continue;
                    }
                },
                "ripple" => {
                    if let Some(addr) = address {
                        (addr, "ripple:1/slip44:144".to_string())
                    } else {
                        log::debug!("No address for ripple");
                        continue;
                    }
                },
                _ => {
                    log::warn!("⚠️ MISSING MAPPING: Unknown coin type '{}' - this could be a missing asset!", coin_name);
                    continue;
                }
            };
            
            // Skip if we've already seen this specific pubkey+caip combination  
            let pubkey_caip_key = format!("{}:{}", pubkey, caip);
            if !seen_pubkeys.insert(pubkey_caip_key) {
                log::debug!("⏭️ Skipping duplicate pubkey+caip for {}: {} -> {}", coin_name, &pubkey[0..8], caip);
                continue;
            }
            
            log::debug!("📊 Adding: {} -> {}", coin_name, caip);
            
            result.push((pubkey, caip));
        }
        
        // 🔍 DEBUG: Log what we're actually sending to Pioneer API
        log::info!("🔍 DEBUG: Sending {} pubkeys to Pioneer API:", result.len());
        for (pubkey, caip) in &result {
            log::info!("  {} -> {}", &pubkey[0..8], caip);
        }
        
        // 🚨 FAIL FAST: Ensure we have at least 1 pubkey for EVERY enabled blockchain
        Self::validate_blockchain_coverage(&result, &enabled_blockchains)?;
        
        Ok(result)
    }

    /// 🚨 FAIL FAST: Validate that we have at least 1 pubkey for EVERY enabled blockchain
    /// This prevents silently missing blockchains and ensures complete portfolio coverage
    fn validate_blockchain_coverage(
        pubkeys: &[(String, String)], 
        enabled_blockchains: &[crate::cache::assets::BlockchainConfig]
    ) -> Result<()> {
        use std::collections::HashSet;
        
        // Extract unique CAIPs from pubkeys
        let covered_caips: HashSet<String> = pubkeys.iter()
            .map(|(_, caip)| caip.clone())
            .collect();
        
        let mut missing_blockchains = Vec::new();
        
        for blockchain in enabled_blockchains {
            if !covered_caips.contains(&blockchain.native_asset.caip) {
                missing_blockchains.push(format!("{} ({})", blockchain.name, blockchain.native_asset.caip));
            }
        }
        
        if !missing_blockchains.is_empty() {
            let error_msg = format!(
                "🚨 FATAL: Missing pubkeys for {} enabled blockchains!\n\
                 📋 Missing: {}\n\
                 💡 This indicates a fundamental frontload failure.\n\
                 🔧 Check device connectivity and derivation path configuration.\n\
                 ⚠️  Exiting to prevent incomplete portfolio data.",
                missing_blockchains.len(),
                missing_blockchains.join(", ")
            );
            
            log::error!("{}", error_msg);
            
            // 💀 FAIL FAST: Exit the entire application
            std::process::exit(1);
        }
        
        log::info!("✅ VALIDATION PASSED: All {} enabled blockchains have pubkey coverage", enabled_blockchains.len());
        Ok(())
    }

    /// Get list of active EVM networks from blockchain configuration
    async fn get_active_evm_networks(&self) -> Vec<String> {
        match self.cache.get_evm_networks().await {
            Ok(evm_networks) => {
                log::info!("📊 Using {} EVM networks from blockchain configuration", evm_networks.len());
                evm_networks
            }
            Err(e) => {
                log::warn!("⚠️ Failed to load EVM networks from config, using fallback: {}", e);
                // Fallback to hardcoded list if configuration fails
                vec![
                    "eip155:1/slip44:60".to_string(),      // Ethereum Mainnet
                    "eip155:8453/slip44:60".to_string(),   // Base
                    "eip155:137/slip44:60".to_string(),    // Polygon  
                    "eip155:56/slip44:60".to_string(),     // BSC (Binance Smart Chain)
                    "eip155:10/slip44:60".to_string(),     // Optimism
                    "eip155:42161/slip44:60".to_string(),  // Arbitrum One
                    "eip155:43114/slip44:60".to_string(),  // Avalanche C-Chain
                ]
            }
        }
    }

    /// Get total USD value of portfolio for a device
    pub async fn get_device_portfolio_total_usd(&self, device_id: &str) -> Result<f64> {
        // Get all portfolio balances for this device
        let balances = self.cache.get_device_portfolio(device_id).await?;
        
        if balances.is_empty() {
            return Ok(0.0);
        }
        
        // Calculate total USD value
        let mut total_usd = 0.0;
        for balance in &balances {
            if let Ok(value) = balance.value_usd.parse::<f64>() {
                total_usd += value;
            }
        }
        
        Ok(total_usd)
    }

    /// 🚨 FAIL FAST: Comprehensive blockchain coverage audit and enforcement
    /// This ensures we have pubkeys for ALL enabled blockchains or fails hard
    async fn audit_and_ensure_blockchain_coverage(&self, device_id: &str) -> Result<()> {
        log::info!("🔍 AUDIT: Starting comprehensive blockchain coverage audit for device: {}", device_id);
        
        // Load enabled blockchains
        let enabled_blockchains = self.cache.load_enabled_blockchains().await?;
        log::info!("📋 AUDIT: Checking coverage for {} enabled blockchains", enabled_blockchains.len());
        
        // Check current pubkey coverage
        let current_pubkeys = self.collect_device_xpubs(device_id).await?;
        log::info!("📊 AUDIT: Found {} existing pubkeys in cache", current_pubkeys.len());
        
        // Validate coverage
        let missing_blockchains = self.identify_missing_blockchain_coverage(&current_pubkeys, &enabled_blockchains)?;
        
        if missing_blockchains.is_empty() {
            log::info!("✅ AUDIT: Complete blockchain coverage verified - all {} blockchains have pubkeys", enabled_blockchains.len());
            return Ok(());
        }
        
        log::error!("🚨 AUDIT: Missing pubkeys for {} blockchains!", missing_blockchains.len());
        for missing in &missing_blockchains {
            log::error!("   ❌ Missing: {}", missing);
        }
        
        // Attempt to populate missing blockchains with aggressive retry
        log::info!("🔧 AUDIT: Attempting to populate missing blockchain pubkeys...");
        
        match self.force_populate_missing_blockchains(device_id, &enabled_blockchains).await {
            Ok(populated_count) => {
                log::info!("✅ AUDIT: Successfully populated {} blockchain pubkeys", populated_count);
                
                // Re-audit to verify success
                let updated_pubkeys = self.collect_device_xpubs(device_id).await?;
                let remaining_missing = self.identify_missing_blockchain_coverage(&updated_pubkeys, &enabled_blockchains)?;
                
                if remaining_missing.is_empty() {
                    log::info!("✅ AUDIT: Complete blockchain coverage achieved after population");
                    return Ok(());
                } else {
                    log::error!("🚨 AUDIT: Still missing {} blockchains after population attempt", remaining_missing.len());
                    for missing in &remaining_missing {
                        log::error!("   ❌ Still missing: {}", missing);
                    }
                    return Err(anyhow!("Failed to achieve complete blockchain coverage. Missing: {}", remaining_missing.join(", ")));
                }
            }
            Err(e) => {
                log::error!("🚨 AUDIT: Failed to populate missing blockchains: {}", e);
                return Err(anyhow!("Cannot achieve complete blockchain coverage: {}. Missing: {}", e, missing_blockchains.join(", ")));
            }
        }
    }
    
    /// Identify which blockchains are missing pubkey coverage
    fn identify_missing_blockchain_coverage(
        &self,
        pubkeys: &[(String, String)], 
        enabled_blockchains: &[crate::cache::assets::BlockchainConfig]
    ) -> Result<Vec<String>> {
        use std::collections::HashSet;
        
        // Extract unique CAIPs from pubkeys
        let covered_caips: HashSet<String> = pubkeys.iter()
            .map(|(_, caip)| caip.clone())
            .collect();
        
        log::info!("🔍 AUDIT: Covered CAIPs: {:?}", covered_caips);
        
        // Check each enabled blockchain for coverage
        let mut missing_blockchains = Vec::new();
        
        for blockchain in enabled_blockchains {
            if !covered_caips.contains(&blockchain.native_asset.caip) {
                missing_blockchains.push(format!("{} ({})", blockchain.name, blockchain.native_asset.caip));
            }
        }
        
        Ok(missing_blockchains)
    }
    
    /// 🔧 FORCE POPULATE: Aggressively attempt to generate missing blockchain pubkeys
    /// Uses multiple strategies including device retry, cached paths, and fallback derivation
    async fn force_populate_missing_blockchains(&self, device_id: &str, enabled_blockchains: &[crate::cache::assets::BlockchainConfig]) -> Result<usize> {
        log::info!("🔧 FORCE POPULATE: Starting aggressive pubkey population for device: {}", device_id);
        
        let mut populated_count = 0;
        
        // Strategy 1: Retry device communication with longer timeout
        log::info!("🔧 Strategy 1: Retry device communication with extended timeout");
        if let Ok(populated) = self.retry_device_derivation_with_extended_timeout(device_id).await {
            populated_count += populated;
            log::info!("✅ Strategy 1: Populated {} pubkeys via device retry", populated);
        } else {
            log::warn!("⚠️ Strategy 1: Device retry failed, proceeding to fallback strategies");
        }
        
        // Strategy 2: Use cached derivation paths to generate missing pubkeys
        if populated_count == 0 {
            log::info!("🔧 Strategy 2: Force derive using cached paths and known derivations");
            if let Ok(populated) = self.force_derive_from_cached_paths(device_id).await {
                populated_count += populated;
                log::info!("✅ Strategy 2: Populated {} pubkeys via cached path derivation", populated);
            } else {
                log::warn!("⚠️ Strategy 2: Cached path derivation failed");
            }
        }
        
        // Strategy 3: Last resort - derive using standard paths for missing blockchains
        if populated_count == 0 {
            log::info!("🔧 Strategy 3: Last resort - derive using standard blockchain paths");
            if let Ok(populated) = self.derive_standard_paths_for_blockchains(device_id, enabled_blockchains).await {
                populated_count += populated;
                log::info!("✅ Strategy 3: Populated {} pubkeys via standard path derivation", populated);
            } else {
                log::error!("❌ Strategy 3: All strategies failed - cannot populate pubkeys");
            }
        }
        
        if populated_count > 0 {
            log::info!("✅ FORCE POPULATE: Successfully populated {} pubkeys total", populated_count);
            Ok(populated_count)
        } else {
            Err(anyhow!("All population strategies failed - device may be disconnected or unresponsive"))
        }
    }
    
    /// Retry device communication with extended timeout and multiple attempts
    async fn retry_device_derivation_with_extended_timeout(&self, device_id: &str) -> Result<usize> {
        log::info!("🔄 RETRY: Attempting device derivation with extended timeout (30s)");
        
        let mut last_error = None;
        
        // Try 3 times with increasing timeouts
        for attempt in 1..=3 {
            let timeout_seconds = 10 + (attempt * 10); // 20s, 30s, 40s
            log::info!("🔄 RETRY: Attempt {}/3 with {}s timeout", attempt, timeout_seconds);
            
            match self.get_or_create_queue_handle(device_id).await {
                Ok(queue_handle) => {
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(timeout_seconds),
                        queue_handle.get_features()
                    ).await {
                        Ok(Ok(_features)) => {
                            log::info!("✅ RETRY: Device features obtained on attempt {}", attempt);
                            
                                                         // Device is responsive, try to derive all cached paths
                             let cached_paths = self.cache.get_all_paths().await?;
                             let mut derived_count = 0;
                             
                             for cached_path in cached_paths {
                                 match self.frontload_cached_path(&queue_handle, device_id, &cached_path).await {
                                     Ok(count) => {
                                         derived_count += count;
                                     }
                                     Err(e) => {
                                         log::warn!("⚠️ Failed to derive path {}: {}", cached_path.path_id, e);
                                     }
                                 }
                             }
                            
                            if derived_count > 0 {
                                log::info!("✅ RETRY: Successfully derived {} pubkeys", derived_count);
                                return Ok(derived_count);
                            }
                        }
                        Ok(Err(e)) => {
                            log::warn!("⚠️ RETRY: Attempt {}: Device features error: {}", attempt, e);
                            last_error = Some(e.to_string());
                        }
                        Err(_) => {
                            log::warn!("⚠️ RETRY: Attempt {}: Timeout after {}s", attempt, timeout_seconds);
                            last_error = Some(format!("Timeout after {}s", timeout_seconds));
                        }
                    }
                }
                Err(e) => {
                    log::warn!("⚠️ RETRY: Attempt {}: Failed to create queue handle: {}", attempt, e);
                    last_error = Some(e.to_string());
                }
            }
            
            // Wait before next attempt
            if attempt < 3 {
                log::info!("⏳ RETRY: Waiting 2s before next attempt...");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
        
        Err(anyhow!("Device retry failed after 3 attempts. Last error: {}", last_error.unwrap_or("Unknown".to_string())))
    }
    
    /// Force derive using cached paths even without device features
    async fn force_derive_from_cached_paths(&self, device_id: &str) -> Result<usize> {
        log::info!("🔧 FORCE DERIVE: Attempting derivation using cached paths");
        
        // Try to get queue handle even if features failed
        match self.get_or_create_queue_handle(device_id).await {
            Ok(queue_handle) => {
                                 let cached_paths = self.cache.get_all_paths().await?;
                let mut derived_count = 0;
                
                log::info!("🔧 FORCE DERIVE: Attempting to derive {} cached paths", cached_paths.len());
                
                for cached_path in cached_paths {
                    // Try to derive this path with shorter timeout to avoid hanging
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        self.frontload_cached_path(&queue_handle, device_id, &cached_path)
                    ).await {
                        Ok(Ok(count)) => {
                            derived_count += count;
                            log::debug!("✅ Derived {} items for path: {}", count, cached_path.path_id);
                        }
                        Ok(Err(e)) => {
                            log::warn!("⚠️ Failed to derive path {}: {}", cached_path.path_id, e);
                        }
                        Err(_) => {
                            log::warn!("⚠️ Timeout deriving path: {}", cached_path.path_id);
                        }
                    }
                }
                
                if derived_count > 0 {
                    log::info!("✅ FORCE DERIVE: Successfully derived {} pubkeys from cached paths", derived_count);
                    Ok(derived_count)
                } else {
                    Err(anyhow!("Failed to derive any pubkeys from cached paths"))
                }
            }
            Err(e) => {
                Err(anyhow!("Cannot create queue handle for forced derivation: {}", e))
            }
        }
    }
    
    /// Last resort: derive standard paths for all enabled blockchains
    async fn derive_standard_paths_for_blockchains(&self, device_id: &str, enabled_blockchains: &[crate::cache::assets::BlockchainConfig]) -> Result<usize> {
        log::info!("🔧 STANDARD PATHS: Deriving standard paths for {} blockchains", enabled_blockchains.len());
        
        match self.get_or_create_queue_handle(device_id).await {
            Ok(queue_handle) => {
                let mut derived_count = 0;
                
                for blockchain in enabled_blockchains {
                    match self.derive_standard_path_for_blockchain(&queue_handle, device_id, blockchain).await {
                        Ok(count) => {
                            derived_count += count;
                            log::info!("✅ Derived {} pubkeys for blockchain: {}", count, blockchain.name);
                        }
                        Err(e) => {
                            log::warn!("⚠️ Failed to derive standard path for {}: {}", blockchain.name, e);
                        }
                    }
                }
                
                if derived_count > 0 {
                    log::info!("✅ STANDARD PATHS: Successfully derived {} pubkeys total", derived_count);
                    Ok(derived_count)
                } else {
                    Err(anyhow!("Failed to derive any standard paths"))
                }
            }
            Err(e) => {
                Err(anyhow!("Cannot create queue handle for standard path derivation: {}", e))
            }
        }
    }
    
    /// Derive a standard path for a specific blockchain
    async fn derive_standard_path_for_blockchain(&self, queue_handle: &DeviceQueueHandle, device_id: &str, blockchain: &crate::cache::assets::BlockchainConfig) -> Result<usize> {
        // Create standard derivation path based on blockchain type
        let derivation_path = format!("m/44'/{}'/0'", blockchain.slip44);
        let path_vec = self.parse_derivation_path(&derivation_path)?;
        
        log::debug!("🔧 Deriving standard path for {}: {}", blockchain.name, derivation_path);
        
        let coin_name = match blockchain.chain_type.as_str() {
            "evm" => "ethereum",  // All EVM chains use ethereum coin_name
            "utxo" => match blockchain.symbol.to_lowercase().as_str() {
                "btc" => "bitcoin",
                "bch" => "bitcoincash", 
                "ltc" => "litecoin",
                "doge" => "dogecoin",
                "dash" => "dash",
                _ => &blockchain.symbol.to_lowercase(),
            },
            "cosmos" => match blockchain.symbol.to_lowercase().as_str() {
                "atom" => "cosmos",
                "osmo" => "osmosis", 
                "rune" => "thorchain",
                "cacao" => "mayachain",
                _ => &blockchain.symbol.to_lowercase(),
            },
            "ripple" => "ripple",
            _ => &blockchain.symbol.to_lowercase(),
        };
        
        // For Bitcoin-like coins, get xpub; for others, get address
        let mut derived_count = 0;
        
                 if matches!(blockchain.chain_type.as_str(), "utxo") {
             // Derive xpub for UTXO chains
             match self.derive_xpub_for_blockchain(queue_handle, device_id, blockchain, &derivation_path, &path_vec, coin_name).await {
                 Ok(count) => {
                     derived_count += count;
                     log::debug!("✅ Cached {} xpubs for blockchain: {}", count, blockchain.name);
                 }
                 Err(e) => {
                     log::warn!("⚠️ Failed to derive xpub for {}: {}", blockchain.name, e);
                 }
             }
         } else {
             // Derive address for other chains
             match self.derive_address_for_blockchain(queue_handle, device_id, blockchain, &derivation_path, &path_vec, coin_name).await {
                 Ok(count) => {
                     derived_count += count;
                     log::debug!("✅ Cached {} addresses for blockchain: {}", count, blockchain.name);
                 }
                 Err(e) => {
                     log::warn!("⚠️ Failed to derive address for {}: {}", blockchain.name, e);
                 }
             }
         }
        
        Ok(derived_count)
    }
    
    /// Parse a derivation path string into a vector of u32
    fn parse_derivation_path(&self, path: &str) -> Result<Vec<u32>> {
        // Remove 'm/' prefix if present
        let path = if path.starts_with("m/") { &path[2..] } else { path };
        
        let mut result = Vec::new();
        for segment in path.split('/') {
            if segment.is_empty() {
                continue;
            }
            
            let (num_str, hardened) = if segment.ends_with('\'') || segment.ends_with('h') {
                (&segment[..segment.len()-1], true)
            } else {
                (segment, false)
            };
            
            let num: u32 = num_str.parse()
                .map_err(|_| anyhow!("Invalid derivation path segment: {}", segment))?;
            
            let value = if hardened { num + 0x80000000 } else { num };
            result.push(value);
        }
        
        Ok(result)
    }
    
    /// Derive xpub for UTXO blockchain
    async fn derive_xpub_for_blockchain(
        &self,
        queue_handle: &DeviceQueueHandle,
        device_id: &str,
        blockchain: &crate::cache::assets::BlockchainConfig,
        derivation_path: &str,
        path_vec: &[u32],
        coin_name: &str
    ) -> Result<usize> {
        let path_str = self.address_n_list_to_string(path_vec);
        
        let request = DeviceRequest::GetPublicKey {
            path: path_str.clone(),
            coin_name: Some(coin_name.to_string()),
            script_type: Some("p2pkh".to_string()),
            ecdsa_curve_name: Some("secp256k1".to_string()),
            show_display: Some(false),
        };
        
        match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.send_device_request(queue_handle, request)
        ).await {
            Ok(Ok(response)) => {
                if let Some(cached) = super::types::CachedPubkey::from_device_response(
                    device_id,
                    derivation_path,
                    coin_name,
                    Some("p2pkh"),
                    &response,
                ) {
                    self.cache.save_pubkey(&cached).await?;
                    Ok(1)
                } else {
                    Err(anyhow!("Failed to create cached pubkey from device response"))
                }
            }
            Ok(Err(e)) => Err(anyhow!("Device error getting public key: {}", e)),
            Err(_) => Err(anyhow!("Timeout getting public key")),
        }
    }
    
    /// Derive address for non-UTXO blockchain
    async fn derive_address_for_blockchain(
        &self,
        queue_handle: &DeviceQueueHandle,
        device_id: &str,
        blockchain: &crate::cache::assets::BlockchainConfig,
        derivation_path: &str,
        path_vec: &[u32],
        coin_name: &str
    ) -> Result<usize> {
        let path_str = self.address_n_list_to_string(path_vec);
        
        let request = DeviceRequest::GetAddress {
            path: path_str.clone(),
            coin_name: coin_name.to_string(),
            script_type: None,
            show_display: Some(false),
        };
        
        match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.send_device_request(queue_handle, request)
        ).await {
            Ok(Ok(response)) => {
                if let Some(cached) = super::types::CachedPubkey::from_device_response(
                    device_id,
                    derivation_path,
                    coin_name,
                    None,
                    &response,
                ) {
                    self.cache.save_pubkey(&cached).await?;
                    Ok(1)
                } else {
                    Err(anyhow!("Failed to create cached address from device response"))
                }
            }
            Ok(Err(e)) => Err(anyhow!("Device error getting address: {}", e)),
            Err(_) => Err(anyhow!("Timeout getting address")),
        }
    }
} 