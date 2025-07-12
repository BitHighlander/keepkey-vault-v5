use axum::{
    extract::{Host, Path as AxumPath, Query, Request},
    http::{HeaderMap, Method, StatusCode},
    response::Response,
    routing::{any, delete, get, head, options, patch, post, put},
    Router,
    body::Body,
};
use std::collections::HashMap;
use reqwest;
use serde_json;
use regex::Regex;
use url;

/// Create the proxy router with wildcard *.keepkey.com support
pub fn create_proxy_router() -> Router {
    Router::new()
        .route("/", get(proxy_root_handler).post(proxy_root_post_handler))
        .route("/*path", get(proxy_handler).post(proxy_post_handler).put(proxy_put_handler).delete(proxy_delete_handler).patch(proxy_patch_handler).options(proxy_options_handler).head(proxy_head_handler))
        .fallback(proxy_fallback_handler)
}

/// Handle GET requests to the root path
async fn proxy_root_handler(
    Host(host): Host,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let target_domain = determine_target_domain(&host, &headers);
    log::info!("🌐 PROXY ROOT GET: / -> {}", target_domain);
    proxy_keepkey_request("", Method::GET, params, headers, None, &target_domain).await
}

/// Handle POST requests to the root path
async fn proxy_root_post_handler(
    Host(host): Host,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    let target_domain = determine_target_domain(&host, &headers);
    log::info!("🌐 PROXY ROOT POST: / -> {}", target_domain);
    let body = extract_body(request).await;
    proxy_keepkey_request("", Method::POST, params, headers, body, &target_domain).await
}

/// Handle GET requests to any path
async fn proxy_handler(
    Host(host): Host,
    AxumPath(path): AxumPath<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let target_domain = determine_target_domain(&host, &headers);
    log::info!("🌐 PROXY GET: /{} -> {}/{}", path, target_domain, path);
    proxy_keepkey_request(&path, Method::GET, params, headers, None, &target_domain).await
}

/// Handle POST requests to any path
async fn proxy_post_handler(
    Host(host): Host,
    AxumPath(path): AxumPath<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    let target_domain = determine_target_domain(&host, &headers);
    log::info!("🌐 PROXY POST: /{} -> {}/{}", path, target_domain, path);
    let body = extract_body(request).await;
    proxy_keepkey_request(&path, Method::POST, params, headers, body, &target_domain).await
}

/// Handle PUT requests to any path
async fn proxy_put_handler(
    Host(host): Host,
    AxumPath(path): AxumPath<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    let target_domain = determine_target_domain(&host, &headers);
    log::info!("🌐 PROXY PUT: /{} -> {}/{}", path, target_domain, path);
    let body = extract_body(request).await;
    proxy_keepkey_request(&path, Method::PUT, params, headers, body, &target_domain).await
}

/// Handle DELETE requests to any path
async fn proxy_delete_handler(
    Host(host): Host,
    AxumPath(path): AxumPath<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    let target_domain = determine_target_domain(&host, &headers);
    log::info!("🌐 PROXY DELETE: /{} -> {}/{}", path, target_domain, path);
    let body = extract_body(request).await;
    proxy_keepkey_request(&path, Method::DELETE, params, headers, body, &target_domain).await
}

/// Handle PATCH requests to any path
async fn proxy_patch_handler(
    Host(host): Host,
    AxumPath(path): AxumPath<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    let target_domain = determine_target_domain(&host, &headers);
    log::info!("🌐 PROXY PATCH: /{} -> {}/{}", path, target_domain, path);
    let body = extract_body(request).await;
    proxy_keepkey_request(&path, Method::PATCH, params, headers, body, &target_domain).await
}

/// Handle OPTIONS requests to any path
async fn proxy_options_handler(
    Host(host): Host,
    AxumPath(path): AxumPath<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let target_domain = determine_target_domain(&host, &headers);
    log::info!("🌐 PROXY OPTIONS: /{} -> {}/{}", path, target_domain, path);
    proxy_keepkey_request(&path, Method::OPTIONS, params, headers, None, &target_domain).await
}

/// Handle HEAD requests to any path
async fn proxy_head_handler(
    Host(host): Host,
    AxumPath(path): AxumPath<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let target_domain = determine_target_domain(&host, &headers);
    log::info!("🌐 PROXY HEAD: /{} -> {}/{}", path, target_domain, path);
    proxy_keepkey_request(&path, Method::HEAD, params, headers, None, &target_domain).await
}

