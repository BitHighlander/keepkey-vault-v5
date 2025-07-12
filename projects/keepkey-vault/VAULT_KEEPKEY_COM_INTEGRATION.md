# Vault.keepkey.com Integration Guide

## Understanding the Architecture

`vault.keepkey.com` is designed to work with a **locally installed KeepKey application** that provides a REST API on `http://localhost:1646`.

### Current Flow:
1. User visits `https://vault.keepkey.com`
2. Website loads Pioneer SDK
3. Pioneer SDK tries to connect to `http://localhost:1646`
4. If **keepkey-desktop** is running → Success ✅
5. If **keepkey-vault** is running → Mixed content blocked ❌

## The Problem

**Mixed Content Blocking**: Browsers prevent HTTPS pages from making HTTP requests. This is a security feature that cannot be bypassed with CORS.

## Solutions

### Option 1: Run Both Apps Together (Development)

Run keepkey-desktop alongside keepkey-vault:
```bash
# Terminal 1: Run keepkey-desktop (provides API on :1646)
cd projects/keepkey-desktop
npm run dev

# Terminal 2: Run keepkey-vault (runs on :1420)  
cd projects/keepkey-rust/projects/keepkey-vault
cargo tauri dev
```

Then access keepkey-vault at `http://localhost:1420` (not vault.keepkey.com).

### Option 2: Make keepkey-vault Compatible

To make keepkey-vault work exactly like keepkey-desktop, you need to ensure the REST API endpoints match. The key endpoints Pioneer SDK expects:

```
GET  /auth/pair         - Verify pairing
POST /auth/pair         - Create pairing
GET  /docs              - API documentation
POST /system/info/get-features - Get device features
```

### Option 3: Production Deployment

For production, you need one of these approaches:

#### A. Distribute keepkey-vault
1. Build and sign the application
2. Distribute to users
3. Users install and run locally
4. vault.keepkey.com connects to local app

#### B. Use HTTPS API
1. Deploy API to `https://api.keepkey.com`
2. Update Pioneer SDK to use HTTPS endpoint
3. No mixed content issues

#### C. Browser Extension
Create a browser extension that:
1. Provides a bridge between HTTPS and local HTTP
2. Handles device communication securely

## Testing Locally

### 1. Verify API Compatibility
```bash
# Test keepkey-desktop API
curl http://localhost:1646/api/health

# Test keepkey-vault API  
curl http://localhost:1646/api/health
```

### 2. Compare Endpoints
Use the test script to verify all endpoints work the same:
```bash
skills/test_keepkey_vault_api/test_keepkey_vault_api.sh
```

### 3. Local Development
For local development, access your app directly:
- keepkey-vault: `http://localhost:1420`
- keepkey-desktop: `http://localhost:3000`

Don't use `https://vault.keepkey.com` for testing local changes.

## Making vault.keepkey.com Work

To make vault.keepkey.com work with your local keepkey-vault:

1. **Ensure API compatibility** - All endpoints must match keepkey-desktop
2. **Run on same port** - Change keepkey-vault to use port 1646
3. **Stop keepkey-desktop** - Only one app can use port 1646
4. **Use HTTP locally** - Visit `http://localhost:1420` instead

## Long-term Solution

The sustainable solution is to:

1. **Complete keepkey-vault development**
2. **Ensure API compatibility with keepkey-desktop**  
3. **Build and sign the application**
4. **Distribute to users**
5. **Update vault.keepkey.com to detect which app is running**

## Current Status

✅ keepkey-vault REST API works locally
✅ CORS configured for all origins
❌ Cannot bypass mixed content blocking
❌ keepkey-vault not yet distributed to users

## Next Steps

1. **For Development**: Use `http://localhost:1420` directly
2. **For Testing with vault.keepkey.com**: Run keepkey-desktop alongside
3. **For Production**: Plan distribution strategy 