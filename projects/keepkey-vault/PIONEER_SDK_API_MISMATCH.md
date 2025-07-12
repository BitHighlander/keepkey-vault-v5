# Pioneer SDK API Mismatch Analysis

## The Problem

Pioneer SDK startup fails because it expects **specific REST API endpoints** that don't match our keepkey-vault implementation. During startup, Pioneer SDK attempts to get public keys/addresses for **21 different cryptocurrency paths**, but the API signatures don't match.

## What Pioneer SDK Expects (from https://pioneers.dev/spec/swagger.json)

### Key Endpoints Pioneer SDK Calls:
1. **`/getNewAddress/{network}/{xpub}`** - GET request expecting `xpub` parameter
2. **`/getChangeAddress/{network}/{xpub}`** - GET request expecting `xpub` parameter  
3. **`/getPubkeyBalance/{asset}/{pubkey}`** - GET request expecting `pubkey` parameter
4. **`/getAccountInfo/{network}/{address}`** - GET request expecting `address` parameter

### Pioneer SDK Expected Response Format:
```json
// /getNewAddress/{network}/{xpub}
{
  "receiveIndex": 0
}

// /getChangeAddress/{network}/{xpub}  
{
  "receiveIndex": 0,
  "changeIndex": 1
}
```

## What keepkey-vault Currently Implements

### Our Current Address Endpoints:
1. **`/addresses/eth`** - POST request with `addressNList` array
2. **`/addresses/utxo`** - POST request with `addressNList` array
3. **`/addresses/cosmos`** - POST request with `addressNList` array
4. **`/addresses/osmosis`** - POST request with `addressNList` array
5. **`/addresses/mayachain`** - POST request with `addressNList` array
6. **`/addresses/thorchain`** - POST request with `addressNList` array
7. **`/addresses/xrp`** - POST request with `addressNList` array

### Our Current Request/Response Format:
```json
// Request to /addresses/eth
{
  "addressNList": [2147483692, 2147483708, 2147483648, 0, 0],
  "curve": "secp256k1", 
  "showDisplay": false
}

// Response from /addresses/eth
{
  "address": "0x1234567890123456789012345678901234567890"
}
```

## The Core Issue

1. **Different API Signatures**: Pioneer SDK expects GET requests with `{network}/{xpub}` parameters, but we implement POST requests with `addressNList` arrays

2. **Different Input Format**: Pioneer SDK passes:
   - `network` (string like "bitcoin", "ethereum")
   - `xpub` (extended public key string)
   
   We expect:
   - `addressNList` (array of BIP44 path components)
   - `curve` (cryptographic curve)
   - `showDisplay` (boolean)

3. **Different Output Format**: Pioneer SDK expects index numbers for address derivation, we return actual addresses

## Pioneer SDK Startup Flow (21 Paths)

During startup, Pioneer SDK tries to get pubkeys for these paths:
```javascript
[
  // Bitcoin accounts (legacy, segwit, native segwit)
  "Bitcoin legacy account 0", "Bitcoin account 0 Segwit", "Bitcoin account 0 Native Segwit",
  "Bitcoin legacy account 1", "Bitcoin account 1 Segwit", "Bitcoin account 1 Native Segwit", 
  "Bitcoin legacy account 2", "Bitcoin account 2 Segwit", "Bitcoin account 2 Native Segwit",
  
  // Ethereum
  "ETH primary (default)",
  
  // Cosmos ecosystem
  "Default COCAO path", "Default RUNE path", "Default ATOM path", "Default OSMO path",
  
  // Other cryptocurrencies  
  "Bitcoin Cash Default", "Litecoin Default", "Litecoin Native Segwit", "Dogecoin Default",
  "Default dash path", "Default ripple path"
]
```

Each path requires a call to get the public key/xpub, then potentially calls to get addresses.

## Required Solutions

### Option 1: Add Pioneer SDK Compatible Endpoints (Recommended)

Add new endpoints that match Pioneer SDK expectations:

```rust
// In src-tauri/src/server/api/pioneer_compat.rs
#[get("/getNewAddress/{network}/{xpub}")]
async fn get_new_address(network: String, xpub: String) -> Result<Json<AddressIndexResponse>, ApiError> {
    // Convert xpub to addressNList
    // Call existing address generation logic
    // Return index instead of full address
}

#[get("/getChangeAddress/{network}/{xpub}")]  
async fn get_change_address(network: String, xpub: String) -> Result<Json<ChangeAddressResponse>, ApiError> {
    // Similar to above but for change addresses
}

#[get("/getPubkeyBalance/{asset}/{pubkey}")]
async fn get_pubkey_balance(asset: String, pubkey: String) -> Result<Json<BalanceResponse>, ApiError> {
    // Get balance for a given public key
    // May need to integrate with external APIs
}
```

### Option 2: Modify Pioneer SDK (Not Recommended)

We could fork Pioneer SDK to match our API, but this breaks compatibility with existing Pioneer-compatible apps.

### Option 3: Add API Translation Layer

Create a reverse proxy/translation layer that converts Pioneer SDK calls to our format.

## Implementation Plan

### Phase 1: Add Missing Core Endpoints
1. **`/getNewAddress/{network}/{xpub}`** - Convert xpub to addressNList, generate address, return index
2. **`/getChangeAddress/{network}/{xpub}`** - Similar but for change addresses  
3. **`/getPubkeyBalance/{asset}/{pubkey}`** - Return balance for pubkey (may return 0 initially)

### Phase 2: Network Mapping
Map Pioneer network names to our implementations:
- `"bitcoin"` → `/addresses/utxo` with Bitcoin params
- `"ethereum"` → `/addresses/eth`
- `"cosmos"` → `/addresses/cosmos`
- `"osmosis"` → `/addresses/osmosis`
- etc.

### Phase 3: XPub Parsing
Implement xpub parsing to extract:
- Network type (from xpub prefix)
- Chain code and key material
- Convert to appropriate `addressNList` format

### Phase 4: Address Index Management
- Track next available receive/change indices
- Store in database or memory
- Return proper index values

## Files to Modify

1. **`src-tauri/src/server/api/mod.rs`** - Add new route module
2. **`src-tauri/src/server/api/pioneer_compat.rs`** - New endpoint implementations
3. **`src-tauri/src/server/api/types.rs`** - Add Pioneer-compatible response types
4. **`src-tauri/Cargo.toml`** - Add xpub parsing dependencies if needed

## Testing Plan

1. **Unit Tests**: Test xpub parsing and network mapping
2. **Integration Tests**: Test endpoints with actual Pioneer SDK calls
3. **Pioneer SDK Integration**: Test full startup flow with 21 paths
4. **Load Test**: Ensure endpoints handle multiple simultaneous requests

## Success Criteria

✅ Pioneer SDK can successfully start up and get pubkeys for all 21 paths
✅ No 500 errors during Pioneer SDK initialization  
✅ All endpoints return proper JSON responses
✅ keepkey-vault can be used from vault.keepkey.com production site
✅ Existing keepkey-vault functionality remains unaffected

## Next Steps

1. **Immediate**: Implement `/getNewAddress/{network}/{xpub}` for Bitcoin and Ethereum
2. **Short-term**: Add remaining networks (Cosmos, Osmosis, etc.)
3. **Medium-term**: Add balance and transaction history endpoints
4. **Long-term**: Full Pioneer SDK compatibility for all features

---

*This analysis identifies the exact reason why Pioneer SDK fails during startup and provides a clear implementation path to fix the issue.* 