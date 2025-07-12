# Missing Endpoints Audit

Based on comparison with `/Users/highlander/WebstormProjects/keepkey-stack/projects/keepkey-desktop/packages/keepkey-desktop/build/api/swagger.json`

## Currently Implemented ✅

### Address Generation
- ✅ `/addresses/utxo` (POST)
- ✅ `/addresses/bnb` (POST)
- ✅ `/addresses/cosmos` (POST)
- ✅ `/addresses/osmosis` (POST)
- ✅ `/addresses/eth` (POST)
- ✅ `/addresses/tendermint` (POST)
- ✅ `/addresses/thorchain` (POST) - implemented as `/address/thorchain`
- ✅ `/addresses/mayachain` (POST)
- ✅ `/addresses/xrp` (POST)

### Authentication
- ✅ `/auth/pair` (GET) - Verify
- ✅ `/auth/pair` (POST) - Pair

### System
- ✅ `/api/health` (GET) - health check
- ✅ `/api/devices` (GET) - list devices
- ✅ `/system/info/get-features` (POST) - get features
- ✅ `/system/ping` (POST) - ping device
- ✅ `/system/info/get-entropy` (POST) - get entropy
- ✅ `/system/info/get-public-key` (POST) - get public key
- ✅ `/system/settings/apply` (POST) - apply settings
- ✅ `/system/clear-session` (POST) - clear session
- ✅ `/system/wipe-device` (POST) - wipe device

## Missing Endpoints ❌

### Transaction Signing - HIGH PRIORITY
- ❌ `/bnb/sign-transaction` (POST)
- ✅ `/cosmos/sign-amino` (POST) - IMPLEMENTED (placeholder)
- ❌ `/cosmos/sign-amino-delegate` (POST)
- ❌ `/cosmos/sign-amino-undelegate` (POST)
- ❌ `/cosmos/sign-amino-redelegate` (POST)
- ❌ `/cosmos/sign-amino-withdraw-delegator-rewards-all` (POST)
- ❌ `/cosmos/sign-amino-ibc-transfer` (POST)
- ✅ `/eth/signTransaction` (POST) - IMPLEMENTED (partial)
- ❌ `/eth/signTypedData` (POST)
- ✅ `/eth/sign` (POST) - IMPLEMENTED
- ❌ `/eth/verify` (POST)
- ❌ `/osmosis/sign-amino` (POST)
- ❌ `/osmosis/sign-amino-delegate` (POST)
- ❌ `/osmosis/sign-amino-undelegate` (POST)
- ❌ `/osmosis/sign-amino-redelegate` (POST)
- ❌ `/osmosis/sign-amino-withdraw-delegator-rewards-all` (POST)
- ❌ `/osmosis/sign-amino-ibc-transfer` (POST)
- ❌ `/osmosis/sign-amino-lp-remove` (POST)
- ❌ `/osmosis/sign-amino-lp-add` (POST)
- ❌ `/osmosis/sign-amino-swap` (POST)
- ❌ `/thorchain/sign-amino-transfer` (POST)
- ❌ `/thorchain/sign-amino-deposit` (POST)
- ✅ `/utxo/sign-transaction` (POST) - IMPLEMENTED (partial)
- ❌ `/xrp/sign-transaction` (POST)
- ❌ `/mayachain/sign-amino-transfer` (POST)
- ❌ `/mayachain/sign-amino-deposit` (POST)

### System Operations - MEDIUM PRIORITY
- ✅ `/system/ping` (POST) - IMPLEMENTED
- ✅ `/system/info/get-entropy` (POST) - IMPLEMENTED
- ✅ `/system/info/get-public-key` (POST) - IMPLEMENTED
- ❌ `/system/info/list-coins` (GET)
- ✅ `/system/settings/apply` (POST) - IMPLEMENTED
- ❌ `/system/policies/apply` (POST)
- ✅ `/system/clear-session` (POST) - IMPLEMENTED
- ✅ `/system/wipe-device` (POST) - IMPLEMENTED
- ❌ `/system/change-pin` (POST)
- ❌ `/system/change-wipe-code` (POST)
- ❌ `/system/sign-identity` (POST)
- ❌ `/system/cipher-key-value` (POST)

