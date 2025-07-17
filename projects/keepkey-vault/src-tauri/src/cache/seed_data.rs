// Load asset and path data from JSON files extracted from pioneer-discovery
use super::{CacheManager, CachedAsset, CachedPath};
use anyhow::Result;
use serde_json;
use std::collections::HashMap;

impl CacheManager {
    /// Load assets from the bundled assets.json file
    pub async fn load_assets_from_json(&self) -> Result<()> {
        let assets_json = include_str!("../data/assets.json");
        let asset_map: HashMap<String, serde_json::Value> = serde_json::from_str(assets_json)?;
        
        log::info!("📊 Loading {} assets from pioneer-discovery data...", asset_map.len());
        
        let mut loaded_count = 0;
        for (caip, asset_data) in asset_map {
            // Convert JSON to CachedAsset
            let asset = CachedAsset {
                caip: caip.clone(),
                network_id: asset_data["networkId"].as_str().unwrap_or("").to_string(),
                chain_id: asset_data["chainId"].as_str().map(|s| s.to_string()),
                symbol: asset_data["symbol"].as_str().unwrap_or("").to_string(),
                name: asset_data["name"].as_str().unwrap_or("").to_string(),
                asset_type: asset_data["assetType"].as_str().unwrap_or("native").to_string(),
                is_native: asset_data["isNative"].as_bool().unwrap_or(false),
                contract_address: asset_data["contractAddress"].as_str().map(|s| s.to_string()),
                icon: asset_data["icon"].as_str().map(|s| s.to_string()),
                color: asset_data["color"].as_str().map(|s| s.to_string()),
                decimals: asset_data["decimals"].as_i64().map(|i| i as i32),
                precision: asset_data["precision"].as_i64().map(|i| i as i32),
                network_name: asset_data["networkName"].as_str().map(|s| s.to_string()),
                explorer: asset_data["explorer"].as_str().map(|s| s.to_string()),
                explorer_address_link: asset_data["explorerAddressLink"].as_str().map(|s| s.to_string()),
                explorer_tx_link: asset_data["explorerTxLink"].as_str().map(|s| s.to_string()),
                coin_gecko_id: asset_data["coinGeckoId"].as_str().map(|s| s.to_string()),
                tags: asset_data["tags"].as_array().map(|arr| {
                    arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect()
                }),
            };
            
            if let Err(e) = self.save_asset(&asset).await {
                log::warn!("Failed to save asset {}: {}", caip, e);
            } else {
                loaded_count += 1;
                
                // Log progress for large datasets
                if loaded_count % 1000 == 0 {
                    log::info!("  ... loaded {} assets", loaded_count);
                }
            }
        }
        
        log::info!("✅ Loaded {} assets from JSON", loaded_count);
        Ok(())
    }
    
    /// Load paths from the bundled paths.json file
    pub async fn load_paths_from_json(&self) -> Result<()> {
        let paths_json = include_str!("../data/paths.json");
        let paths_by_blockchain: HashMap<String, Vec<serde_json::Value>> = serde_json::from_str(paths_json)?;
        
        log::info!("📂 Loading derivation paths for {} blockchains...", paths_by_blockchain.len());
        
        let mut total_loaded = 0;
        for (blockchain, paths) in paths_by_blockchain {
            log::info!("  Loading {} paths for {}", paths.len(), blockchain);
            
            for path_data in paths {
                let path = CachedPath {
                    path_id: path_data["id"].as_str().unwrap_or("").to_string(),
                    note: path_data["note"].as_str().map(|s| s.to_string()),
                    blockchain: path_data["blockchain"].as_str().unwrap_or("").to_string(),
                    symbol: path_data["symbol"].as_str().unwrap_or("").to_string(),
                    networks: path_data["networks"].as_array()
                        .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect())
                        .unwrap_or_default(),
                    script_type: path_data["script_type"].as_str().map(|s| s.to_string()),
                    address_n_list: path_data["addressNList"].as_array()
                        .map(|arr| arr.iter().filter_map(|v| v.as_i64()).map(|i| i as u32).collect())
                        .unwrap_or_default(),
                    address_n_list_master: path_data["addressNListMaster"].as_array()
                        .map(|arr| arr.iter().filter_map(|v| v.as_i64()).map(|i| i as u32).collect())
                        .unwrap_or_default(),
                    curve: path_data["curve"].as_str().unwrap_or("secp256k1").to_string(),
                    show_display: path_data["showDisplay"].as_bool().unwrap_or(false),
                    is_default: false, // Will be determined by logic later
                };
                
                if let Err(e) = self.save_path(&path).await {
                    log::warn!("Failed to save path {}: {}", path.path_id, e);
                } else {
                    total_loaded += 1;
                }
            }
        }
        
        log::info!("✅ Loaded {} derivation paths", total_loaded);
        Ok(())
    }
    
    /// Initialize cache with data from JSON files if empty
    pub async fn init_from_json_data(&self) -> Result<()> {
        // Reset and recreate asset tables to ensure clean state
        self.reset_asset_tables().await?;
        
        log::info!("🌱 Initializing cache from pioneer-discovery JSON data...");
        
        // Load asset data
        self.load_assets_from_json().await?;
        
        // Load path data
        self.load_paths_from_json().await?;
        
        log::info!("✅ Cache initialization from JSON data complete");
        Ok(())
    }
    
    /// Quick check if cache has been initialized
    pub async fn is_cache_initialized(&self) -> Result<bool> {
        let db = self.db.lock().await;
        
        // Check if we have any assets
        let asset_count: i64 = db.query_row(
            "SELECT COUNT(*) FROM assets",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        // Check if we have any paths
        let path_count: i64 = db.query_row(
            "SELECT COUNT(*) FROM derivation_paths",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        Ok(asset_count > 0 && path_count > 0)
    }
    
    /// Get cache initialization stats
    pub async fn get_cache_stats(&self) -> Result<(i64, i64, i64)> {
        let db = self.db.lock().await;
        
        let asset_count: i64 = db.query_row(
            "SELECT COUNT(*) FROM assets",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        let path_count: i64 = db.query_row(
            "SELECT COUNT(*) FROM derivation_paths",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        let network_count: i64 = db.query_row(
            "SELECT COUNT(DISTINCT network_id) FROM assets",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        Ok((asset_count, path_count, network_count))
    }
} 