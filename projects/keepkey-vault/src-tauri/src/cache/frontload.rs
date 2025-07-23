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
    /// Create a new frontload controller with optional Pioneer API integration
    pub fn new(cache: Arc<CacheManager>, queue_manager: DeviceQueueManager) -> Self {
        // Try to create Pioneer client - generate unique API key if none provided
        let pioneer_client = match std::env::var("PIONEER_API_KEY") {
            Ok(api_key) => {
                match PioneerClient::new(Some(api_key)) {
                    Ok(client) => {
                        log::info!("✅ Pioneer API client initialized with provided API key");
                        Some(client)
                    }
                    Err(e) => {
                        log::warn!("⚠️ Failed to initialize Pioneer API client with provided key: {}", e);
                        None
                    }
                }
            }
            Err(_) => {
                // Generate a unique UUID as API key for this user session
                let unique_api_key = uuid::Uuid::new_v4().to_string();
                log::info!("🔑 No PIONEER_API_KEY found, generated unique session key: {}", &unique_api_key[0..8]);
                match PioneerClient::new(Some(unique_api_key)) {
                    Ok(client) => {
                        log::info!("✅ Pioneer API client initialized with unique session key");
                        Some(client)
                    }
                    Err(e) => {
                        log::warn!("⚠️ Failed to initialize Pioneer API client with generated key: {}", e);
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
        
        // Initialize cache if not already done
        if !self.cache.is_cache_initialized().await.unwrap_or(false) {
            log::info!("🌱 Initializing asset cache during frontload...");
            self.cache.init_from_json_data().await
                .map_err(|e| anyhow!("Failed to initialize cache: {}", e))?;
        }
        
        // Get cached derivation paths, fallback to empty list if unavailable
        let cached_paths = self.cache.get_all_paths().await.unwrap_or_else(|e| {
            log::warn!("⚠️ Failed to load cached paths, using empty list: {}", e);
            Vec::new()
        });
        
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
        let total_paths = cached_paths.len();
        let mut errors = Vec::new();
        
        // Process each cached derivation path
        for (i, cached_path) in cached_paths.iter().enumerate() {
            log::debug!("🔄 Processing cached path {}/{}: {} ({})", 
                i + 1, total_paths, cached_path.path_id, cached_path.blockchain);
            
            // Skip if already cached (check cache first)
            let derivation_path = self.address_n_list_to_string(&cached_path.address_n_list);
            if self.is_already_cached(device_id, &derivation_path, &cached_path.blockchain, cached_path.script_type.as_deref().unwrap_or("")).await? {
                log::debug!("⏭️ Skipping already cached path: {}", cached_path.path_id);
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

        // Phase 2: Fetch portfolio data using collected xpubs (70-100%)
        log::info!("💰 Starting portfolio data collection phase...");
        match self.frontload_portfolio_data(device_id).await {
            Ok(portfolio_count) => {
                log::info!("✅ Cached portfolio data for {} assets", portfolio_count);
            }
            Err(e) => {
                log::warn!("⚠️ Failed to fetch portfolio data: {}", e);
                errors.push(format!("Portfolio fetch: {}", e));
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
            // 1. Get XPUB at account level (m/44'/0'/0')
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
            
            // 2. Get address at master level (m/44'/0'/0'/0/0)
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
                    if let Some(cached) = super::types::CachedPubkey::from_device_response(
                        device_id,
                        &master_path_str,
                        &cached_path.blockchain,
                        cached_path.script_type.as_deref(),
                        &response,
                    ) {
                        if let Err(e) = self.cache.save_pubkey(&cached).await {
                            log::warn!("Failed to cache {} address for {}: {}", cached_path.blockchain, cached_path.path_id, e);
                        } else {
                            count += 1;
                            log::debug!("🏠 Cached {} address for {}: {}", cached_path.blockchain, cached_path.path_id, master_path_str);
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
        let db = self.cache.db.lock().await;
        
        // Query both xpubs and addresses from cached_pubkeys, similar to portfolio refresh
        let mut stmt = db.prepare("
            SELECT DISTINCT 
                coin_name,
                xpub,
                address
            FROM cached_pubkeys 
            WHERE device_id = ?1 
            AND (xpub IS NOT NULL OR address IS NOT NULL)
        ")?;
        
        let rows = stmt.query_map([device_id], |row| {
            Ok((
                row.get::<_, String>(0)?,  // coin_name
                row.get::<_, Option<String>>(1)?,  // xpub
                row.get::<_, Option<String>>(2)?,  // address
            ))
        })?;
        
        let pubkey_data = rows.collect::<Result<Vec<_>, _>>()?;
        
        // Convert to (pubkey, caip) tuples - use addresses for Cosmos chains, xpubs for others
        let mut result = Vec::new();
        for (coin_name, xpub, address) in pubkey_data {
            let (pubkey, caip) = match coin_name.to_lowercase().as_str() {
                // Cosmos chains need addresses, not xpubs
                "cosmos" => {
                    if let Some(addr) = address {
                        (addr, "cosmos:cosmoshub-4/slip44:118".to_string())
                    } else {
                        log::warn!("⚠️ No address found for cosmos pubkey, skipping");
                        continue;
                    }
                },
                "osmosis" => {
                    if let Some(addr) = address {
                        (addr, "cosmos:osmosis-1/slip44:118".to_string())
                    } else {
                        log::warn!("⚠️ No address found for osmosis pubkey, skipping");
                        continue;
                    }
                },
                "thorchain" => {
                    if let Some(addr) = address {
                        (addr, "cosmos:thorchain-mainnet-v1/slip44:931".to_string())
                    } else {
                        log::warn!("⚠️ No address found for thorchain pubkey, skipping");
                        continue;
                    }
                },
                "mayachain" => {
                    if let Some(addr) = address {
                        (addr, "cosmos:mayachain-mainnet-v1/slip44:931".to_string())
                    } else {
                        log::warn!("⚠️ No address found for mayachain pubkey, skipping");
                        continue;
                    }
                },
                // Bitcoin-like chains use xpubs
                "bitcoin" => {
                    if let Some(xpub_val) = xpub {
                        (xpub_val, "bip122:000000000019d6689c085ae165831e93/slip44:0".to_string())
                    } else {
                        log::warn!("⚠️ No xpub found for bitcoin pubkey, skipping");
                        continue;
                    }
                },
                "ethereum" => {
                    if let Some(xpub_val) = xpub {
                        (xpub_val, "eip155:1/slip44:60".to_string())
                    } else {
                        log::warn!("⚠️ No xpub found for ethereum pubkey, skipping");
                        continue;
                    }
                },
                "litecoin" => {
                    if let Some(xpub_val) = xpub {
                        (xpub_val, "bip122:12a765e31ffd4059bada1e25190f6e98/slip44:2".to_string())
                    } else {
                        log::warn!("⚠️ No xpub found for litecoin pubkey, skipping");
                        continue;
                    }
                },
                "dogecoin" => {
                    if let Some(xpub_val) = xpub {
                        (xpub_val, "bip122:1a91e3dace36e2be3bf030a65679fe82/slip44:3".to_string())
                    } else {
                        log::warn!("⚠️ No xpub found for dogecoin pubkey, skipping");
                        continue;
                    }
                },
                "bitcoincash" => {
                    if let Some(xpub_val) = xpub {
                        (xpub_val, "bip122:000000000000000000651ef99cb9fcbe/slip44:145".to_string())
                    } else {
                        log::warn!("⚠️ No xpub found for bitcoincash pubkey, skipping");
                        continue;
                    }
                },
                "dash" => {
                    if let Some(xpub_val) = xpub {
                        (xpub_val, "bip122:00000ffd590b1485b3caadc19b22e637/slip44:5".to_string())
                    } else {
                        log::warn!("⚠️ No xpub found for dash pubkey, skipping");
                        continue;
                    }
                },
                "ripple" => {
                    if let Some(addr) = address {
                        (addr, "ripple:1/slip44:144".to_string())
                    } else {
                        log::warn!("⚠️ No address found for ripple pubkey, skipping");
                        continue;
                    }
                },
                _ => {
                    log::warn!("⚠️ Unknown coin type: {}, skipping", coin_name);
                    continue;
                }
            };
            
            log::info!("📊 Frontload adding: {} -> {} ({})", coin_name, caip,
                if coin_name.starts_with("cosmos") || coin_name.ends_with("chain") { "address" } else { "xpub" });
            
            result.push((pubkey, caip));
        }
        
        Ok(result)
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
} 