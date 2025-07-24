// Portfolio cache management extensions
use super::CacheManager;
use crate::pioneer_api::{PortfolioBalance, Dashboard, NetworkSummary, AssetSummary};
use anyhow::{Result, anyhow};
use rusqlite::{params, OptionalExtension};
use serde_json;

impl CacheManager {
    /// Save portfolio balance to cache with pubkey linkage
    pub async fn save_portfolio_balance(&self, balance: &PortfolioBalance, device_id: &str) -> Result<()> {
        self.save_portfolio_balance_with_pubkey(balance, device_id, None).await
    }

    /// Save portfolio balance with optional pubkey
    pub async fn save_portfolio_balance_with_pubkey(&self, balance: &PortfolioBalance, device_id: &str, pubkey: Option<&str>) -> Result<()> {
        let db = self.db.lock().await;
        
        // Use provided pubkey or the one from balance
        let final_pubkey = pubkey.unwrap_or(&balance.pubkey);
        
        // Enhanced balance type detection based on field presence
        let balance_type = if balance.validator.is_some() {
            "delegation"
        } else if balance.unbonding_end.is_some() {
            "unbonding"
        } else {
            "balance"
        };
        
        // Create a unique balance key to prevent logical duplicates
        let balance_key = format!("{}:{}:{}:{}:{}", 
            balance.caip,
            balance.address.as_deref().unwrap_or("no-address"),
            balance.balance,
            balance_type,
            balance.ticker.as_deref().unwrap_or("UNKNOWN")
        );
        
        // Enhanced duplicate detection - check both exact matches and logical duplicates
        let exists: bool = db.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM portfolio_balances 
                WHERE device_id = ?1 
                AND (
                    -- Exact duplicate check
                    (pubkey = ?2 AND caip = ?3 AND COALESCE(address, '') = COALESCE(?4, '') 
                     AND type = ?5 AND balance = ?6 AND ABS(COALESCE(balance_usd, 0) - COALESCE(?7, 0)) < 0.01)
                    OR
                    -- Logical duplicate check (same asset/balance for device regardless of pubkey)
                    (device_id = ?1 AND caip = ?3 AND COALESCE(address, '') = COALESCE(?4, '') 
                     AND type = ?5 AND balance = ?6 AND ticker = ?8)
                )
                LIMIT 1
            )",
            params![
                device_id,
                final_pubkey,
                balance.caip,
                balance.address,
                balance_type,
                balance.balance,
                balance.value_usd.parse::<f64>().unwrap_or(0.0),
                balance.ticker.as_ref().unwrap_or(&"UNKNOWN".to_string()),
            ],
            |row| row.get(0)
        ).unwrap_or(false);
        
        if exists {
            log::debug!("⏭️ Skipping duplicate balance: {}", balance_key);
            return Ok(());
        }
        
        db.execute(
            "INSERT OR REPLACE INTO portfolio_balances 
             (device_id, caip, ticker, balance, balance_usd, price_usd, network_id, 
              address, type, name, icon, precision, contract, validator, 
              unbonding_end, rewards_available, pubkey, last_updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                device_id,
                balance.caip,
                balance.ticker.as_ref().unwrap_or(&"UNKNOWN".to_string()),
                balance.balance,
                balance.value_usd,  // This is balance_usd in the database
                balance.price_usd,
                balance.network_id.as_ref().unwrap_or(&"unknown".to_string()),
                balance.address,
                balance_type,
                balance.name,
                balance.icon,
                balance.precision,
                balance.contract,
                balance.validator,
                balance.unbonding_end,
                balance.rewards_available,
                final_pubkey,
                chrono::Utc::now().timestamp(),
            ],
        )?;
        
        Ok(())
    }

    /// Find matching pubkey for a balance entry
    pub async fn find_matching_pubkey(&self, device_id: &str, network_id: &str, address: Option<&str>) -> Option<String> {
        // Try to find a cached pubkey that matches this network/address
        let result = match address {
            Some(addr) => {
                // Look for exact address match first
                self.db.lock().await.query_row(
                    "SELECT xpub FROM cached_pubkeys 
                     WHERE device_id = ?1 AND address = ?2 AND xpub IS NOT NULL
                     LIMIT 1",
                    params![device_id, addr],
                    |row| row.get::<_, String>(0),
                ).ok()
            }
            None => None,
        };

        if result.is_some() {
            return result;
        }

        // Fallback: find any xpub for this network/coin
        let coin_name = self.network_id_to_coin_name(network_id);
        self.db.lock().await.query_row(
            "SELECT xpub FROM cached_pubkeys 
             WHERE device_id = ?1 AND coin_name = ?2 AND xpub IS NOT NULL
             LIMIT 1",
            params![device_id, coin_name],
            |row| row.get::<_, String>(0),
        ).ok()
    }

    /// Clean up duplicate portfolio balances
    pub async fn clean_duplicate_portfolio_balances(&self) -> Result<usize> {
        let db = self.db.lock().await;
        
        // First, identify and log duplicates
        let duplicates: Vec<(String, String, String, String, String, String, String)> = db
            .prepare(
                "SELECT device_id, pubkey, caip, COALESCE(address, '') as addr, type, balance, balance_usd
                 FROM portfolio_balances
                 GROUP BY device_id, pubkey, caip, addr, type, balance, balance_usd
                 HAVING COUNT(*) > 1"
            )?
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        
        if duplicates.is_empty() {
            log::info!("✅ No duplicate portfolio balances found");
            return Ok(0);
        }
        
        log::warn!("⚠️ Found {} duplicate portfolio balance entries, cleaning up...", duplicates.len());
        
        // Delete duplicates, keeping only one of each
        let mut total_deleted = 0;
        for (device_id, pubkey, caip, address, balance_type, balance, balance_usd) in duplicates {
            // Delete all but the most recent one
            let deleted = db.execute(
                "DELETE FROM portfolio_balances 
                 WHERE device_id = ?1 
                 AND pubkey = ?2 
                 AND caip = ?3 
                 AND COALESCE(address, '') = ?4
                 AND type = ?5
                 AND balance = ?6
                 AND balance_usd = ?7
                 AND id NOT IN (
                     SELECT MAX(id) 
                     FROM portfolio_balances 
                     WHERE device_id = ?1 
                     AND pubkey = ?2 
                     AND caip = ?3 
                     AND COALESCE(address, '') = ?4
                     AND type = ?5
                     AND balance = ?6
                     AND balance_usd = ?7
                 )",
                params![device_id, pubkey, caip, address, balance_type, balance, balance_usd],
            )?;
            
            if deleted > 0 {
                log::debug!("🧹 Deleted {} duplicate entries for {} - {}", deleted, caip, balance);
                total_deleted += deleted;
            }
        }
        
        log::info!("✅ Cleaned up {} duplicate portfolio balance entries", total_deleted);
        Ok(total_deleted)
    }
    
    /// Convert network ID to coin name for pubkey lookup
    fn network_id_to_coin_name(&self, network_id: &str) -> &str {
        match network_id {
            "eip155:1" => "ethereum",
            "bip122:000000000019d6689c085ae165831e93" => "bitcoin", // Bitcoin mainnet
            "cosmos:cosmoshub-4" => "cosmos",
            "cosmos:osmosis-1" => "osmosis",
            "thorchain:thorchain-mainnet-v1" => "thorchain",
            "mayachain:mayachain-mainnet-v1" => "mayachain",
            "bip122:12a765e31ffd4059bada1e25190f6e98" => "litecoin",
            "bip122:1a91e3dace36e2be3bf030a65679fe82" => "dogecoin",
            _ => "ethereum", // Default fallback
        }
    }
    
    /// Get all portfolio balances for a device
    pub async fn get_device_portfolio(&self, device_id: &str) -> Result<Vec<PortfolioBalance>> {
        let db = self.db.lock().await;
        
        let mut stmt = db.prepare(
            "SELECT caip, ticker, balance, balance_usd, price_usd, network_id, address, 
                    type, name, icon, precision, contract, validator, unbonding_end, 
                    rewards_available, pubkey
             FROM portfolio_balances 
             WHERE device_id = ?1 
             ORDER BY CAST(balance_usd AS REAL) DESC"
        )?;
        
        let balances = stmt.query_map([device_id], |row| {
            Ok(PortfolioBalance {
                caip: row.get(0)?,
                pubkey: row.get(15).unwrap_or_else(|_| "unknown".to_string()), // Get pubkey from database
                ticker: Some(row.get(1)?),
                balance: row.get(2)?,
                value_usd: row.get(3)?,  // Maps to balance_usd column
                price_usd: row.get(4)?,
                network_id: Some(row.get(5)?),
                address: row.get(6)?,
                balance_type: row.get(7)?,
                name: row.get(8)?,
                icon: row.get(9)?,
                precision: row.get(10)?,
                contract: row.get(11)?,
                validator: row.get(12)?,
                unbonding_end: row.get(13)?,
                rewards_available: row.get(14)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(balances)
    }
    
    /// Get combined portfolio across all devices
    pub async fn get_combined_portfolio(&self) -> Result<Vec<PortfolioBalance>> {
        let db = self.db.lock().await;
        
        let mut stmt = db.prepare(
            "SELECT caip, ticker, SUM(CAST(balance AS REAL)) as total_balance, 
                    SUM(CAST(balance_usd AS REAL)) as total_value_usd,
                    MAX(price_usd) as price_usd, network_id, 
                    MAX(name) as name, MAX(icon) as icon, MAX(precision) as precision,
                    MAX(contract) as contract, '' as pubkey
             FROM portfolio_balances 
             WHERE type = 'balance'
             GROUP BY caip, ticker, network_id
             ORDER BY total_value_usd DESC"
        )?;
        
        let balances = stmt.query_map([], |row| {
            Ok(PortfolioBalance {
                caip: row.get(0)?,
                pubkey: "unknown".to_string(), // Default pubkey for combined portfolio
                network_id: Some(row.get(4)?),
                ticker: Some(row.get(1)?),
                balance: row.get::<_, f64>(2)?.to_string(),
                value_usd: row.get::<_, f64>(3)?.to_string(),
                price_usd: row.get(4)?,
                address: None,
                balance_type: Some("balance".to_string()),
                name: row.get(5)?,
                icon: row.get(6)?,
                precision: row.get(7)?,
                contract: row.get(8)?,
                validator: None,
                unbonding_end: None,
                rewards_available: None,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(balances)
    }
    
    /// Update portfolio dashboard cache
    pub async fn update_dashboard(&self, device_id: &str, dashboard: &Dashboard) -> Result<()> {
        let db = self.db.lock().await;
        
        let now = chrono::Utc::now().timestamp();
        let networks_json = serde_json::to_string(&dashboard.networks)?;
        let assets_json = serde_json::to_string(&dashboard.assets)?;
        
        db.execute(
            "INSERT OR REPLACE INTO portfolio_dashboard 
             (device_id, total_value_usd, networks_json, assets_json, 
              total_assets, total_networks, last_updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                device_id,
                dashboard.total_value_usd.to_string(),
                networks_json,
                assets_json,
                dashboard.assets.len() as i64,
                dashboard.networks.len() as i64,
                now
            ],
        )?;
        
        Ok(())
    }
    
    /// Get cached dashboard for a device
    pub async fn get_dashboard(&self, device_id: &str) -> Result<Option<Dashboard>> {
        let db = self.db.lock().await;
        
        let result = db.query_row(
            "SELECT total_value_usd, networks_json, assets_json 
             FROM portfolio_dashboard 
             WHERE device_id = ?1",
            params![device_id],
            |row| {
                let total_value_usd: String = row.get(0)?;
                let networks_json: String = row.get(1)?;
                let assets_json: String = row.get(2)?;
                
                Ok((total_value_usd, networks_json, assets_json))
            },
        );
        
        let result = match result {
            Ok(data) => Some(data),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e.into()),
        };
        
        if let Some((total_value_usd, networks_json, assets_json)) = result {
            let total_value_usd: String = total_value_usd;
            let networks_json: String = networks_json;
            let assets_json: String = assets_json;
            let networks: Vec<NetworkSummary> = serde_json::from_str(&networks_json)?;
            let assets: Vec<AssetSummary> = serde_json::from_str(&assets_json)?;
            
            Ok(Some(Dashboard {
                total_value_usd: total_value_usd.parse()?,
                networks,
                assets,
            }))
        } else {
            Ok(None)
        }
    }
    
    /// Check if portfolio cache is stale (older than TTL minutes)
    pub async fn is_portfolio_stale(&self, device_id: &str, ttl_minutes: i64) -> Result<bool> {
        let db = self.db.lock().await;
        
        let now = chrono::Utc::now().timestamp();
        let ttl_seconds = ttl_minutes * 60;
        
        let count: i64 = db.query_row(
            "SELECT COUNT(*) FROM portfolio_dashboard 
             WHERE device_id = ?1 AND last_updated > ?2",
            params![device_id, now - ttl_seconds],
            |row| row.get(0),
        )?;
        
        Ok(count == 0)
    }
    
    /// Clear portfolio cache for a device
    pub async fn clear_portfolio_cache(&self, device_id: &str) -> Result<()> {
        let db = self.db.lock().await;
        
        db.execute("DELETE FROM portfolio_balances WHERE device_id = ?1", params![device_id])?;
        db.execute("DELETE FROM portfolio_dashboard WHERE device_id = ?1", params![device_id])?;
        db.execute("DELETE FROM portfolio_history WHERE device_id = ?1", params![device_id])?;
        db.execute("DELETE FROM transaction_cache WHERE device_id = ?1", params![device_id])?;
        
        Ok(())
    }
    
    /// Save portfolio snapshot to history
    pub async fn save_portfolio_snapshot(&self, device_id: &str, total_value_usd: f64) -> Result<()> {
        let db = self.db.lock().await;
        let now = chrono::Utc::now().timestamp();
        
        // Get current portfolio for asset count
        let asset_count: i32 = db.query_row(
            "SELECT COUNT(DISTINCT ticker) FROM portfolio_balances WHERE device_id = ?1",
            params![device_id],
            |row| row.get(0)
        ).unwrap_or(0);
        
        // Calculate 24h change
        let change_24h = self.calculate_portfolio_change(&db, device_id, 86400)?;
        
        // Save to history
        db.execute(
            "INSERT OR REPLACE INTO portfolio_history 
             (device_id, timestamp, total_value_usd, asset_count, change_24h)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![device_id, now, total_value_usd, asset_count, change_24h]
        )?;
        
        // Update last value cache
        let previous_value = db.query_row(
            "SELECT total_value_usd FROM portfolio_last_value WHERE device_id = ?1",
            params![device_id],
            |row| row.get::<_, f64>(0)
        ).ok();
        
        let change_from_previous = previous_value.map(|prev| ((total_value_usd - prev) / prev * 100.0));
        
        db.execute(
            "INSERT OR REPLACE INTO portfolio_last_value 
             (device_id, total_value_usd, last_updated, change_from_previous)
             VALUES (?1, ?2, ?3, ?4)",
            params![device_id, total_value_usd, now, change_from_previous]
        )?;
        
        Ok(())
    }
    
    /// Get last cached portfolio value for instant loading
    pub async fn get_last_portfolio_value(&self, device_id: &str) -> Result<Option<(f64, i64, Option<f64>)>> {
        let db = self.db.lock().await;
        
        db.query_row(
            "SELECT total_value_usd, last_updated, change_from_previous 
             FROM portfolio_last_value WHERE device_id = ?1",
            params![device_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        ).optional()
        .map_err(Into::into)
    }
    
    /// Get portfolio history for charting
    pub async fn get_portfolio_history(
        &self, 
        device_id: &str,
        from_timestamp: Option<i64>,
        to_timestamp: Option<i64>
    ) -> Result<Vec<(i64, f64)>> {
        let db = self.db.lock().await;
        
        let from = from_timestamp.unwrap_or(0);
        let to = to_timestamp.unwrap_or(chrono::Utc::now().timestamp());
        
        let mut stmt = db.prepare(
            "SELECT timestamp, total_value_usd 
             FROM portfolio_history 
             WHERE device_id = ?1 AND timestamp BETWEEN ?2 AND ?3
             ORDER BY timestamp ASC"
        )?;
        
        let history = stmt.query_map(params![device_id, from, to], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(history)
    }
    
    /// Calculate portfolio change over a time period
    fn calculate_portfolio_change(&self, db: &rusqlite::Connection, device_id: &str, seconds_ago: i64) -> Result<Option<f64>> {
        let cutoff = chrono::Utc::now().timestamp() - seconds_ago;
        
        let current_value: f64 = db.query_row(
            "SELECT SUM(CAST(balance_usd AS REAL)) 
             FROM portfolio_balances WHERE device_id = ?1",
            params![device_id],
            |row| row.get(0),
        ).unwrap_or(0.0);
        
        // Get historical value
        let historical: Option<f64> = db.query_row(
            "SELECT total_value_usd FROM portfolio_history 
             WHERE device_id = ?1 AND timestamp <= ?2 
             ORDER BY timestamp DESC LIMIT 1",
            params![device_id, cutoff],
            |row| row.get(0)
        ).optional()?;
        
        if let Some(hist) = historical {
            if hist > 0.0 {
                Ok(Some(((current_value - hist) / hist) * 100.0))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    
    /// Get combined portfolio history across all devices
    pub async fn get_combined_portfolio_history(
        &self,
        from_timestamp: Option<i64>,
        to_timestamp: Option<i64>
    ) -> Result<Vec<(i64, f64)>> {
        let db = self.db.lock().await;
        
        let from = from_timestamp.unwrap_or(0);
        let to = to_timestamp.unwrap_or(chrono::Utc::now().timestamp());
        
        // Get aggregated history
        let mut stmt = db.prepare(
            "SELECT timestamp, SUM(total_value_usd) as total
             FROM portfolio_history 
             WHERE timestamp BETWEEN ?1 AND ?2
             GROUP BY timestamp
             ORDER BY timestamp ASC"
        )?;
        
        let history = stmt.query_map(params![from, to], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(history)
    }
} 