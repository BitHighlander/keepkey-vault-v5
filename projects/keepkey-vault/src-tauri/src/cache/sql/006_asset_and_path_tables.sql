-- Migration 006: Add asset and path cache tables
-- Store assetData from @pioneer-platform/pioneer-discovery and paths configuration

-- Asset registry table - stores all known assets
CREATE TABLE IF NOT EXISTS assets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    caip TEXT NOT NULL UNIQUE,              -- e.g., "eip155:1/slip44:60", "eip155:1/erc20:0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    network_id TEXT NOT NULL,               -- e.g., "eip155:1", "cosmos:cosmoshub-4"
    chain_id TEXT,                          -- e.g., "1" for Ethereum mainnet
    symbol TEXT NOT NULL,                   -- e.g., "ETH", "BTC", "USDC"
    name TEXT NOT NULL,                     -- e.g., "Ethereum", "Bitcoin", "USD Coin"
    
    -- Asset type information
    asset_type TEXT CHECK(asset_type IN ('native', 'token', 'nft')),
    is_native BOOLEAN DEFAULT 0,
    contract_address TEXT,                  -- For tokens/NFTs
    token_id TEXT,                         -- For NFTs
    
    -- Display information
    icon TEXT,                             -- Icon URL
    color TEXT,                            -- Hex color code
    decimals INTEGER,                      -- Decimal places
    precision INTEGER,                     -- Display precision
    
    -- Network information
    network_name TEXT,                     -- e.g., "Ethereum Mainnet"
    native_asset_caip TEXT,                -- CAIP of the native gas asset for this network
    
    -- Explorer links
    explorer TEXT,                         -- Base explorer URL
    explorer_address_link TEXT,            -- Address explorer pattern
    explorer_tx_link TEXT,                 -- Transaction explorer pattern
    
    -- Additional metadata
    coin_gecko_id TEXT,                    -- CoinGecko ID for price data
    chain_reference TEXT,                  -- Reference in chain (e.g., IBC denom)
    tags TEXT,                            -- JSON array of tags
    
    -- Source tracking
    source TEXT DEFAULT 'pioneer-discovery',
    is_verified BOOLEAN DEFAULT 1,
    
    -- Timestamps
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    last_updated INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Derivation paths table - stores HD wallet paths from default-paths.json
CREATE TABLE IF NOT EXISTS derivation_paths (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path_id TEXT NOT NULL UNIQUE,          -- e.g., "bitcoin_44", "ethereum_44"
    note TEXT,                             -- Human-readable description
    blockchain TEXT NOT NULL,              -- e.g., "bitcoin", "ethereum", "cosmos"
    symbol TEXT NOT NULL,                  -- Primary symbol (e.g., "BTC", "ETH")
    
    -- Network associations
    networks TEXT NOT NULL,                -- JSON array of network IDs this path works with
    
    -- Script type (for UTXO chains)
    script_type TEXT,                      -- e.g., "p2pkh", "p2sh-p2wpkh", "p2wpkh"
    
    -- BIP32 path components
    address_n_list TEXT NOT NULL,          -- JSON array for account path (e.g., [44, 0, 0])
    address_n_list_master TEXT NOT NULL,   -- JSON array for address path (e.g., [44, 0, 0, 0, 0])
    
    -- Cryptographic curve
    curve TEXT NOT NULL DEFAULT 'secp256k1',
    
    -- UI hints
    show_display BOOLEAN DEFAULT 0,        -- Whether to show on device display
    is_default BOOLEAN DEFAULT 0,          -- Whether this is the default path for the blockchain
    
    -- Additional metadata
    tags TEXT,                            -- JSON array of tags
    version INTEGER DEFAULT 1,             -- Path format version
    
    -- Timestamps
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    last_updated INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Path to CAIP mapping table - maps derivation paths to asset CAIPs
CREATE TABLE IF NOT EXISTS path_asset_mapping (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path_id TEXT NOT NULL,                 -- References derivation_paths.path_id
    caip TEXT NOT NULL,                    -- References assets.caip
    network_id TEXT NOT NULL,              -- Network this mapping applies to
    is_primary BOOLEAN DEFAULT 0,          -- Whether this is the primary path for this asset
    
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    
    UNIQUE(path_id, caip, network_id),
    FOREIGN KEY (path_id) REFERENCES derivation_paths(path_id),
    FOREIGN KEY (caip) REFERENCES assets(caip)
);

-- Network metadata table - comprehensive network information
CREATE TABLE IF NOT EXISTS networks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    network_id TEXT NOT NULL UNIQUE,       -- e.g., "eip155:1", "cosmos:cosmoshub-4"
    name TEXT NOT NULL,                    -- e.g., "Ethereum Mainnet", "Cosmos Hub"
    short_name TEXT,                       -- e.g., "ETH", "COSMOS"
    chain_id TEXT,                         -- Chain ID (numeric for EVM, string for others)
    
    -- Network type
    network_type TEXT CHECK(network_type IN ('evm', 'utxo', 'cosmos', 'other')),
    
    -- Native asset
    native_asset_caip TEXT NOT NULL,       -- CAIP of native asset
    native_symbol TEXT NOT NULL,           -- Native asset symbol
    
    -- RPC endpoints
    rpc_urls TEXT,                         -- JSON array of RPC URLs
    ws_urls TEXT,                          -- JSON array of WebSocket URLs
    
    -- Explorer information
    explorer_url TEXT,
    explorer_api_url TEXT,
    explorer_api_key_required BOOLEAN DEFAULT 0,
    
    -- Chain specific features
    supports_eip1559 BOOLEAN DEFAULT 0,    -- EVM: Supports EIP-1559
    supports_memo BOOLEAN DEFAULT 0,       -- Supports memo/message field
    supports_tokens BOOLEAN DEFAULT 0,     -- Supports token standards
    
    -- Fee configuration
    fee_asset_caip TEXT,                   -- Asset used for fees (usually same as native)
    min_fee TEXT,                          -- Minimum fee in native units
    
    -- Additional metadata
    tags TEXT,                             -- JSON array of tags
    is_testnet BOOLEAN DEFAULT 0,
    is_active BOOLEAN DEFAULT 1,
    
    -- Timestamps
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    last_updated INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    
    FOREIGN KEY (native_asset_caip) REFERENCES assets(caip)
);

