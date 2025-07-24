-- Migration 006: Fix portfolio_balances table to prevent duplicate assets
-- The previous UNIQUE constraint with nullable fields (address, validator) 
-- was causing duplicate entries to be created instead of updating existing ones

-- Drop the old table and recreate with better constraints
DROP TABLE IF EXISTS portfolio_balances_old;
ALTER TABLE portfolio_balances RENAME TO portfolio_balances_old;

-- Create new portfolio_balances table with improved UNIQUE constraint
CREATE TABLE portfolio_balances (
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
    
    -- Better UNIQUE constraint that handles NULL values properly
    -- Use COALESCE to provide default values for nullable fields
    UNIQUE(device_id, pubkey, caip, COALESCE(address, ''), type, COALESCE(validator, ''))
);

-- Copy data from old table, removing duplicates
INSERT INTO portfolio_balances 
SELECT DISTINCT 
    id, device_id, pubkey, caip, network_id, ticker, address, 
    balance, balance_usd, price_usd, type, name, icon, precision, 
    contract, validator, unbonding_end, rewards_available, 
    last_updated, last_block_height, is_verified
FROM portfolio_balances_old;

-- Drop the old table
DROP TABLE portfolio_balances_old;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_portfolio_balances_device 
ON portfolio_balances(device_id);

CREATE INDEX IF NOT EXISTS idx_portfolio_balances_lookup 
ON portfolio_balances(device_id, network_id, ticker);

CREATE INDEX IF NOT EXISTS idx_portfolio_balances_pubkey 
ON portfolio_balances(pubkey);

CREATE INDEX IF NOT EXISTS idx_portfolio_balances_updated 
ON portfolio_balances(last_updated); 