use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::{Result, anyhow};
use rusqlite::{Connection, params, OptionalExtension};
use super::types::{CachedPubkey, CacheMetadata, CacheStatus, FrontloadStatus};

/// Thread-safe cache manager for SQLite operations
pub struct CacheManager {
    db: Arc<Mutex<Connection>>,
    stats: Arc<Mutex<CacheStats>>,
}

#[derive(Default)]
struct CacheStats {
    hits: i64,
    misses: i64,
}

impl CacheManager {
    /// Create a new cache manager
    pub async fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        let conn = Connection::open(&db_path)?;
        
        // Enable WAL mode for better concurrency
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        
        // Apply migrations
        Self::apply_migrations(&conn)?;
        
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            stats: Arc::new(Mutex::new(CacheStats::default())),
        })
    }
    
    /// Get the database path
    fn get_db_path() -> Result<std::path::PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow!("Could not determine home directory"))?;
        
        let db_dir = home_dir.join(".keepkey").join("vault");
        std::fs::create_dir_all(&db_dir)?;
        
        Ok(db_dir.join("cache.db"))
    }
    
    /// Apply database migrations
    fn apply_migrations(conn: &Connection) -> Result<()> {
        // For now, just execute the migration SQL directly
        // In a production system, you'd track which migrations have been applied
        let migration_sql = include_str!("sql/004_cache_tables.sql");
        conn.execute_batch(migration_sql)?;
        Ok(())
    }
    
    /// Get a cached pubkey
    pub async fn get_cached_pubkey(
        &self,
        device_id: &str,
        derivation_path: &str,
        coin_name: &str,
        script_type: Option<&str>,
    ) -> Option<CachedPubkey> {
        let db = self.db.lock().await;
        
        let result: Option<CachedPubkey> = db.query_row(
            "SELECT id, device_id, derivation_path, coin_name, script_type, 
                    xpub, address, chain_code, public_key, cached_at, last_used
             FROM cached_pubkeys 
             WHERE device_id = ?1 AND derivation_path = ?2 AND coin_name = ?3 
             AND (script_type = ?4 OR (?4 IS NULL AND script_type IS NULL))",
            params![device_id, derivation_path, coin_name, script_type],
            |row| {
                Ok(CachedPubkey {
                    id: row.get(0)?,
                    device_id: row.get(1)?,
                    derivation_path: row.get(2)?,
                    coin_name: row.get(3)?,
                    script_type: row.get(4)?,
                    xpub: row.get(5)?,
                    address: row.get(6)?,
                    chain_code: row.get(7)?,
                    public_key: row.get(8)?,
                    cached_at: row.get(9)?,
                    last_used: row.get(10)?,
                })
            },
        ).optional().ok().flatten();
        
        // Update stats
        let mut stats = self.stats.lock().await;
        if result.is_some() {
            stats.hits += 1;
            // Update last_used timestamp
            if let Some(ref cached) = result {
                if let Some(id) = cached.id {
                    let _ = db.execute(
                        "UPDATE cached_pubkeys SET last_used = strftime('%s', 'now') WHERE id = ?1",
                        params![id],
                    );
                }
            }
        } else {
            stats.misses += 1;
        }
        
        result
    }
    
    /// Save a pubkey to cache
    pub async fn save_pubkey(&self, pubkey: &CachedPubkey) -> Result<()> {
        let db = self.db.lock().await;
        
        db.execute(
            "INSERT OR REPLACE INTO cached_pubkeys 
             (device_id, derivation_path, coin_name, script_type, xpub, address, 
              chain_code, public_key, cached_at, last_used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                pubkey.device_id,
                pubkey.derivation_path,
                pubkey.coin_name,
                pubkey.script_type,
                pubkey.xpub,
                pubkey.address,
                pubkey.chain_code,
                pubkey.public_key,
                pubkey.cached_at,
                pubkey.last_used,
            ],
        )?;
        
        Ok(())
    }
    
    /// Get cache metadata for a device
    pub async fn get_cache_metadata(&self, device_id: &str) -> Option<CacheMetadata> {
        let db = self.db.lock().await;
        
        db.query_row(
            "SELECT device_id, label, firmware_version, initialized, 
                    frontload_status, frontload_progress, last_frontload, error_message
             FROM cache_metadata WHERE device_id = ?1",
            params![device_id],
            |row| {
                let status_str: String = row.get(4)?;
                let status = FrontloadStatus::from_str(&status_str)
                    .unwrap_or(FrontloadStatus::Pending);
                
                Ok(CacheMetadata {
                    device_id: row.get(0)?,
                    label: row.get(1)?,
                    firmware_version: row.get(2)?,
                    initialized: row.get(3)?,
                    frontload_status: status,
                    frontload_progress: row.get(5)?,
                    last_frontload: row.get(6)?,
                    error_message: row.get(7)?,
                })
            },
        ).optional().ok().flatten()
    }
    
    /// Update cache metadata
    pub async fn update_cache_metadata(&self, metadata: &CacheMetadata) -> Result<()> {
        let db = self.db.lock().await;
        
        db.execute(
            "INSERT OR REPLACE INTO cache_metadata 
             (device_id, label, firmware_version, initialized, 
              frontload_status, frontload_progress, last_frontload, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                metadata.device_id,
                metadata.label,
                metadata.firmware_version,
                metadata.initialized,
                metadata.frontload_status.as_str(),
                metadata.frontload_progress,
                metadata.last_frontload,
                metadata.error_message,
            ],
        )?;
        
        Ok(())
    }
    
    /// Get cache status for a device
    pub async fn get_cache_status(&self, device_id: &str) -> Result<CacheStatus> {
        let db = self.db.lock().await;
        let stats = self.stats.lock().await;
        
        // Count cached entries for this device
        let total_cached: i64 = db.query_row(
            "SELECT COUNT(*) FROM cached_pubkeys WHERE device_id = ?1",
            params![device_id],
            |row| row.get(0),
        )?;
        
        // Get metadata
        let metadata = self.get_cache_metadata(device_id).await
            .unwrap_or_else(|| CacheMetadata {
                device_id: device_id.to_string(),
                label: None,
                firmware_version: None,
                initialized: false,
                frontload_status: FrontloadStatus::Pending,
                frontload_progress: 0,
                last_frontload: None,
                error_message: None,
            });
        
        let hit_rate = if stats.hits + stats.misses > 0 {
            (stats.hits as f64) / ((stats.hits + stats.misses) as f64)
        } else {
            0.0
        };
        
        Ok(CacheStatus {
            device_id: device_id.to_string(),
            total_cached,
            cache_hits: stats.hits,
            cache_misses: stats.misses,
            hit_rate,
            last_frontload: metadata.last_frontload,
            frontload_status: metadata.frontload_status,
            frontload_progress: metadata.frontload_progress,
        })
    }
    
    /// Clear cache for a specific device
    pub async fn clear_device_cache(&self, device_id: &str) -> Result<()> {
        let db = self.db.lock().await;
        
        db.execute(
            "DELETE FROM cached_pubkeys WHERE device_id = ?1",
            params![device_id],
        )?;
        
        db.execute(
            "DELETE FROM cache_metadata WHERE device_id = ?1",
            params![device_id],
        )?;
        
        Ok(())
    }
    
    /// Clean up old cache entries (older than 30 days)
    pub async fn cleanup_old_entries(&self) -> Result<i64> {
        let db = self.db.lock().await;
        let thirty_days_ago = chrono::Utc::now().timestamp() - (30 * 24 * 60 * 60);
        
        let count = db.execute(
            "DELETE FROM cached_pubkeys WHERE last_used < ?1",
            params![thirty_days_ago],
        )?;
        
        Ok(count as i64)
    }
} 