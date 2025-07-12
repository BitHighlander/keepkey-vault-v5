# KeepKey Vault REST API Debug Guide

## Architecture Overview

**KeepKey Vault** uses a dual-interface architecture:

### 1. **Frontend ↔ Backend Communication**
- **Technology:** Tauri `invoke()` commands (direct Rust function calls)
- **Protocol:** Native IPC, not HTTP
- **Port:** None (direct process communication)
- **Usage:** Internal frontend components

### 2. **External Applications ↔ REST API**
- **Technology:** Axum HTTP server
- **Protocol:** REST/JSON over HTTP  
- **Port:** `1646`
- **Usage:** External integrations, tools, MCP clients

## Quick Diagnostics

### 1. Check if REST API is Running
```bash
curl -s http://localhost:1646/api/health | jq
```

Expected response:
```json
{
  "status": "ok",
  "timestamp": "2024-01-01T12:00:00Z",
  "service": "KeepKey Vault API",
  "version": "2.0.0"
}
```

### 2. Test Basic Device Detection
```bash
curl -s http://localhost:1646/api/devices | jq
```

### 3. Run Comprehensive Test Suite
```bash
./test_api.sh -v
```

## Common Issues & Solutions

### Issue: Mixed Content Blocked
**Symptom:** Browser console shows blocked HTTP requests from HTTPS pages
```
[blocked] The page at https://vault.keepkey.com/ requested insecure content from http://localhost:1646/
```

**Root Cause:** External applications trying to access localhost HTTP from HTTPS context

**Solutions:**
1. **For Local Development:** Use HTTP version of your app
2. **For Production:** Implement HTTPS proxy or use browser extensions
3. **For MCP Clients:** Use localhost HTTP endpoints directly

### Issue: CORS Errors
**Symptom:** `Access-Control-Allow-Origin` errors in browser console

**Solution:** The CORS configuration now includes common development origins:
- `https://vault.keepkey.com` (production)
- `http://localhost:1420` (Tauri dev)
- `http://localhost:3000` (React dev)
- `http://localhost:5173` (Vite dev)

### Issue: Device Not Found
**Symptom:** API returns empty device list or 404 errors

**Troubleshooting Steps:**
1. Ensure KeepKey is connected via USB
2. Check device is unlocked and initialized
3. Verify Tauri backend is running (`ps aux | grep keepkey`)
4. Check device permissions (macOS: System Preferences > Privacy > USB)

### Issue: API Endpoint Not Found (404)
**Symptom:** Specific endpoints return 404

**Check Implementation Status:**
- ✅ `/api/health` - Health check
- ✅ `/api/devices` - List connected devices  
- ✅ `/system/info/get-features` - Device features
- ✅ `/auth/pair` - Authentication
- ⚠️ `/addresses/*` - Address generation (partial)
- ⚠️ `/system/*` - System operations (partial)

See `MISSING_ENDPOINTS.md` for full implementation status.

## Testing Tools

### 1. Automated Test Script
```bash
# Run basic tests
./test_api.sh

# Run with verbose output
./test_api.sh -v

# Test specific endpoint manually
curl -X POST http://localhost:1646/auth/pair \
  -H "Content-Type: application/json" \
  -d '{"name":"Test App","url":"http://localhost","image_url":""}'
```

### 2. API Documentation
- **Swagger UI:** http://localhost:1646/docs
- **OpenAPI Spec:** http://localhost:1646/api-docs/openapi.json

### 3. MCP Testing
```bash
# Test MCP endpoint
curl -X POST http://localhost:1646/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
```

## Development Workflow

### 1. Start Development Environment
```bash
# Terminal 1: Start Tauri app
cd projects/keepkey-rust/projects/keepkey-vault
cargo tauri dev

# Terminal 2: Test REST API
./test_api.sh
```

### 2. Monitor Logs
```bash
# Tauri logs show both frontend and backend activity
# Backend server logs show API requests
```

### 3. Test External Integration
```bash
# Example: Test from another application
curl -X POST http://localhost:1646/addresses/utxo \
  -H "Content-Type: application/json" \
  -d '{
    "address_n": [2147483692, 2147483648, 2147483648, 0, 0],
    "coin": "Bitcoin", 
    "script_type": "p2pkh",
    "show_display": false
  }'
```

## Debugging Checklist

### Backend Issues
- [ ] Tauri app is running (`cargo tauri dev`)
- [ ] REST server started (check startup logs)
- [ ] Port 1646 is not blocked by firewall
- [ ] Device is connected and accessible

### Frontend Issues  
- [ ] Frontend uses `invoke()`, not HTTP requests
- [ ] Check browser console for Tauri IPC errors
- [ ] Verify Tauri commands are properly exposed

### External Integration Issues
- [ ] CORS headers are set correctly
- [ ] API endpoints are implemented
- [ ] Authentication is handled if required
- [ ] Content-Type headers are correct

## Performance Considerations

### REST API Response Times
- **Health check:** < 10ms
- **Device list:** < 100ms  
- **Address generation:** < 2s (requires device interaction)
- **Transaction signing:** < 30s (requires user confirmation)

### Rate Limiting
Currently no rate limiting is implemented. For production use, consider:
- Request rate limiting per IP
- Device operation queuing
- Timeout handling for device operations

## Security Notes

### Local Only
The REST API binds to `127.0.0.1:1646` (localhost only) for security:
- Not accessible from external networks
- Requires local access to the machine
- Suitable for local development and MCP clients

### Authentication
Currently implements basic pairing authentication:
- Pair requests generate API keys
- Keys should be included in Authorization headers
- Production deployments should enhance this

### Device Access
- Requires exclusive USB access to KeepKey
- Multiple applications cannot use device simultaneously
- Device operations require user physical confirmation

## Support & Troubleshooting

### Debug Logs
Enable verbose logging:
```bash
RUST_LOG=debug cargo tauri dev
```

### Common Command Line Tools
```bash
# Check if port is in use
lsof -i :1646

# Test network connectivity
nc -zv localhost 1646

# Monitor HTTP requests
tcpdump -i lo0 port 1646
```

### Get Help
- **Documentation:** `/docs` in this repository
- **API Docs:** http://localhost:1646/docs (when running)
- **Issues:** Create GitHub issue with reproduction steps 