-- Create views for common queries

-- View: All assets with their network information
CREATE VIEW IF NOT EXISTS v_assets_with_networks AS
SELECT 
    a.*,
    n.name as network_name,
    n.network_type,
    n.explorer_url as network_explorer,
    n.supports_tokens,
    n.is_testnet
FROM assets a
LEFT JOIN networks n ON a.network_id = n.network_id;

-- View: Derivation paths with their primary assets
CREATE VIEW IF NOT EXISTS v_paths_with_assets AS
SELECT 
    dp.*,
    pam.caip,
    a.symbol as asset_symbol,
    a.name as asset_name,
    a.icon as asset_icon
FROM derivation_paths dp
LEFT JOIN path_asset_mapping pam ON dp.path_id = pam.path_id AND pam.is_primary = 1
LEFT JOIN assets a ON pam.caip = a.caip;

-- Triggers to update timestamps
CREATE TRIGGER IF NOT EXISTS update_assets_timestamp 
AFTER UPDATE ON assets
FOR EACH ROW
BEGIN
    UPDATE assets SET last_updated = strftime('%s', 'now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS update_paths_timestamp 
AFTER UPDATE ON derivation_paths
FOR EACH ROW
BEGIN
    UPDATE derivation_paths SET last_updated = strftime('%s', 'now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS update_networks_timestamp 
AFTER UPDATE ON networks
FOR EACH ROW
BEGIN
    UPDATE networks SET last_updated = strftime('%s', 'now') WHERE id = NEW.id;
END;

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_assets_network_id ON assets(network_id);
CREATE INDEX IF NOT EXISTS idx_assets_symbol ON assets(symbol);
CREATE INDEX IF NOT EXISTS idx_assets_contract ON assets(contract_address);
CREATE INDEX IF NOT EXISTS idx_assets_type ON assets(asset_type);

CREATE INDEX IF NOT EXISTS idx_paths_blockchain ON derivation_paths(blockchain);
CREATE INDEX IF NOT EXISTS idx_paths_symbol ON derivation_paths(symbol);