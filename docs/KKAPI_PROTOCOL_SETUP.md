# kkapi:// Protocol Setup Guide

This document explains how to use the custom `kkapi://` protocol to securely connect `https://vault.keepkey.com` to your local KeepKey Vault REST API without HTTPS certificate issues or mixed content errors.

## How It Works

```
┌──────────────┐   kkapi://…   ┌──────────────────┐   http://localhost:1646
│ WebView      │ ────────────▶ │ Tauri protocol   │ ──▶│ Local REST API │
│ (Vault site) │               │ handler (Rust)   │    └────────────────┘
└──────────────┘               └──────────────────┘
```

The Tauri app registers a custom `kkapi://` protocol that proxies all requests to `http://localhost:1646`, solving:
- ❌ Mixed content errors (HTTPS → HTTP)
- ❌ CORS issues
- ❌ Certificate requirements

## Implementation Details

### 1. Backend Protocol Handler

The Rust backend in `src-tauri/src/lib.rs` registers the protocol:

```rust
.register_uri_scheme_protocol("kkapi", |app, request| {
    // Rewrite kkapi://… → http://localhost:1646/…
    let proxied_url = request.uri().replace("kkapi://", "http://localhost:1646/");
    
    // Forward the request with CORS headers
    // ... (see implementation for full details)
})
```

### 2. Frontend Configuration

To use this with `https://vault.keepkey.com`, you need to configure the KeepKey SDK to use `kkapi://` instead of `http://localhost:1646`.

#### Option A: Browser Console Injection

Open browser DevTools and run:

```javascript
// Configure the SDK to use kkapi protocol
window.localStorage.setItem('keepkey-sdk-config', JSON.stringify({
  apiKey: '57dd3fa6-9344-4bc5-8a92-924629076018',
  pairingInfo: {
    name: 'KeepKey SDK Demo App',
    imageUrl: 'https://pioneers.dev/coins/keepkey.png',
    basePath: 'kkapi://spec/swagger.json',
    url: 'kkapi://'
  }
}));

// Reload the page
window.location.reload();
```

#### Option B: Tauri WebView Injection

From the Tauri app, inject configuration:

```rust
// In your Tauri setup
app.get_webview_window("main").unwrap().eval(
    "window.localStorage.setItem('keepkey-sdk-config', JSON.stringify({
        apiKey: '57dd3fa6-9344-4bc5-8a92-924629076018',
        pairingInfo: {
            name: 'KeepKey SDK Demo App',
            imageUrl: 'https://pioneers.dev/coins/keepkey.png',
            basePath: 'kkapi://spec/swagger.json',
            url: 'kkapi://'
        }
    }));"
).unwrap();
```

### 3. Testing the Protocol

The app includes a test button that:
1. Calls the Tauri test command
2. Makes a test fetch request to `kkapi://info`
3. Displays the result

## Configuration Files

### tauri.conf.json
```json
{
  "app": {
    "security": {
      "csp": null,
      "dangerousDisableAssetCspModification": true
    }
  }
}
```

### Cargo.toml
```toml
[dependencies]
reqwest = { version = "0.11", features = ["blocking"] }
```

## Usage Steps

1. **Build and run the Tauri app**:
   ```bash
   make vault-dev
   ```

2. **Test the protocol**: Click the "Test kkapi:// Protocol" button in the app

3. **Configure vault.keepkey.com**: Use one of the injection methods above

4. **Verify**: Check that requests to `kkapi://auth/pair` work without CORS errors

## Troubleshooting

### Protocol Not Registered
- Ensure Tauri configuration allows custom protocols
- Check that the protocol handler is registered in `lib.rs`

### CORS Errors
- Verify CORS headers are set in the protocol handler
- Check browser DevTools Network tab for response headers

### Connection Refused
- Ensure your local REST API is running on port 1646
- Check that the proxy URL rewriting is correct

## Security Notes

- The `kkapi://` protocol only works within the Tauri WebView
- All requests are proxied to localhost only
- No external network access via this protocol
- CORS headers are permissive for local development

## Next Steps

Once this is working, you can:
1. Add authentication to the protocol handler
2. Implement request/response logging
3. Add configuration for different API ports
4. Create automated injection scripts 