/// Fallback handler for any method/path combination
async fn proxy_fallback_handler(
    request: Request,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path();
    let headers = request.headers().clone();
    
    // Extract host from headers if not available from extractor
    let host = headers.get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost:8080");
    
    let target_domain = determine_target_domain(host, &headers);
    log::info!("🌐 PROXY FALLBACK: {} {} -> {}{}", method, path, target_domain, path);
    
    let query_params = extract_query_params(uri.query());
    let body = extract_body(request).await;
    
    proxy_keepkey_request(path.trim_start_matches('/'), method, query_params, headers, body, &target_domain).await
}

/// Determine the target KeepKey domain based on routing rules with wildcard support
fn determine_target_domain(host: &str, headers: &HeaderMap) -> String {
    // Check for explicit subdomain routing in headers
    if let Some(target_header) = headers.get("x-keepkey-target") {
        if let Ok(target) = target_header.to_str() {
            if is_valid_keepkey_domain(target) {
                return format!("https://{}", target);
            }
        }
    }
    
    // Parse the incoming host to determine target subdomain
    let host_clean = host.split(':').next().unwrap_or(host); // Remove port if present
    
    // Check if the request is for a specific subdomain pattern
    if let Some(subdomain) = extract_keepkey_subdomain(host_clean) {
        return format!("https://{}.keepkey.com", subdomain);
    }
    
    // Check for wildcard subdomain in query params (for development)
    if let Some(subdomain_header) = headers.get("x-keepkey-subdomain") {
        if let Ok(subdomain) = subdomain_header.to_str() {
            if is_valid_subdomain(subdomain) {
                return format!("https://{}.keepkey.com", subdomain);
            }
        }
    }
    
    // Default routing - spec consistent
    "https://keepkey.com".to_string()
}

/// Extract subdomain from host if it follows KeepKey patterns (true wildcard support)
fn extract_keepkey_subdomain(host: &str) -> Option<String> {
    // Handle localhost with subdomain simulation for development
    if host.starts_with("localhost") || host.starts_with("127.0.0.1") {
        // Revert to spec-consistent default for local development
        return Some("vault".to_string());
    }
    
    // Handle actual subdomain requests (for when deployed)
    // Pattern: subdomain.keepkey.local or subdomain.keepkey.dev (for development)
    if host.ends_with(".keepkey.local") || host.ends_with(".keepkey.dev") {
        let parts: Vec<&str> = host.split('.').collect();
        if parts.len() >= 3 {
            return Some(parts[0].to_string());
        }
    }
    
    // Handle production patterns: any subdomain of keepkey.com
    if host.ends_with(".keepkey.com") {
        let parts: Vec<&str> = host.split('.').collect();
        if parts.len() >= 3 {
            // Extract the subdomain (everything before .keepkey.com)
            let subdomain_parts = &parts[..parts.len()-2];
            if !subdomain_parts.is_empty() {
                return Some(subdomain_parts.join("."));
            }
        }
    }
    
    None
}

/// Validate that a domain is a legitimate KeepKey domain (wildcard support)
fn is_valid_keepkey_domain(domain: &str) -> bool {
    // Use regex to match *.keepkey.com pattern
    lazy_static::lazy_static! {
        static ref KEEPKEY_DOMAIN_REGEX: Regex = Regex::new(r"^([a-zA-Z0-9-]+\.)*keepkey\.com$").unwrap();
    }
    
    // Check exact match for root domain
    if domain == "keepkey.com" {
        return true;
    }
    
    // Check wildcard pattern *.keepkey.com
    KEEPKEY_DOMAIN_REGEX.is_match(domain)
}

/// Validate subdomain name
fn is_valid_subdomain(subdomain: &str) -> bool {
    // Basic validation for subdomain names
    lazy_static::lazy_static! {
        static ref SUBDOMAIN_REGEX: Regex = Regex::new(r"^[a-zA-Z0-9-]+$").unwrap();
    }
    
    !subdomain.is_empty() && 
    subdomain.len() <= 63 && 
    !subdomain.starts_with('-') && 
    !subdomain.ends_with('-') &&
    SUBDOMAIN_REGEX.is_match(subdomain)
}

