# HTTPS Setup for KeepKey Vault REST API

## Quick Fix: Self-Signed Certificate

### 1. Generate Self-Signed Certificate
```bash
# Create certificates directory
mkdir -p src-tauri/certs
cd src-tauri/certs

# Generate private key and certificate
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes \
  -subj "/C=US/ST=State/L=City/O=KeepKey/CN=localhost"
```

### 2. Update Cargo.toml
Add TLS dependencies:
```toml
[dependencies]
axum-server = { version = "0.6", features = ["tls-rustls"] }
```

### 3. Update Server Code
```rust
use axum_server::tls_rustls::RustlsConfig;

pub async fn start_server(device_queue_manager: crate::commands::DeviceQueueManager, app_handle: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // ... existing code ...
    
    let addr = "127.0.0.1:1646";
    
    // Load TLS configuration
    let config = RustlsConfig::from_pem_file(
        "src-tauri/certs/cert.pem",
        "src-tauri/certs/key.pem"
    ).await?;
    
    // Use HTTPS
    axum_server::bind_rustls(addr.parse()?, config)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}
```

## Production Solution: Reverse Proxy

### Using Nginx
```nginx
server {
    listen 443 ssl;
    server_name vault-api.keepkey.com;
    
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    
    location / {
        proxy_pass http://127.0.0.1:1646;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        
        # CORS headers if needed
        add_header 'Access-Control-Allow-Origin' 'https://vault.keepkey.com' always;
        add_header 'Access-Control-Allow-Methods' 'GET, POST, OPTIONS' always;
        add_header 'Access-Control-Allow-Headers' 'DNT,User-Agent,X-Requested-With,If-Modified-Since,Cache-Control,Content-Type,Range' always;
    }
}
```

## Development Workarounds

### 1. Browser Flag (Chrome/Edge)
```bash
# Launch Chrome with disabled security (DEVELOPMENT ONLY)
open -n -a "Google Chrome" --args --disable-web-security --user-data-dir="/tmp/chrome_dev"
```

### 2. Browser Extension
Install "CORS Unblock" or similar extensions for development.

### 3. Local Development
Always use `http://localhost:1420` for local development instead of the production HTTPS domain.

## Security Notes

⚠️ **WARNING**: 
- Never use `--disable-web-security` in production
- Self-signed certificates will show browser warnings
- Always use proper certificates in production
- Mixed content blocking exists for security reasons 