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

impl CacheManager {
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
    
    /// Save a cached asset
    pub async fn save_asset(&self, asset: &CachedAsset) -> Result<()> {
        let db = self.db.lock().await;
        
        let tags_json = asset.tags.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default());
        
        db.execute(
            "INSERT OR REPLACE INTO assets 
             (caip, network_id, chain_id, symbol, name, asset_type, is_native, 
              contract_address, icon, color, decimals, precision, network_name,
              explorer, explorer_address_link, explorer_tx_link, coin_gecko_id, tags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                asset.caip,
                asset.network_id,
                asset.chain_id,
                asset.symbol,
                asset.name,
                asset.asset_type,
                asset.is_native,
                asset.contract_address,
                asset.icon,
                asset.color,
                asset.decimals,
                asset.precision,
                asset.network_name,
                asset.explorer,
                asset.explorer_address_link,
                asset.explorer_tx_link,
                asset.coin_gecko_id,
                tags_json
            ],
        )?;
        
        Ok(())
    }
    
    /// Get asset by CAIP
    pub async fn get_asset(&self, caip: &str) -> Result<Option<CachedAsset>> {
        let db = self.db.lock().await;
        
        let result = db.query_row(
            "SELECT caip, network_id, chain_id, symbol, name, asset_type, is_native,
                    contract_address, icon, color, decimals, precision, network_name,
                    explorer, explorer_address_link, explorer_tx_link, coin_gecko_id, tags
             FROM assets WHERE caip = ?1",
            params![caip],
            |row| {
                let tags_json: Option<String> = row.get(17)?;
                let tags = tags_json.and_then(|t| serde_json::from_str(&t).ok());
                
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
        
        Ok(result)
    }
    
    /// Get all assets for a network
    pub async fn get_network_assets(&self, network_id: &str) -> Result<Vec<CachedAsset>> {
        let db = self.db.lock().await;
        
        let mut stmt = db.prepare(
            "SELECT caip, network_id, chain_id, symbol, name, asset_type, is_native,
                    contract_address, icon, color, decimals, precision, network_name,
                    explorer, explorer_address_link, explorer_tx_link, coin_gecko_id, tags
             FROM assets WHERE network_id = ?1"
        )?;
        
        let assets = stmt.query_map(params![network_id], |row| {
            let tags_json: Option<String> = row.get(17)?;
            let tags = tags_json.and_then(|t| serde_json::from_str(&t).ok());
            
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

    /// Get all assets for a blockchain (by searching network_id patterns)
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
            let tags = tags_json.and_then(|t| serde_json::from_str(&t).ok());
            
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
        
        // For specific blockchains, filter further by symbol or other criteria
        let filtered_assets = match blockchain {
            "bitcoin" => assets.into_iter().filter(|a| a.symbol.to_lowercase() == "btc").collect(),
            "litecoin" => assets.into_iter().filter(|a| a.symbol.to_lowercase() == "ltc").collect(),
            "dogecoin" => assets.into_iter().filter(|a| a.symbol.to_lowercase() == "doge").collect(),
            _ => assets,
        };
        
        Ok(filtered_assets)
    }
    
    /// Save a derivation path
    pub async fn save_path(&self, path: &CachedPath) -> Result<()> {
        let db = self.db.lock().await;
        
        let networks_json = serde_json::to_string(&path.networks)?;
        let address_n_list_json = serde_json::to_string(&path.address_n_list)?;
        let address_n_list_master_json = serde_json::to_string(&path.address_n_list_master)?;
        
        db.execute(
            "INSERT OR REPLACE INTO derivation_paths 
             (path_id, note, blockchain, symbol, networks, script_type,
              address_n_list, address_n_list_master, curve, show_display, is_default)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                path.path_id,
                path.note,
                path.blockchain,
                path.symbol,
                networks_json,
                path.script_type,
                address_n_list_json,
                address_n_list_master_json,
                path.curve,
                path.show_display,
                path.is_default
            ],
        )?;
        
        Ok(())
    }
    
    /// Get paths for a blockchain
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
             FROM derivation_paths"
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
    
    /// Build CAIP from blockchain and path info
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