/// Extract body from request with size limit
async fn extract_body(request: Request) -> Option<Vec<u8>> {
    const MAX_BODY_SIZE: usize = 10 * 1024 * 1024; // 10MB limit
    match axum::body::to_bytes(request.into_body(), MAX_BODY_SIZE).await {
        Ok(bytes) => if bytes.is_empty() { None } else { Some(bytes.to_vec()) },
        Err(e) => {
            log::warn!("Failed to extract request body (可能 exceeding size limit): {}", e);
            None
        }
    }
}

/// Extract query parameters from query string
fn extract_query_params(query: Option<&str>) -> HashMap<String, String> {
    query.map(|q| {
        url::form_urlencoded::parse(q.as_bytes())
            .into_owned()
            .collect()
    }).unwrap_or_default()
}

/// Core proxy function that handles all requests to *.keepkey.com domains
async fn proxy_keepkey_request(
    path: &str,
    method: Method,
    params: HashMap<String, String>,
    headers: HeaderMap,
    body: Option<Vec<u8>>,
    target_domain: &str,
) -> Response {
    // Build the target URL
    let target_url = if path.is_empty() {
        format!("{}/", target_domain)
    } else {
        format!("{}/{}", target_domain, path)
    };
    
    log::debug!("🔄 Proxying {} {} -> {}", method, path, target_url);
    
    // Create HTTP client with appropriate settings
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(false) // Use proper SSL validation for production
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("KeepKey-Vault-Proxy/2.0")
        .build()
        .unwrap();
    
    // Convert axum Method to reqwest Method
    let reqwest_method = match method {
        Method::GET => reqwest::Method::GET,
        Method::POST => reqwest::Method::POST,
        Method::PUT => reqwest::Method::PUT,
        Method::DELETE => reqwest::Method::DELETE,
        Method::PATCH => reqwest::Method::PATCH,
        Method::HEAD => reqwest::Method::HEAD,
        Method::OPTIONS => reqwest::Method::OPTIONS,
        _ => reqwest::Method::GET,
    };
    
    // Build request
    let mut request = client.request(reqwest_method, &target_url);
    
    // Add query parameters
    if !params.is_empty() {
        request = request.query(&params);
    }
    
    // Forward appropriate headers (exclude problematic ones)
    for (name, value) in headers.iter() {
        let name_str = name.as_str().to_lowercase();
        if !is_hop_by_hop_header(&name_str) && !is_problematic_header(&name_str) {
            if let Ok(value_str) = value.to_str() {
                // Special handling for Host header - set it to target domain
                if name_str == "host" {
                    let target_host = target_domain.trim_start_matches("https://").trim_start_matches("http://");
                    request = request.header("host", target_host);
                } else {
                    request = request.header(name.as_str(), value_str);
                }
            }
        }
    }
    
    // Add body for POST/PUT/PATCH requests
    if let Some(body_bytes) = body {
        request = request.body(body_bytes);
    }
    
    // Make the request
    match request.send().await {
        Ok(response) => {
            log::debug!("✅ Proxy response: {} {}", response.status(), target_url);
            convert_response_to_axum(response, target_domain).await
        }
        Err(e) => {
            log::error!("❌ Proxy request failed for {}: {}", target_url, e);
            create_error_response(StatusCode::BAD_GATEWAY, &format!("Proxy error: {}", e))
        }
    }
}

