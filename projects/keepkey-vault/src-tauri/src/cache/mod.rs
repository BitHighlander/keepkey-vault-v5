pub mod manager;
pub mod frontload;
pub mod migrations;
pub mod types;
pub mod portfolio;
pub mod assets;
pub mod seed_data;

pub use manager::CacheManager;
pub use frontload::FrontloadController;
pub use types::{CachedPubkey, CacheMetadata, CacheStatus};
pub use assets::{CachedAsset, CachedPath};

use std::sync::Arc;

/// Initialize the cache system and return a shared cache manager
pub async fn init_cache() -> Result<Arc<CacheManager>, String> {
    let cache = CacheManager::new().await
        .map_err(|e| format!("Failed to initialize cache: {}", e))?;
    Ok(Arc::new(cache))
} 