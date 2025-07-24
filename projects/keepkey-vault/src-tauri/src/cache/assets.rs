// Asset cache management
use super::CacheManager;
use anyhow::{Result, anyhow};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAsset {
    pub caip: String,
    pub network_id: String,
    pub chain_id: Option<String>,
    pub symbol: String,
    pub name: String,
    pub asset_type: String, // "native", "token", "nft"
    pub is_native: bool,
    pub contract_address: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub decimals: Option<i32>,
    pub precision: Option<i32>,
    pub network_name: Option<String>,
    pub explorer: Option<String>,
    pub explorer_address_link: Option<String>,
    pub explorer_tx_link: Option<String>,
    pub coin_gecko_id: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedPath {
    pub path_id: String,
    pub note: Option<String>,
    pub blockchain: String,
    pub symbol: String,
    pub networks: Vec<String>,
    pub script_type: Option<String>,
    pub address_n_list: Vec<u32>,
    pub address_n_list_master: Vec<u32>,
    pub curve: String,
    pub show_display: bool,
    pub is_default: bool,
}

/// Blockchain configuration from blockchains.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainConfig {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub network_id: String,
    pub enabled: bool,
    #[serde(rename = "type")]
    pub chain_type: String,
    pub slip44: u32,
    pub derivation_path: String,
    pub native_asset: NativeAsset,
    pub rpc_urls: Vec<String>,
    pub explorer_url: String,
    pub supports_tokens: Option<bool>,
    pub supports_eip1559: Option<bool>,
    pub supports_staking: Option<bool>,
    pub supports_rbf: Option<bool>,
    pub supports_memo: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeAsset {
    pub caip: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainsData {
    pub version: String,
    pub description: String,
    pub blockchains: Vec<BlockchainConfig>,
    pub metadata: BlockchainMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainMetadata {
    pub total_blockchains: u32,
    pub evm_chains: u32,
    pub utxo_chains: u32,
    pub cosmos_chains: u32,
    pub other_chains: u32,
    pub enabled_by_default: u32,
}

impl CacheManager {
    /// Load enabled blockchains from blockchains.json configuration
    pub async fn load_enabled_blockchains(&self) -> Result<Vec<BlockchainConfig>> {
        log::info!("📋 Loading blockchain configuration from blockchains.json...");
        
        let blockchains_json = include_str!("../data/blockchains.json");
        let blockchains_data: BlockchainsData = serde_json::from_str(blockchains_json)
            .map_err(|e| anyhow!("Failed to parse blockchains.json: {}", e))?;
        
        let enabled_blockchains: Vec<BlockchainConfig> = blockchains_data.blockchains
            .into_iter()
            .filter(|bc| bc.enabled)
            .collect();
        
        log::info!("✅ Loaded {} enabled blockchains from configuration", enabled_blockchains.len());
        
        // Log the enabled blockchains for debugging
        for blockchain in &enabled_blockchains {
            log::info!("  🔗 {} ({}) - {} - enabled", 
                blockchain.name, blockchain.symbol, blockchain.network_id);
        }
        
        Ok(enabled_blockchains)
    }
    
    /// Get network IDs for enabled blockchains (used for Pioneer API calls)
    pub async fn get_enabled_network_ids(&self) -> Result<Vec<String>> {
        let blockchains = self.load_enabled_blockchains().await?;
        let network_ids: Vec<String> = blockchains.iter()
            .map(|bc| bc.network_id.clone())
            .collect();
        
        log::info!("📊 Extracted {} network IDs from blockchain configuration", network_ids.len());
        Ok(network_ids)
    }
    
    /// Get EVM networks from blockchain configuration  
    pub async fn get_evm_networks(&self) -> Result<Vec<String>> {
        let blockchains = self.load_enabled_blockchains().await?;
        let evm_caips: Vec<String> = blockchains.iter()
            .filter(|bc| bc.chain_type == "evm")
            .map(|bc| bc.native_asset.caip.clone())
            .collect();
        
        log::info!("📊 Found {} EVM networks in blockchain configuration", evm_caips.len());
        Ok(evm_caips)
    }
    
    /// Get blockchain configuration by network ID
    pub async fn get_blockchain_by_network_id(&self, network_id: &str) -> Result<Option<BlockchainConfig>> {
        let blockchains = self.load_enabled_blockchains().await?;
        Ok(blockchains.into_iter().find(|bc| bc.network_id == network_id))
    }

    /// Drop and recreate asset-related tables
    pub async fn reset_asset_tables(&self) -> Result<()> {
        let db = self.db.lock().await;
        
        // Drop existing tables
        db.execute_batch("
            DROP TABLE IF EXISTS path_asset_mapping;
            DROP TABLE IF EXISTS derivation_paths;
            DROP TABLE IF EXISTS networks;
            DROP TABLE IF EXISTS assets;
            DROP VIEW IF EXISTS v_assets_with_networks;
            DROP VIEW IF EXISTS v_paths_with_assets;
        ")?;
        
        // Recreate tables with simpler schema
        db.execute_batch("
            CREATE TABLE assets (
                caip TEXT PRIMARY KEY,
                network_id TEXT NOT NULL,
                chain_id TEXT,
                symbol TEXT NOT NULL,
                name TEXT NOT NULL,
                asset_type TEXT DEFAULT 'native',
                is_native INTEGER DEFAULT 0,
                contract_address TEXT,
                icon TEXT,
                color TEXT,
                decimals INTEGER,
                precision INTEGER,
                network_name TEXT,
                explorer TEXT,
                explorer_address_link TEXT,
                explorer_tx_link TEXT,
                coin_gecko_id TEXT,
                tags TEXT,
                created_at INTEGER DEFAULT (strftime('%s', 'now')),
                last_updated INTEGER DEFAULT (strftime('%s', 'now'))
            );
            
            CREATE TABLE derivation_paths (
                path_id TEXT PRIMARY KEY,
                note TEXT,
                blockchain TEXT NOT NULL,
                symbol TEXT NOT NULL,
                networks TEXT NOT NULL,
                script_type TEXT,
                address_n_list TEXT NOT NULL,
                address_n_list_master TEXT NOT NULL,
                curve TEXT DEFAULT 'secp256k1',
                show_display INTEGER DEFAULT 0,
                is_default INTEGER DEFAULT 0,
                created_at INTEGER DEFAULT (strftime('%s', 'now'))
            );
            
            CREATE TABLE networks (
                network_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                chain_id TEXT,
                network_type TEXT,
                native_asset_caip TEXT,
                native_symbol TEXT,
                explorer_url TEXT,
                is_testnet INTEGER DEFAULT 0,
                created_at INTEGER DEFAULT (strftime('%s', 'now'))
            );
            
            CREATE INDEX idx_assets_network ON assets(network_id);
            CREATE INDEX idx_assets_symbol ON assets(symbol);
            CREATE INDEX idx_paths_blockchain ON derivation_paths(blockchain);
        ")?;
        
        Ok(())
    }

    /// Save an asset to the cache
    pub async fn save_asset(&self, asset: &CachedAsset) -> Result<()> {
        let db = self.db.lock().await;
        
        let tags_json = match &asset.tags {
            Some(tags) => serde_json::to_string(tags).unwrap_or_default(),
            None => String::new(),
        };
        
        db.execute(
            "INSERT OR REPLACE INTO assets (
                caip, network_id, chain_id, symbol, name, asset_type, is_native,
                contract_address, icon, color, decimals, precision, network_name,
                explorer, explorer_address_link, explorer_tx_link, coin_gecko_id, tags
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                asset.caip, asset.network_id, asset.chain_id, asset.symbol, asset.name,
                asset.asset_type, asset.is_native, asset.contract_address, asset.icon,
                asset.color, asset.decimals, asset.precision, asset.network_name,
                asset.explorer, asset.explorer_address_link, asset.explorer_tx_link,
                asset.coin_gecko_id, tags_json
            ],
        )?;
        
        Ok(())
    }
    
    /// Save a derivation path to the cache
    pub async fn save_path(&self, path: &CachedPath) -> Result<()> {
        let db = self.db.lock().await;
        
        let networks_json = serde_json::to_string(&path.networks)?;
        let address_n_list_json = serde_json::to_string(&path.address_n_list)?;
        let address_n_list_master_json = serde_json::to_string(&path.address_n_list_master)?;
        
        db.execute(
            "INSERT OR REPLACE INTO derivation_paths (
                path_id, note, blockchain, symbol, networks, script_type,
                address_n_list, address_n_list_master, curve, show_display, is_default
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                path.path_id, path.note, path.blockchain, path.symbol, networks_json,
                path.script_type, address_n_list_json, address_n_list_master_json,
                path.curve, path.show_display, path.is_default
            ],
        )?;
        
        Ok(())
    }
    
    /// Get an asset by CAIP
    pub async fn get_asset(&self, caip: &str) -> Result<Option<CachedAsset>> {
        let db = self.db.lock().await;
        
        let asset = db.query_row(
            "SELECT caip, network_id, chain_id, symbol, name, asset_type, is_native,
                    contract_address, icon, color, decimals, precision, network_name,
                    explorer, explorer_address_link, explorer_tx_link, coin_gecko_id, tags
             FROM assets WHERE caip = ?1",
            [caip],
            |row| {
                let tags_json: Option<String> = row.get(17)?;
                let tags = tags_json.and_then(|json| serde_json::from_str(&json).ok());
                
                Ok(CachedAsset {
                    caip: row.get(0)?,
                    network_id: row.get(1)?,
                    chain_id: row.get(2)?,
                    symbol: row.get(3)?,
                    name: row.get(4)?,
                    asset_type: row.get(5)?,
                    is_native: row.get(6)?,
                    contract_address: row.get(7)?,
                    icon: row.get(8)?,
                    color: row.get(9)?,
                    decimals: row.get(10)?,
                    precision: row.get(11)?,
                    network_name: row.get(12)?,
                    explorer: row.get(13)?,
                    explorer_address_link: row.get(14)?,
                    explorer_tx_link: row.get(15)?,
                    coin_gecko_id: row.get(16)?,
                    tags,
                })
            },
        ).optional()?;
        
        Ok(asset)
    }
    
    /// Get assets for a blockchain
    pub async fn get_blockchain_assets(&self, blockchain: &str) -> Result<Vec<CachedAsset>> {
        let db = self.db.lock().await;
        
        // Map blockchain to network_id patterns
        let network_pattern = match blockchain {
            "bitcoin" => "bip122:%",
            "ethereum" => "eip155:%",
            "cosmos" => "cosmos:%",
            "osmosis" => "cosmos:osmosis%",
            "thorchain" => "thorchain:%", 
            "mayachain" => "mayachain:%",
            "litecoin" => "bip122:%",
            "dogecoin" => "bip122:%",
            "ripple" => "xrpl:%",
            _ => {
                log::warn!("Unknown blockchain pattern for: {}", blockchain);
                return Ok(Vec::new());
            }
        };
        
        let mut stmt = db.prepare(
            "SELECT caip, network_id, chain_id, symbol, name, asset_type, is_native,
                    contract_address, icon, color, decimals, precision, network_name,
                    explorer, explorer_address_link, explorer_tx_link, coin_gecko_id, tags
             FROM assets WHERE network_id LIKE ?1"
        )?;
        
        let assets = stmt.query_map(params![network_pattern], |row| {
            let tags_json: Option<String> = row.get(17)?;
            let tags = tags_json.and_then(|json| serde_json::from_str(&json).ok());
            
            Ok(CachedAsset {
                caip: row.get(0)?,
                network_id: row.get(1)?,
                chain_id: row.get(2)?,
                symbol: row.get(3)?,
                name: row.get(4)?,
                asset_type: row.get(5)?,
                is_native: row.get(6)?,
                contract_address: row.get(7)?,
                icon: row.get(8)?,
                color: row.get(9)?,
                decimals: row.get(10)?,
                precision: row.get(11)?,
                network_name: row.get(12)?,
                explorer: row.get(13)?,
                explorer_address_link: row.get(14)?,
                explorer_tx_link: row.get(15)?,
                coin_gecko_id: row.get(16)?,
                tags,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(assets)
    }
    
    /// Get all assets
    pub async fn get_all_assets(&self) -> Result<Vec<CachedAsset>> {
        let db = self.db.lock().await;
        
        let mut stmt = db.prepare(
            "SELECT caip, network_id, chain_id, symbol, name, asset_type, is_native,
                    contract_address, icon, color, decimals, precision, network_name,
                    explorer, explorer_address_link, explorer_tx_link, coin_gecko_id, tags
             FROM assets ORDER BY symbol"
        )?;
        
        let assets = stmt.query_map([], |row| {
            let tags_json: Option<String> = row.get(17)?;
            let tags = tags_json.and_then(|json| serde_json::from_str(&json).ok());
            
            Ok(CachedAsset {
                caip: row.get(0)?,
                network_id: row.get(1)?,
                chain_id: row.get(2)?,
                symbol: row.get(3)?,
                name: row.get(4)?,
                asset_type: row.get(5)?,
                is_native: row.get(6)?,
                contract_address: row.get(7)?,
                icon: row.get(8)?,
                color: row.get(9)?,
                decimals: row.get(10)?,
                precision: row.get(11)?,
                network_name: row.get(12)?,
                explorer: row.get(13)?,
                explorer_address_link: row.get(14)?,
                explorer_tx_link: row.get(15)?,
                coin_gecko_id: row.get(16)?,
                tags,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(assets)
    }
    
    /// Get derivation paths for a blockchain
    pub async fn get_blockchain_paths(&self, blockchain: &str) -> Result<Vec<CachedPath>> {
        let db = self.db.lock().await;
        
        let mut stmt = db.prepare(
            "SELECT path_id, note, blockchain, symbol, networks, script_type,
                    address_n_list, address_n_list_master, curve, show_display, is_default
             FROM derivation_paths WHERE blockchain = ?1"
        )?;
        
        let paths = stmt.query_map(params![blockchain], |row| {
            let networks_json: String = row.get(4)?;
            let address_n_list_json: String = row.get(6)?;
            let address_n_list_master_json: String = row.get(7)?;
            
            let networks = serde_json::from_str(&networks_json).unwrap_or_default();
            let address_n_list = serde_json::from_str(&address_n_list_json).unwrap_or_default();
            let address_n_list_master = serde_json::from_str(&address_n_list_master_json).unwrap_or_default();
            
            Ok(CachedPath {
                path_id: row.get(0)?,
                note: row.get(1)?,
                blockchain: row.get(2)?,
                symbol: row.get(3)?,
                networks,
                script_type: row.get(5)?,
                address_n_list,
                address_n_list_master,
                curve: row.get(8)?,
                show_display: row.get(9)?,
                is_default: row.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(paths)
    }
    
    /// Get all derivation paths
    pub async fn get_all_paths(&self) -> Result<Vec<CachedPath>> {
        let db = self.db.lock().await;
        
        let mut stmt = db.prepare(
            "SELECT path_id, note, blockchain, symbol, networks, script_type,
                    address_n_list, address_n_list_master, curve, show_display, is_default
             FROM derivation_paths ORDER BY blockchain, path_id"
        )?;
        
        let paths = stmt.query_map([], |row| {
            let networks_json: String = row.get(4)?;
            let address_n_list_json: String = row.get(6)?;
            let address_n_list_master_json: String = row.get(7)?;
            
            let networks = serde_json::from_str(&networks_json).unwrap_or_default();
            let address_n_list = serde_json::from_str(&address_n_list_json).unwrap_or_default();
            let address_n_list_master = serde_json::from_str(&address_n_list_master_json).unwrap_or_default();
            
            Ok(CachedPath {
                path_id: row.get(0)?,
                note: row.get(1)?,
                blockchain: row.get(2)?,
                symbol: row.get(3)?,
                networks,
                script_type: row.get(5)?,
                address_n_list,
                address_n_list_master,
                curve: row.get(8)?,
                show_display: row.get(9)?,
                is_default: row.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(paths)
    }
    
    /// Build CAIP identifier for a path
    pub fn build_caip_for_path(&self, blockchain: &str, network_id: &str) -> String {
        match blockchain {
            "bitcoin" => format!("{}/slip44:0", network_id),
            "ethereum" => format!("{}/slip44:60", network_id),
            "cosmos" => format!("{}/slip44:118", network_id),
            "osmosis" => format!("{}/slip44:118", network_id),
            "thorchain" => format!("{}/slip44:931", network_id),
            "mayachain" => format!("{}/slip44:931", network_id),
            "dogecoin" => format!("{}/slip44:3", network_id),
            "litecoin" => format!("{}/slip44:2", network_id),
            _ => format!("{}/slip44:0", network_id), // Default fallback
        }
    }
} 