/// Convert reqwest Response to axum Response
async fn convert_response_to_axum(response: reqwest::Response, target_domain: &str) -> Response {
    // Convert status code
    let status_code = match response.status().as_u16() {
        200 => StatusCode::OK,
        201 => StatusCode::CREATED,
        204 => StatusCode::NO_CONTENT,
        301 => StatusCode::MOVED_PERMANENTLY,
        302 => StatusCode::FOUND,
        304 => StatusCode::NOT_MODIFIED,
        400 => StatusCode::BAD_REQUEST,
        401 => StatusCode::UNAUTHORIZED,
        403 => StatusCode::FORBIDDEN,
        404 => StatusCode::NOT_FOUND,
        405 => StatusCode::METHOD_NOT_ALLOWED,
        429 => StatusCode::TOO_MANY_REQUESTS,
        500 => StatusCode::INTERNAL_SERVER_ERROR,
        502 => StatusCode::BAD_GATEWAY,
        503 => StatusCode::SERVICE_UNAVAILABLE,
        504 => StatusCode::GATEWAY_TIMEOUT,
        code => StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
    };
    
    let response_headers = response.headers().clone();
    let content_type = response_headers.get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    
    // Get response body
    let body_bytes = match response.bytes().await {
        Ok(b) => b,
        Err(e) => {
            log::error!("Failed to read response body: {}", e);
            return create_error_response(StatusCode::BAD_GATEWAY, "Failed to read response body");
        }
    };

    // Process HTML content for URL rewriting
    let final_body = if content_type.contains("text/html") {
        let html_content = String::from_utf8_lossy(&body_bytes);
        let rewritten_html = rewrite_keepkey_urls(&html_content, target_domain);
        Body::from(rewritten_html.into_bytes())
    } else if content_type.contains("javascript") || content_type.contains("json") {
        // Also rewrite JavaScript and JSON responses that might contain URLs
        let content = String::from_utf8_lossy(&body_bytes);
        let rewritten_content = rewrite_js_urls(&content, target_domain);
        Body::from(rewritten_content.into_bytes())
    } else {
        Body::from(body_bytes)
    };
    
    // Build response with appropriate headers
    let mut resp_builder = Response::builder().status(status_code);
    
    // Copy safe headers from the original response
    for (name, value) in response_headers.iter() {
        let name_str = name.as_str().to_lowercase();
        if !is_hop_by_hop_header(&name_str) && !is_security_header(&name_str) {
            if let Ok(value_str) = value.to_str() {
                resp_builder = resp_builder.header(name.as_str(), value_str);
            }
        }
    }
    
    // Add proxy-specific headers
    resp_builder = resp_builder
        .header("cache-control", "no-cache, no-store, must-revalidate")
        .header("x-proxy-by", "keepkey-vault")
        .header("x-proxy-target", target_domain)
        .header("access-control-allow-origin", "*")
        .header("access-control-allow-methods", "GET, POST, PUT, DELETE, OPTIONS, PATCH")
        .header("access-control-allow-headers", "content-type, authorization, x-requested-with, x-keepkey-target, x-keepkey-subdomain");
    
    resp_builder.body(final_body).unwrap()
}

