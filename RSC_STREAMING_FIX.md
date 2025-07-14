# RSC Streaming Fix for KeepKey Vault Proxy

## Problem Description

The KeepKey Vault production build was getting stuck on a loading spinner when accessed through the proxy at `http://localhost:8080`. This was happening because:

1. **React Server Components (RSC) streaming** was being broken by the proxy
2. The proxy was **buffering entire responses** instead of streaming them
3. RSC relies on **chunked transfer encoding** to stream data progressively
4. The app would show a loading spinner indefinitely waiting for data that never arrived

## Root Cause

In `src-tauri/src/server/proxy.rs`, the `convert_response_to_axum` function was calling:

```rust
let body_bytes = response.bytes().await?;
```

This line **buffers the entire response** in memory before processing it, which breaks RSC streaming that requires **real-time data transmission**.

## Solution

### 1. Added RSC Detection
The proxy now detects streaming responses by checking for:
- `text/x-component` content type (RSC)
- `text/plain` content type (common for streaming)
- `application/x-ndjson` content type (newline-delimited JSON)
- `transfer-encoding: chunked` header
- Next.js specific headers (`x-nextjs-stream`, `x-nextjs-page`)

### 2. Implemented Streaming Passthrough
For detected streaming responses, the proxy:
- **Streams data directly** without buffering using `response.bytes_stream()`
- **Preserves critical headers** like `transfer-encoding: chunked`
- **Skips URL rewriting** (not needed for RSC data)
- **Maintains connection integrity**

### 3. Preserved Existing Functionality
For non-streaming responses (HTML, CSS, JS), the proxy continues to:
- Buffer and process content
- Rewrite URLs for proxy compatibility
- Apply security headers

## Code Changes

### Modified Files:
- `src-tauri/src/server/proxy.rs` - Added streaming detection and passthrough
- `src-tauri/Cargo.toml` - Added `stream` feature to reqwest dependency

### Key Functions Added:
```rust
// Detects RSC streaming responses
let is_rsc_stream = content_type.contains("text/x-component") || 
                   content_type.contains("text/plain") ||
                   response_headers.get("transfer-encoding")
                       .and_then(|v| v.to_str().ok())
                       .map(|v| v.contains("chunked"))
                       .unwrap_or(false);

// Streams response directly without buffering
fn stream_response_directly(response: reqwest::Response, ...) -> Response {
    let body_stream = response.bytes_stream();
    let body = Body::from_stream(body_stream);
    // ... preserve streaming headers
}
```

## Testing

### Automated Test
Run the test script to verify the fix:
```bash
./test_rsc_streaming.sh
```

### Manual Testing
1. Start KeepKey Vault: `cargo tauri dev`
2. Open `http://localhost:8080/` in browser
3. Open Developer Tools (F12)
4. Check Console tab for errors
5. **Success**: No "Connection closed" errors appear
6. **Success**: App loads beyond the loading spinner

## Technical Details

### Before the Fix:
```
Browser → Proxy → vault.keepkey.com (RSC stream)
                ↓
        [BUFFERING] ← Breaks streaming
                ↓
        Browser (stuck loading)
```

### After the Fix:
```
Browser → Proxy → vault.keepkey.com (RSC stream)
                ↓
        [STREAMING] ← Preserves real-time data
                ↓
        Browser (loads successfully)
```

## Impact

- ✅ **Fixed**: Production build loading issues
- ✅ **Preserved**: All existing proxy functionality
- ✅ **Improved**: Better handling of modern web frameworks
- ✅ **Compatible**: Works with Next.js, React Server Components, and other streaming technologies

## Future Considerations

This fix makes the proxy compatible with:
- React Server Components (RSC)
- Next.js App Router streaming
- Server-Sent Events (SSE)
- Any chunked transfer encoding responses

The proxy now properly handles both traditional buffered responses and modern streaming responses, making it future-proof for evolving web technologies. 