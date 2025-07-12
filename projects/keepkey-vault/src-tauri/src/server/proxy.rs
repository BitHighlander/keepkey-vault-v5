use axum::{
    extract::{Query, Path as AxumPath},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::get,
    Router,
    body::Body,
};
use std::collections::HashMap;

pub fn create_proxy_router() -> Router {
    Router::new()
        .route("/", get(proxy_root_handler))
        .route("/*path", get(proxy_handler))
}

async fn proxy_root_handler(
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    // Always proxy to vault.keepkey.com for root requests
    proxy_vault_request("", params, headers).await
}

async fn proxy_handler(
    AxumPath(path): AxumPath<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    // Proxy all paths to vault.keepkey.com
    proxy_vault_request(&path, params, headers).await
}

async fn proxy_vault_request(
    path: &str,
    params: HashMap<String, String>,
    headers: HeaderMap,
) -> Response {
    // Build the target URL for vault.keepkey.com
    let target_url = if path.is_empty() {
        "https://vault.keepkey.com/".to_string()
    } else {
        format!("https://vault.keepkey.com/{}", path)
    };
    
    log::info!("🌐 PROXY: {} -> {}", path, target_url);
    
    // Create HTTP client with relaxed settings
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    
    // Build request
    let mut request = client.get(&target_url);
    
    // Add query parameters
    if !params.is_empty() {
        request = request.query(&params);
    }
    
    // Forward some headers (but not Host)
    for (name, value) in headers.iter() {
        let name_str = name.as_str();
        if !matches!(name_str, "host" | "connection" | "upgrade") {
            if let Ok(value_str) = value.to_str() {
                request = request.header(name_str, value_str);
            }
        }
    }
    
    // Make the request
    match request.send().await {
        Ok(response) => {
            // Convert reqwest::StatusCode to axum::http::StatusCode
            let status_code = match response.status().as_u16() {
                200 => StatusCode::OK,
                404 => StatusCode::NOT_FOUND,
                500 => StatusCode::INTERNAL_SERVER_ERROR,
                502 => StatusCode::BAD_GATEWAY,
                code => StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            };
            
            let response_headers = response.headers().clone();
            let body = match response.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    log::error!("Failed to read response body: {}", e);
                    return Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .body(Body::from("Failed to read response"))
                        .unwrap();
                }
            };

            // Check if this is HTML content and rewrite URLs
            let content_type = response_headers.get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            
            let final_body = if content_type.contains("text/html") {
                // Simple URL rewriting for vault.keepkey.com
                let html_content = String::from_utf8_lossy(&body);
                let rewritten_html = rewrite_vault_urls(&html_content);
                Body::from(rewritten_html.into_bytes())
            } else {
                Body::from(body)
            };
            
            // Build response
            let mut resp_builder = Response::builder().status(status_code);
            
            // Copy headers (except some security-related ones)
            for (name, value) in response_headers.iter() {
                let name_str = name.as_str();
                if !matches!(
                    name_str, 
                    "content-security-policy" | 
                    "x-frame-options" | 
                    "strict-transport-security" |
                    "connection" |
                    "transfer-encoding"
                ) {
                    // Convert reqwest header types to axum header types
                    if let Ok(value_str) = value.to_str() {
                        resp_builder = resp_builder.header(name_str, value_str);
                    }
                }
            }
            
            // Add cache control to prevent caching issues
            resp_builder = resp_builder.header("cache-control", "no-cache");
            
            resp_builder
                .body(final_body)
                .unwrap()
        }
        Err(e) => {
            log::error!("Proxy request failed: {}", e);
            Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!("Proxy error: {}", e)))
                .unwrap()
        }
    }
}

fn rewrite_vault_urls(html: &str) -> String {
    let mut result = html.to_string();
    
    // Add base tag to ensure all relative URLs resolve to our proxy
    if let Some(head_pos) = result.find("<head>") {
        let insert_pos = head_pos + "<head>".len();
        result.insert_str(insert_pos, r#"
    <base href="http://localhost:8080/">"#);
    }
    
    // Rewrite absolute URLs to vault.keepkey.com to point to our proxy
    result = result.replace("https://vault.keepkey.com/", "http://localhost:8080/");
    result = result.replace("https://vault.keepkey.com", "http://localhost:8080");
    
    // Rewrite relative URLs that start with /
    result = result.replace("href=\"/", "href=\"http://localhost:8080/");
    result = result.replace("src=\"/", "src=\"http://localhost:8080/");
    result = result.replace("action=\"/", "action=\"http://localhost:8080/");
    
    // Also handle single quotes
    result = result.replace("href='/", "href='http://localhost:8080/");
    result = result.replace("src='/", "src='http://localhost:8080/");
    result = result.replace("action='/", "action='http://localhost:8080/");
    
    // Rewrite JavaScript fetch/xhr calls
    result = result.replace("\"https://vault.keepkey.com", "\"http://localhost:8080");
    result = result.replace("'https://vault.keepkey.com", "'http://localhost:8080");
    
    result
} 