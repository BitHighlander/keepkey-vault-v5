import { invoke } from '@tauri-apps/api/core';
import Database from '@tauri-apps/plugin-sql';

export interface CacheStatus {
  device_id: string;
  total_cached: number;
  cache_hits: number;
  cache_misses: number;
  hit_rate: number;
  last_frontload: number | null;
  frontload_status: 'pending' | 'in_progress' | 'completed' | 'failed';
  frontload_progress: number;
}

export interface CachedPubkey {
  id: number | null;
  device_id: string;
  derivation_path: string;
  coin_name: string;
  script_type: string | null;
  xpub: string | null;
  address: string | null;
  chain_code: Uint8Array | null;
  public_key: Uint8Array | null;
  cached_at: number;
  last_used: number;
}

export interface CacheMetadata {
  device_id: string;
  label: string | null;
  firmware_version: string | null;
  initialized: boolean;
  frontload_status: 'pending' | 'in_progress' | 'completed' | 'failed';
  frontload_progress: number;
  last_frontload: number | null;
  error_message: string | null;
}

export class CacheAPI {
  /**
   * Get cache status for a specific device
   */
  static async getCacheStatus(deviceId: string): Promise<CacheStatus> {
    try {
      return await invoke('get_cache_status', { deviceId });
    } catch (error) {
      console.error('Failed to get cache status:', error);
      throw error;
    }
  }

  /**
   * Trigger frontload process for a device
   * This runs in the background and returns immediately
   */
  static async triggerFrontload(deviceId: string): Promise<void> {
    try {
      await invoke('trigger_frontload', { deviceId });
    } catch (error) {
      console.error('Failed to trigger frontload:', error);
      throw error;
    }
  }

  /**
   * Clear all cached data for a specific device
   */
  static async clearCache(deviceId: string): Promise<void> {
    try {
      await invoke('clear_device_cache', { deviceId });
    } catch (error) {
      console.error('Failed to clear device cache:', error);
      throw error;
    }
  }

  /**
   * Get cached public keys for a device directly from SQLite
   * This demonstrates frontend direct database access
   */
  static async getCachedPubkeys(deviceId: string): Promise<CachedPubkey[]> {
    try {
      const db = await Database.load('sqlite:keepkey-vault.db');
      const result = await db.select<CachedPubkey[]>(
        'SELECT * FROM cached_pubkeys WHERE device_id = $1 ORDER BY last_used DESC',
        [deviceId]
      );
      return result;
    } catch (error) {
      console.error('Failed to get cached pubkeys:', error);
      throw error;
    }
  }

  /**
   * Get cache metadata for a device
   */
  static async getCacheMetadata(deviceId: string): Promise<CacheMetadata | null> {
    try {
      const db = await Database.load('sqlite:keepkey-vault.db');
      const result = await db.select<CacheMetadata[]>(
        'SELECT * FROM cache_metadata WHERE device_id = $1',
        [deviceId]
      );
      return result.length > 0 ? result[0] : null;
    } catch (error) {
      console.error('Failed to get cache metadata:', error);
      throw error;
    }
  }

  /**
   * Search cached addresses by coin name
   */
  static async searchCachedAddresses(deviceId: string, coinName: string): Promise<CachedPubkey[]> {
    try {
      const db = await Database.load('sqlite:keepkey-vault.db');
      const result = await db.select<CachedPubkey[]>(
        'SELECT * FROM cached_pubkeys WHERE device_id = $1 AND coin_name = $2 AND address IS NOT NULL ORDER BY derivation_path',
        [deviceId, coinName]
      );
      return result;
    } catch (error) {
      console.error('Failed to search cached addresses:', error);
      throw error;
    }
  }

  /**
   * Get cache statistics for all devices
   */
  static async getGlobalCacheStats(): Promise<{
    total_devices: number;
    total_cached_entries: number;
    total_size_bytes: number;
  }> {
    try {
      const db = await Database.load('sqlite:keepkey-vault.db');
      
      const deviceCount = await db.select<[{ count: number }]>(
        'SELECT COUNT(DISTINCT device_id) as count FROM cached_pubkeys'
      );
      
      const entryCount = await db.select<[{ count: number }]>(
        'SELECT COUNT(*) as count FROM cached_pubkeys'
      );
      
      // Rough estimate of cache size (this is approximate)
      const sizeEstimate = entryCount[0].count * 500; // ~500 bytes per entry
      
      return {
        total_devices: deviceCount[0].count,
        total_cached_entries: entryCount[0].count,
        total_size_bytes: sizeEstimate,
      };
    } catch (error) {
      console.error('Failed to get global cache stats:', error);
      throw error;
    }
  }

  /**
   * Monitor frontload progress
   * Returns a promise that resolves when frontload is complete
   */
  static async monitorFrontload(deviceId: string, onProgress?: (progress: number) => void): Promise<void> {
    return new Promise((resolve, reject) => {
      const checkInterval = setInterval(async () => {
        try {
          const metadata = await this.getCacheMetadata(deviceId);
          
          if (!metadata) {
            clearInterval(checkInterval);
            reject(new Error('Device not found'));
            return;
          }
          
          if (onProgress) {
            onProgress(metadata.frontload_progress);
          }
          
          if (metadata.frontload_status === 'completed') {
            clearInterval(checkInterval);
            resolve();
          } else if (metadata.frontload_status === 'failed') {
            clearInterval(checkInterval);
            reject(new Error(metadata.error_message || 'Frontload failed'));
          }
        } catch (error) {
          clearInterval(checkInterval);
          reject(error);
        }
      }, 1000); // Check every second
      
      // Timeout after 5 minutes
      setTimeout(() => {
        clearInterval(checkInterval);
        reject(new Error('Frontload timeout'));
      }, 300000);
    });
  }
} 