use std::sync::Arc;
use anyhow::{Result, anyhow};
use keepkey_rust::device_queue::DeviceQueueHandle;
use super::{CacheManager, CacheMetadata};
use super::types::FrontloadStatus;
use crate::commands::{DeviceQueueManager, DeviceRequest, DeviceResponse};
use crate::pioneer_api::{PioneerClient, PortfolioRequest};
use serde::{Deserialize, Serialize};
use serde_json;

/// Controller for frontloading device public keys and addresses
pub struct FrontloadController {
    cache: Arc<CacheManager>,
    queue_manager: DeviceQueueManager,
    pioneer_client: Option<PioneerClient>,
}



impl FrontloadController {
    /// Create a new frontload controller with optional Pioneer API integration
    pub fn new(cache: Arc<CacheManager>, queue_manager: DeviceQueueManager) -> Self {
        // Try to create Pioneer client - don't fail if API key is missing
        let pioneer_client = match std::env::var("PIONEER_API_KEY") {
            Ok(api_key) => {
                match PioneerClient::new(Some(api_key)) {
                    Ok(client) => {
                        log::info!("✅ Pioneer API client initialized for portfolio fetching");
                        Some(client)
                    }
                    Err(e) => {
                        log::warn!("⚠️ Failed to initialize Pioneer API client: {}", e);
                        None
                    }
                }
            }
            Err(_) => {
                log::info!("ℹ️ No PIONEER_API_KEY found, portfolio fetching disabled");
                None
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

    /// Fetch and cache portfolio data for a device using collected xpubs
    async fn frontload_portfolio_data(&self, device_id: &str) -> Result<usize> {
        // Check if Pioneer API client is available
        let pioneer_client = match &self.pioneer_client {
            Some(client) => client,
            None => {
                log::info!("📊 Skipping portfolio fetch - Pioneer API not configured");
                return Ok(0);
            }
        };

        // Collect all xpubs for this device
        let xpubs = self.collect_device_xpubs(device_id).await?;
        if xpubs.is_empty() {
            log::warn!("No xpubs found for device {}, skipping portfolio fetch", device_id);
            return Ok(0);
        }

        log::info!("💰 Fetching portfolio data for {} xpubs...", xpubs.len());

        // Update progress to 75%
        if let Some(mut metadata) = self.cache.get_cache_metadata(device_id).await {
            metadata.frontload_progress = 75;
            let _ = self.cache.update_cache_metadata(&metadata).await;
        }

        // Create portfolio requests with proper CAIP construction
        let mut portfolio_requests = Vec::new();
        for (xpub, blockchain) in &xpubs {
            // Build proper CAIP based on blockchain and cached asset data
            if let Some(caip) = self.build_caip_for_xpub(blockchain).await {
                portfolio_requests.push(PortfolioRequest {
                    caip,
                    pubkey: xpub.clone(),
                });
            } else {
                log::warn!("⚠️ No CAIP mapping found for blockchain: {}", blockchain);
            }
        }

        // Fetch portfolio balances
        match pioneer_client.get_portfolio_balances(portfolio_requests).await {
            Ok(portfolio_balances) => {
                log::info!("📈 Received {} balance entries from Pioneer API", portfolio_balances.len());

                // Update progress to 85%
                if let Some(mut metadata) = self.cache.get_cache_metadata(device_id).await {
                    metadata.frontload_progress = 85;
                    let _ = self.cache.update_cache_metadata(&metadata).await;
                }

                // Save portfolio balances to cache with pubkey linkage
                let mut saved_count = 0;
                for balance in &portfolio_balances {
                    // Try to find the specific pubkey that generated this balance
                    let matching_pubkey = self.find_pubkey_for_balance(device_id, balance).await;
                    match self.cache.save_portfolio_balance_with_pubkey(
                        balance, 
                        device_id, 
                        matching_pubkey.as_deref()
                    ).await {
                        Ok(_) => saved_count += 1,
                        Err(e) => log::warn!("Failed to save balance for {}: {}", balance.ticker, e),
                    }
                }

                // Update progress to 90%
                if let Some(mut metadata) = self.cache.get_cache_metadata(device_id).await {
                    metadata.frontload_progress = 90;
                    let _ = self.cache.update_cache_metadata(&metadata).await;
                }

                // Fetch and cache staking positions if we have any cosmos/osmosis addresses
                if let Some(staking_positions) = self.fetch_staking_positions(pioneer_client, &xpubs).await? {
                    log::info!("🥩 Received {} staking positions", staking_positions.len());
                    for (network_id, positions) in staking_positions {
                        for position in positions {
                            // Convert staking position to portfolio balance format
                            if let Some(balance) = self.staking_position_to_balance(&position, &network_id) {
                                if let Err(e) = self.cache.save_portfolio_balance(&balance, device_id).await {
                                    log::warn!("Failed to save staking position for validator {}: {}", position.validator, e);
                                } else {
                                    saved_count += 1;
                                }
                            }
                        }
                    }
                }

                // Update progress to 95%
                if let Some(mut metadata) = self.cache.get_cache_metadata(device_id).await {
                    metadata.frontload_progress = 95;
                    let _ = self.cache.update_cache_metadata(&metadata).await;
                }

                // Build and cache dashboard data
                if let Err(e) = self.build_and_cache_dashboard(device_id, pioneer_client, &xpubs).await {
                    log::warn!("Failed to build dashboard cache: {}", e);
                }

                log::info!("💾 Successfully cached {} portfolio entries", saved_count);
                Ok(saved_count)
            }
            Err(e) => {
                log::error!("❌ Failed to fetch portfolio from Pioneer API: {}", e);
                Err(e)
            }
        }
    }

    /// Collect all xpubs for a device from the cache with blockchain info
    async fn collect_device_xpubs(&self, device_id: &str) -> Result<Vec<(String, String)>> {
        let db = self.cache.db.lock().await;
        
        let mut stmt = db.prepare(
            "SELECT DISTINCT xpub, coin_name FROM cached_pubkeys 
             WHERE device_id = ?1 AND xpub IS NOT NULL AND xpub != ''"
        )?;
        
        let xpubs = stmt.query_map([device_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(xpubs)
    }

    /// Build CAIP for a blockchain using cached asset data
    async fn build_caip_for_xpub(&self, blockchain: &str) -> Option<String> {
        // Get the primary network for this blockchain from cached assets
        match self.cache.get_blockchain_assets(blockchain).await {
            Ok(assets) => {
                // Find the native asset for this blockchain
                if let Some(native_asset) = assets.iter().find(|a| a.is_native) {
                    Some(native_asset.caip.clone())
                } else if !assets.is_empty() {
                    // Fallback to first asset if no native found
                    Some(assets[0].caip.clone())
                } else {
                    log::warn!("No assets found for blockchain: {}", blockchain);
                    None
                }
            }
            Err(e) => {
                log::warn!("Failed to get assets for blockchain {}: {}", blockchain, e);
                None
            }
        }
    }

    // REMOVED: fetch_staking_positions function with fake placeholder address
    // This function violated the "NEVER MOCK ANYTHING" rule by using 
    // a fake cosmos address "cosmos1placeholder". Real Cosmos address 
    // derivation should be implemented from actual xpubs, not fake addresses.

    /// Convert staking position to portfolio balance format
    fn staking_position_to_balance(&self, position: &crate::pioneer_api::StakingPosition, network_id: &str) -> Option<crate::pioneer_api::PortfolioBalance> {
        // Derive CAIP and ticker from network
        let (caip, ticker) = match network_id {
            "cosmos:cosmoshub-4" => ("cosmos:cosmoshub-4/slip44:118".to_string(), "ATOM".to_string()),
            "cosmos:osmosis-1" => ("cosmos:osmosis-1/slip44:118".to_string(), "OSMO".to_string()),
            _ => return None,
        };

        Some(crate::pioneer_api::PortfolioBalance {
            caip,
            ticker: ticker.clone(),
            balance: position.amount.clone(),
            value_usd: "0".to_string(), // Would be calculated from price * amount
            price_usd: Some("0".to_string()), // Would need price lookup
            network_id: network_id.to_string(),
            address: None,
            balance_type: Some("staking".to_string()),
            name: Some(format!("Staked {}", ticker)),
            icon: None,
            precision: Some(6), // Standard cosmos precision
            contract: None,
            validator: Some(position.validator.clone()),
            unbonding_end: position.unbonding_end,
            rewards_available: Some(position.rewards.clone()),
        })
    }

    /// Build and cache dashboard data
    async fn build_and_cache_dashboard(&self, device_id: &str, client: &PioneerClient, xpubs: &[(String, String)]) -> Result<()> {
        let xpub_refs: Vec<&str> = xpubs.iter().map(|(s, _)| s.as_str()).collect();
        match client.build_portfolio(xpub_refs).await {
            Ok(dashboard) => {
                if let Err(e) = self.cache.update_dashboard(device_id, &dashboard).await {
                    log::warn!("Failed to cache dashboard for device {}: {}", device_id, e);
                }
                Ok(())
            }
            Err(e) => {
                log::warn!("Failed to build portfolio dashboard: {}", e);
                Err(e)
            }
        }
    }

    /// Find the pubkey that corresponds to a specific balance
    async fn find_pubkey_for_balance(&self, device_id: &str, balance: &crate::pioneer_api::PortfolioBalance) -> Option<String> {
        // Use the cache manager's method to find matching pubkey
        self.cache.find_matching_pubkey(device_id, &balance.network_id, balance.address.as_deref()).await
    }
} 