### Device Management - MEDIUM PRIORITY
- ❌ `/system/load-device` (POST)
- ❌ `/system/recover-device` (POST)
- ❌ `/system/reset-device` (POST)
- ❌ `/system/software-reset` (POST)

### Firmware Operations - LOW PRIORITY
- ❌ `/system/firmware/flash-hash` (POST)
- ❌ `/system/firmware/flash-write` (POST)
- ❌ `/system/firmware/update` (POST)

### Debug Operations - LOW PRIORITY
- ❌ `/system/debug/fill-config` (POST)
- ❌ `/system/debug/flash-dump` (POST)
- ❌ `/system/debug/get-state` (POST)

### Wallet Management - MEDIUM PRIORITY
- ❌ `/wallet/list` (GET)
- ❌ `/wallet/current` (GET)
- ❌ `/wallet/switch` (POST)

### Device Info - MEDIUM PRIORITY
- ❌ `/usb/list-devices` (GET)
- ❌ `/usb/state` (GET)
- ❌ `/info` (GET)

### Raw Protocol - LOW PRIORITY
- ❌ `/raw` (POST)

### AI Integration (Ollama) - NOT APPLICABLE?
- ❌ `/ollama/*` endpoints (multiple)

## Summary

- **Total Implemented**: 23 endpoints (13 original + 6 system + 4 transaction)
- **Total Missing**: 47+ endpoints
- **High Priority Missing**: 22 transaction signing endpoints (down from 26)
- **Medium Priority Missing**: 13 system/device endpoints
- **Low Priority Missing**: 12+ debug/firmware/other endpoints

## Recent Progress

### Round 1: System Operations (6 endpoints)
- `/system/ping` - Device ping with optional button protection
- `/system/info/get-entropy` - Generate random entropy
- `/system/info/get-public-key` - Get extended public key
- `/system/settings/apply` - Apply device settings
- `/system/clear-session` - Clear current session
- `/system/wipe-device` - Factory reset device

### Round 2: Transaction Signing (4 endpoints)
- `/utxo/sign-transaction` - Sign Bitcoin/UTXO transactions
- `/eth/signTransaction` - Sign Ethereum transactions (partial implementation)
- `/eth/sign` - Sign Ethereum messages
- `/cosmos/sign-amino` - Sign Cosmos Amino transactions (placeholder)

## Critical Missing for Vault Functionality

### Most Critical (Blocking Vault)
1. **`/system/info/get-public-key`** - Needed for getPubkeys() in pioneer-sdk ✅ DONE
2. **Transaction signing** - Basic signing endpoints ✅ PARTIAL
3. **Complete Bitcoin signing protocol** - Currently placeholder ❌ NEEDED
4. **Complete Ethereum signing protocol** - Currently placeholder ❌ NEEDED

### Important but not blocking
1. Complete Cosmos/Thorchain/Mayachain signing
2. XRP transaction signing
3. Device recovery/reset endpoints
4. Wallet management endpoints

## Next Steps

1. **Fix Bitcoin signing** - Implement full protocol in queue.rs
2. **Fix Ethereum signing** - Implement EthereumTxRequest/TxAck protocol
3. **Add remaining system operations** - PIN/policies/identity
4. **Consider which low-priority endpoints are actually needed**

## Notes

- All endpoints should use the device queue system
- No direct device communication
- Follow the existing pattern in address_operations.rs and system_operations.rs
- Update DeviceRequest/DeviceResponse enums for each new operation type
- Bitcoin/Ethereum signing require multi-step protocols (not simple request/response) 