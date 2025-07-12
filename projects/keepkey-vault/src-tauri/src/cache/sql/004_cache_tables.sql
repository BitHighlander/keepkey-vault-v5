-- Migration 004: Add frontload cache tables
-- This migration adds tables for caching public keys and addresses
-- to improve response times for common operations

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
    frontload_status TEXT CHECK(frontload_status IN ('pending', 'in_progress', 'completed', 'failed')),
    frontload_progress INTEGER DEFAULT 0,
    last_frontload INTEGER,
    error_message TEXT
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_cached_pubkeys_lookup 
ON cached_pubkeys(device_id, derivation_path);

CREATE INDEX IF NOT EXISTS idx_cached_pubkeys_coin 
ON cached_pubkeys(device_id, coin_name);

CREATE INDEX IF NOT EXISTS idx_cached_pubkeys_last_used
ON cached_pubkeys(last_used);

-- Trigger to update last_used timestamp on access
CREATE TRIGGER IF NOT EXISTS update_last_used_timestamp 
AFTER UPDATE ON cached_pubkeys
FOR EACH ROW
WHEN NEW.last_used = OLD.last_used
BEGIN
    UPDATE cached_pubkeys 
    SET last_used = strftime('%s', 'now') 
    WHERE id = NEW.id;
END; 