-- Migration 005: Add comprehensive portfolio cache tables
-- This migration extends the existing cache to support full portfolio data
-- as migrated from pioneer-sdk TypeScript to Rust

-- Enhanced portfolio cache with more detailed balance information
CREATE TABLE IF NOT EXISTS portfolio_balances (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL,
    pubkey TEXT NOT NULL,      -- xpub from wallet_xpubs
    caip TEXT NOT NULL,        -- CAIP identifier (e.g., "eip155:1/slip44:60")
    network_id TEXT NOT NULL,  -- Network identifier (e.g., "eip155:1")
    ticker TEXT NOT NULL,      -- Asset ticker (e.g., "ETH", "BTC")
    address TEXT,              -- Specific address if applicable
    balance TEXT NOT NULL,     -- Balance as string (preserve precision)
    balance_usd TEXT NOT NULL, -- USD value as string
    price_usd TEXT NOT NULL,   -- Price per unit in USD
    type TEXT,                 -- 'balance', 'staking', 'delegation', 'reward', 'unbonding'
    
    -- Additional fields from pioneer-sdk
    name TEXT,                 -- Asset full name
    icon TEXT,                 -- Asset icon URL
    precision INTEGER,         -- Decimal places for display
    contract TEXT,             -- Contract address for tokens
    
    -- Staking specific fields
    validator TEXT,            -- Validator address for delegations
    unbonding_end INTEGER,     -- Timestamp when unbonding completes
    rewards_available TEXT,    -- Available rewards amount
    
    -- Metadata
    last_updated INTEGER NOT NULL,
    last_block_height INTEGER,
    is_verified BOOLEAN DEFAULT 0,
    
    UNIQUE(device_id, pubkey, caip, address, type, validator),
    FOREIGN KEY (device_id) REFERENCES devices(device_id) ON DELETE CASCADE
);

-- Asset metadata cache for token/coin information
CREATE TABLE IF NOT EXISTS asset_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    caip TEXT NOT NULL UNIQUE,
    ticker TEXT NOT NULL,
    name TEXT NOT NULL,
    icon TEXT,
    network_id TEXT NOT NULL,
    contract TEXT,
    decimals INTEGER,
    coin_gecko_id TEXT,
    is_native BOOLEAN DEFAULT 0,
    is_verified BOOLEAN DEFAULT 0,
    last_updated INTEGER NOT NULL
);

-- Network metadata cache
CREATE TABLE IF NOT EXISTS network_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    network_id TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    chain_id INTEGER,
    native_asset_caip TEXT,
    explorer_url TEXT,
    rpc_url TEXT,
    is_testnet BOOLEAN DEFAULT 0,
    last_updated INTEGER NOT NULL
);

-- Dashboard aggregation cache (pre-computed totals)
CREATE TABLE IF NOT EXISTS portfolio_dashboard (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL,
    total_value_usd TEXT NOT NULL,
    
    -- Network breakdowns (JSON)
    networks_json TEXT NOT NULL,      -- Array of {networkId, name, valueUsd, percentage}
    assets_json TEXT NOT NULL,        -- Array of {ticker, name, valueUsd, balance, percentage}
    
    -- Statistics
    total_assets INTEGER,
    total_networks INTEGER,
    last_24h_change_usd TEXT,
    last_24h_change_percent TEXT,
    
    -- Combined portfolio flag
    is_combined BOOLEAN DEFAULT 0,    -- True if this is a combined multi-device portfolio
    included_devices TEXT,            -- JSON array of device_ids if combined
    
    last_updated INTEGER NOT NULL,
    UNIQUE(device_id)
);

-- Portfolio history for tracking value over time
CREATE TABLE IF NOT EXISTS portfolio_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    total_value_usd TEXT NOT NULL,
    snapshot_json TEXT              -- Full portfolio snapshot as JSON
);

-- Transaction cache for recent activity
CREATE TABLE IF NOT EXISTS transaction_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL,
    txid TEXT NOT NULL,
    caip TEXT NOT NULL,
    type TEXT NOT NULL,              -- 'send', 'receive', 'swap', 'stake', 'unstake'
    amount TEXT NOT NULL,
    amount_usd TEXT,
    fee TEXT,
    fee_usd TEXT,
    from_address TEXT,
    to_address TEXT,
    timestamp INTEGER NOT NULL,
    block_height INTEGER,
    status TEXT,                     -- 'pending', 'confirmed', 'failed'
    metadata_json TEXT,              -- Additional transaction-specific data
    UNIQUE(device_id, txid, caip)
);

-- Frontload progress tracking per asset/network
CREATE TABLE IF NOT EXISTS frontload_progress (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL,
    network_id TEXT NOT NULL,
    paths_total INTEGER NOT NULL,
    paths_completed INTEGER NOT NULL,
    last_path TEXT,
    status TEXT CHECK(status IN ('pending', 'in_progress', 'completed', 'failed')),
    error_message TEXT,
    started_at INTEGER,
    completed_at INTEGER,
    UNIQUE(device_id, network_id)
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_portfolio_history_lookup 
ON portfolio_history(device_id, timestamp);

CREATE INDEX IF NOT EXISTS idx_portfolio_balances_device 
ON portfolio_balances(device_id);

CREATE INDEX IF NOT EXISTS idx_portfolio_balances_lookup 
ON portfolio_balances(device_id, network_id, ticker);

CREATE INDEX IF NOT EXISTS idx_portfolio_balances_pubkey 
ON portfolio_balances(pubkey);

CREATE INDEX IF NOT EXISTS idx_portfolio_balances_updated 
ON portfolio_balances(last_updated);

CREATE INDEX IF NOT EXISTS idx_asset_metadata_ticker 
ON asset_metadata(ticker, network_id);

CREATE INDEX IF NOT EXISTS idx_transaction_cache_device 
ON transaction_cache(device_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_transaction_cache_status 
ON transaction_cache(status, timestamp DESC);

-- Views for common queries

-- Combined portfolio view across all devices
CREATE VIEW IF NOT EXISTS v_combined_portfolio AS
SELECT 
    'combined' as device_id,
    caip,
    network_id,
    ticker,
    SUM(CAST(balance AS REAL)) as total_balance,
    SUM(CAST(balance_usd AS REAL)) as total_value_usd,
    MAX(price_usd) as price_usd,
    MAX(last_updated) as last_updated
FROM portfolio_balances
WHERE type = 'balance'
GROUP BY caip, network_id, ticker;

-- Per-device portfolio summary
CREATE VIEW IF NOT EXISTS v_device_portfolio_summary AS
SELECT 
    device_id,
    COUNT(DISTINCT caip) as total_assets,
    COUNT(DISTINCT network_id) as total_networks,
    SUM(CAST(balance_usd AS REAL)) as total_value_usd,
    MAX(last_updated) as last_updated
FROM portfolio_balances
WHERE type = 'balance'
GROUP BY device_id; 