/// Rewrite URLs in HTML content to point to our proxy for all KeepKey domains (wildcard support with subdomain preservation)
fn rewrite_keepkey_urls(html: &str, target_domain: &str) -> String {
    let mut result = html.to_string();
    let proxy_base = "http://localhost:8080";
    // Extract subdomain from target for preservation
    let subdomain = target_domain.trim_start_matches("https://").split('.').next().unwrap_or("");
    let proxy_base_with_sub = if subdomain.is_empty() { proxy_base.to_string() } else { format!("{}/{}", proxy_base, subdomain) };
    // Add base tag
    if let Some(head_pos) = result.find("<head>") {
        let insert_pos = head_pos + "<head>".len();
        result.insert_str(insert_pos, &format!(r#"
    <base href="{}/"/>
    <meta name="proxy-rewritten" content="keepkey-vault"/>
    <meta name="proxy-target" content="{}"/>"#, proxy_base_with_sub, target_domain));
    }
    // Enhanced regex to capture and preserve subdomain
    lazy_static::lazy_static! {
        static ref KEEPKEY_URL_REGEX: Regex = Regex::new(r"https?://((?:[a-zA-Z0-9-]+\.)*)keepkey\.com").unwrap();
    }
    result = KEEPKEY_URL_REGEX.replace_all(&result, |caps: &regex::Captures| {
        let sub = &caps[1];
        if sub.is_empty() {
            proxy_base.to_string()
        } else {
            format!("{}/{}", proxy_base, sub.trim_end_matches('.'))
        }
    }).to_string();
    // Rewrite relative URLs that start with /
    result = rewrite_attribute_urls(&result, "href", proxy_base);
    result = rewrite_attribute_urls(&result, "src", proxy_base);
    result = rewrite_attribute_urls(&result, "action", proxy_base);
    
    // Rewrite common API patterns for any KeepKey subdomain
    result = rewrite_keepkey_api_calls(&result, proxy_base);
    
    log::debug!("🔄 Rewrote HTML URLs for KeepKey proxy compatibility (wildcard)");
    result
}

/// Rewrite JavaScript/JSON content for KeepKey domains (wildcard support with subdomain preservation)
fn rewrite_js_urls(content: &str, target_domain: &str) -> String {
    let mut result = content.to_string();
    let proxy_base = "http://localhost:8080";
    // Extract subdomain
    let subdomain = target_domain.trim_start_matches("https://").split('.').next().unwrap_or("");
    // Enhanced regex to capture subdomain
    lazy_static::lazy_static! {
        static ref KEEPKEY_JS_REGEX: Regex = Regex::new(r#"["']https?://((?:[a-zA-Z0-9-]+\.)*)keepkey\.com([^"']*)["']"#).unwrap();
    }
    result = KEEPKEY_JS_REGEX.replace_all(&result, |caps: &regex::Captures| {
        let quote = &caps[0][0..1];
        let sub = &caps[1];
        let path = &caps[2];
        let proxy_path = if sub.is_empty() {
            format!("{}{}", proxy_base, path)
        } else {
            format!("{}/{}{}", proxy_base, sub.trim_end_matches('.'), path)
        };
        format!("{}{}{}", quote, proxy_path, quote)
    }).to_string();
    log::debug!("🔄 Rewrote JavaScript URLs for KeepKey proxy compatibility (wildcard)");
    result
}

/// Rewrite API calls for KeepKey domains
fn rewrite_keepkey_api_calls(html: &str, proxy_base: &str) -> String {
    let mut result = html.to_string();
    
    // Rewrite fetch calls
    result = result.replace("fetch(\"/", &format!("fetch(\"{}/", proxy_base));
    result = result.replace("fetch('/", &format!("fetch('{}/", proxy_base));
    
    // Rewrite XMLHttpRequest calls
    result = result.replace(".open(\"GET\", \"/", &format!(".open(\"GET\", \"{}/", proxy_base));
    result = result.replace(".open('GET', '/", &format!(".open('GET', '{}/", proxy_base));
    result = result.replace(".open(\"POST\", \"/", &format!(".open(\"POST\", \"{}/", proxy_base));
    result = result.replace(".open('POST', '/", &format!(".open('POST', '{}/", proxy_base));
    
    // Rewrite axios calls
    result = result.replace("axios.get(\"/", &format!("axios.get(\"{}/", proxy_base));
    result = result.replace("axios.get('/", &format!("axios.get('{}/", proxy_base));
    result = result.replace("axios.post(\"/", &format!("axios.post(\"{}/", proxy_base));
    result = result.replace("axios.post('/", &format!("axios.post('{}/", proxy_base));
    
    result
}

/// Rewrite specific HTML attributes
fn rewrite_attribute_urls(html: &str, attribute: &str, proxy_base: &str) -> String {
    let mut result = html.to_string();
    
    // Handle double quotes
    let pattern_double = format!("{}=\"/", attribute);
    let replacement_double = format!("{}=\"{}/", attribute, proxy_base);
    result = result.replace(&pattern_double, &replacement_double);
    
    // Handle single quotes
    let pattern_single = format!("{}='/", attribute);
    let replacement_single = format!("{}='{}/", attribute, proxy_base);
    result = result.replace(&pattern_single, &replacement_single);
    
    result
}

/// Check if header is a hop-by-hop header that shouldn't be forwarded
fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(name, 
        "connection" | "keep-alive" | "proxy-authenticate" | 
        "proxy-authorization" | "te" | "trailers" | "transfer-encoding" | "upgrade"
    )
}

/// Check if header is problematic for proxying
fn is_problematic_header(name: &str) -> bool {
    matches!(name, 
        "content-length" | "content-encoding" |
        "accept-encoding" // Let the client handle encoding
    )
}

/// Check if header is a security header that should be filtered
fn is_security_header(name: &str) -> bool {
    matches!(name,
        "content-security-policy" | "x-frame-options" | 
        "strict-transport-security" | "x-xss-protection" |
        "x-content-type-options" | "referrer-policy"
    )
}

/// Create a standardized error response
fn create_error_response(status: StatusCode, message: &str) -> Response {
    let error_body = serde_json::json!({
        "error": "KeepKey Proxy Error",
        "message": message,
        "status": status.as_u16(),
        "proxy": "keepkey-vault",
        "wildcard_support": "*.keepkey.com",
        "default_target": "keepkey.com",
        "examples": [
            "keepkey.com",
            "vault.keepkey.com",
            "app.keepkey.com", 
            "api.keepkey.com",
            "bridge.keepkey.com",
            "support.keepkey.com",
            "docs.keepkey.com",
            "any-subdomain.keepkey.com"
        ]
    });
    
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("access-control-allow-origin", "*")
        .header("x-proxy-error", "true")
        .header("x-proxy-by", "keepkey-vault")
        .header("x-wildcard-support", "*.keepkey.com")
        .header("x-default-target", "keepkey.com")
        .body(Body::from(error_body.to_string()))
        .unwrap()
} 