# KeepKey Vault Frontload Implementation Plan

## Overview

This document outlines the implementation plan for adding a frontload cache system to keepkey-vault that caches public keys and addresses to improve response times. The system will use Tauri's SQLite plugin for storage and integrate transparently with the existing device queue system.

## Architecture

### Key Principles

1. **Always use device queue** - All device communication must go through the existing `DeviceQueueHandle` system
2. **Transparent cache layer** - The cache acts as a pass-through, falling back to device on cache miss
3. **Multi-device support** - Each device has its own cache namespace identified by device_id
4. **Frontend visibility** - The frontend can access the SQLite database directly via Tauri SQL plugin

### Components

1. **Cache Manager Module** (`src-tauri/src/cache/mod.rs`)
   - Manages SQLite database operations
   - Provides cache lookup and storage
   - Handles cache invalidation

2. **Frontload Controller** (`src-tauri/src/cache/frontload.rs`)
   - Orchestrates the frontloading process
   - Uses device queue for all device operations
   - Manages frontload progress

3. **Database Schema** (via Tauri SQL migrations)
   - Stores cached public keys by device and derivation path
   - Tracks cache timestamps and validity

4. **API Integration**
   - Existing endpoints enhanced with cache checks
   - Transparent fallback to device queue

## Database Schema

```sql
-- Cached public keys and addresses
CREATE TABLE IF NOT EXISTS cached_pubkeys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL,
    derivation_path TEXT NOT NULL,
    coin_name TEXT NOT NULL,
    script_type TEXT,
    xpub TEXT,
    address TEXT,
    chain_code BLOB,
    public_key BLOB,
    cached_at INTEGER NOT NULL,
    last_used INTEGER NOT NULL,
    UNIQUE(device_id, derivation_path, coin_name, script_type)
);

-- Device cache metadata
CREATE TABLE IF NOT EXISTS cache_metadata (
    device_id TEXT PRIMARY KEY,
    label TEXT,
    firmware_version TEXT,
    initialized BOOLEAN,
    frontload_status TEXT, -- 'pending', 'in_progress', 'completed', 'failed'
    frontload_progress INTEGER,
    last_frontload INTEGER,
    error_message TEXT
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_cached_pubkeys_lookup 
ON cached_pubkeys(device_id, derivation_path);

CREATE INDEX IF NOT EXISTS idx_cached_pubkeys_coin 
ON cached_pubkeys(device_id, coin_name);
```

## Implementation Steps

### Phase 1: Cache Infrastructure

1. **Add SQL migrations**
   ```rust
   // src-tauri/src/cache/migrations.rs
   pub fn get_cache_migrations() -> Vec<Migration> {
       vec![
           Migration {
               version: 4,
               description: "create_cache_tables",
               sql: include_str!("sql/004_cache_tables.sql"),
               kind: MigrationKind::Up,
           }
       ]
   }
   ```

2. **Create Cache Manager**
   ```rust
   // src-tauri/src/cache/manager.rs
   pub struct CacheManager {
       db: Arc<Mutex<SqlitePool>>,
   }
   
   impl CacheManager {
       pub async fn get_cached_pubkey(&self, device_id: &str, path: &str) -> Option<CachedPubkey>
       pub async fn save_pubkey(&self, pubkey: &CachedPubkey) -> Result<()>
       pub async fn invalidate_device_cache(&self, device_id: &str) -> Result<()>
   }
   ```

### Phase 2: Frontload Implementation

1. **Create Frontload Controller**
   ```rust
   // src-tauri/src/cache/frontload.rs
   pub struct FrontloadController {
       cache: Arc<CacheManager>,
       queue_manager: Arc<Mutex<HashMap<String, DeviceQueueHandle>>>,
   }
   
   impl FrontloadController {
       pub async fn frontload_device(&self, device_id: &str) -> Result<()> {
           // Get device queue handle
           let queue_handle = self.get_queue_handle(device_id).await?;
           
           // Frontload common paths
           for (coin, paths) in COMMON_PATHS.iter() {
               for path in paths {
                   self.frontload_path(&queue_handle, device_id, coin, path).await?;
               }
           }
       }
   }
   ```

2. **Define common paths to frontload**
   ```rust
   // Based on existing kkcli implementation
   const BITCOIN_PATHS: &[(&str, &[u32])] = &[
       ("p2pkh", &[0x8000002C, 0x80000000, 0x80000000, 0, 0]),
       ("p2wpkh", &[0x80000054, 0x80000000, 0x80000000, 0, 0]),
       // ... more paths
   ];
   ```

### Phase 3: API Integration

1. **Enhance existing endpoints**
   ```rust
   // In device/system_operations.rs
   pub async fn process_system_request_with_cache(
       cache: &CacheManager,
       queue_handle: &DeviceQueueHandle,
       request: &DeviceRequest,
   ) -> Result<DeviceResponse> {
       match request {
           DeviceRequest::GetPublicKey { path, .. } => {
               // Check cache first
               if let Some(cached) = cache.get_cached_pubkey(device_id, path).await {
                   return Ok(cached.into());
               }
               
               // Cache miss - fetch from device
               let response = process_system_request(queue_handle, request).await?;
               
               // Save to cache
               cache.save_pubkey(&response).await?;
               
               Ok(response)
           }
           _ => process_system_request(queue_handle, request).await,
       }
   }
   ```

### Phase 4: Frontend Integration

1. **Add Tauri commands for cache management**
   ```rust
   #[tauri::command]
   pub async fn get_cache_status(device_id: String) -> Result<CacheStatus> {
       // Return cache statistics and frontload status
   }
   
   #[tauri::command]
   pub async fn trigger_frontload(device_id: String) -> Result<()> {
       // Start frontload process for device
   }
   
   #[tauri::command]
   pub async fn clear_device_cache(device_id: String) -> Result<()> {
       // Clear cache for specific device
   }
   ```

2. **Frontend API**
   ```typescript
   // src/lib/cache.ts
   export class CacheAPI {
     static async getCacheStatus(deviceId: string): Promise<CacheStatus>
     static async triggerFrontload(deviceId: string): Promise<void>
     static async clearCache(deviceId: string): Promise<void>
     
     // Direct SQL access for frontend
     static async getCachedPubkeys(deviceId: string): Promise<CachedPubkey[]> {
       const db = await Database.load('sqlite:keepkey-vault.db');
       return db.select('SELECT * FROM cached_pubkeys WHERE device_id = ?', [deviceId]);
     }
   }
   ```

## Testing Strategy

1. **Unit Tests**
   - Test cache manager operations
   - Test frontload logic
   - Test cache invalidation

2. **Integration Tests**
   - Test cache hit/miss scenarios
   - Test multi-device caching
   - Test device disconnection handling

3. **Performance Tests**
   - Measure response time improvement
   - Test cache performance with large datasets
   - Test concurrent device operations

## Migration from kkcli

The implementation will adapt the proven frontload logic from `kkcli/src/server/cache/frontload.rs` but with key differences:

1. Use Tauri SQL instead of direct rusqlite
2. Integrate with existing device queue manager
3. Provide frontend visibility via SQL plugin
4. Support existing API patterns in keepkey-vault

## Success Metrics

1. **Performance**: API response times < 50ms for cached data
2. **Coverage**: >90% cache hit rate for common operations
3. **Reliability**: Transparent fallback with no user-visible errors
4. **Multi-device**: Proper isolation between device caches

## Future Enhancements

1. **Smart prefetching** - Predict and cache likely next addresses
2. **Cache warming** - Background refresh of stale entries
3. **Analytics** - Track cache performance metrics
4. **Selective frontload** - Allow users to choose which paths to cache 