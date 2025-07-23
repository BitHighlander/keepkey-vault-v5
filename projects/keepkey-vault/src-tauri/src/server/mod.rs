pub mod routes;
pub mod context;
pub mod auth;
pub mod api;
pub mod proxy;
pub mod portfolio_unified;

use axum::{
    Router,
    serve,
    routing::{get, post},
    response::Json,
};

use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::info;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use tauri::Emitter;

pub struct ServerState {
    pub device_queue_manager: crate::commands::DeviceQueueManager,
    pub app_handle: tauri::AppHandle,
    pub cache_manager: std::sync::Arc<once_cell::sync::OnceCell<std::sync::Arc<crate::cache::CacheManager>>>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::health_check,
        // Context endpoints - commented out until full device interaction is implemented
        // routes::api_get_context,
        // routes::api_set_context,
        // routes::api_clear_context,
        routes::api_list_devices,
        routes::api_get_features,
        routes::mcp_handle,
        auth::auth_verify,
        auth::auth_pair,
        api::addresses::thorchain_get_address,
        api::addresses::utxo_get_address,
        api::addresses::binance_get_address,
        api::addresses::cosmos_get_address,
        api::addresses::osmosis_get_address,
        api::addresses::ethereum_get_address,
        api::addresses::tendermint_get_address,
        api::addresses::mayachain_get_address,
        api::addresses::xrp_get_address,
        api::system::system_ping,
        api::system::get_entropy,
        api::system::get_public_key,
        api::system::apply_settings,
        api::system::clear_session,
        api::system::wipe_device,
        api::system::exit_application,
        api::transactions::utxo_sign_transaction,
        api::transactions::eth_sign_transaction,
        api::transactions::eth_sign_message,
        api::transactions::cosmos_sign_amino,
        api::portfolio::get_combined_portfolio,
        api::portfolio::get_device_portfolio,
        api::portfolio::get_instant_portfolio,
        api::portfolio::get_portfolio_history,
    ),
    components(
        schemas(
            routes::HealthResponse,
            routes::DeviceInfo,
            routes::KeepKeyInfo,
            routes::Features,
            // Context schemas - commented out until needed
            // context::DeviceContext,
            // context::ContextResponse,
            // context::SetContextRequest,
            auth::PairingInfo,
            auth::AuthResponse,
            api::addresses::ThorchainAddressRequest,
            api::addresses::AddressRequest,
            api::addresses::AddressResponse,
            api::addresses::UtxoAddressRequest,
            api::system::PingRequest,
            api::system::PingResponse,
            api::system::GetEntropyRequest,
            api::system::GetEntropyResponse,
            api::system::GetPublicKeyRequest,
            api::system::GetPublicKeyResponse,
            api::system::ApplySettingsRequest,
            api::system::ApplySettingsResponse,
            api::system::ClearSessionResponse,
            api::system::WipeDeviceResponse,
            api::transactions::UtxoSignTransactionRequest,
            api::transactions::UtxoSignTransactionResponse,
            api::transactions::EthSignTransactionRequest,
            api::transactions::EthSignTransactionResponse,
            api::transactions::EthSignMessageRequest,
            api::transactions::EthSignMessageResponse,
            api::transactions::CosmosSignAminoRequest,
            api::transactions::CosmosSignAminoResponse,
            crate::commands::BitcoinUtxoInput,
            crate::commands::BitcoinUtxoOutput,
            api::portfolio::PortfolioQuery,
            api::portfolio::PortfolioResponse,
            crate::pioneer_api::PortfolioBalance,
            crate::pioneer_api::Dashboard,
            crate::pioneer_api::NetworkSummary,
            crate::pioneer_api::AssetSummary,
        )
    ),
    tags(
        (name = "system", description = "System health and status endpoints"),
        (name = "device", description = "Device management endpoints"),
        (name = "mcp", description = "Model Context Protocol endpoints"),
        (name = "auth", description = "Authentication and pairing endpoints"),
        (name = "addresses", description = "Address generation endpoints"),
        (name = "Transaction", description = "Transaction signing endpoints"),
        (name = "portfolio", description = "Portfolio management endpoints")
    ),
    info(
        title = "KeepKey Vault API",
        description = "REST API and MCP server for KeepKey device management (Bitcoin-only)",
        version = "2.0.0"
    )
)]
struct ApiDoc;

pub async fn start_server(device_queue_manager: crate::commands::DeviceQueueManager, app_handle: tauri::AppHandle, cache_manager: std::sync::Arc<once_cell::sync::OnceCell<std::sync::Arc<crate::cache::CacheManager>>>) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing if not already done
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "vault_v2=info,axum=info");
    }
    
    // Try to initialize tracing, ignore if already initialized
    let _ = tracing_subscriber::fmt::try_init();
    
    // Create server state
    let server_state = Arc::new(ServerState {
        device_queue_manager,
        app_handle: app_handle.clone(),
        cache_manager,
    });
    
    // Create Swagger UI
    let swagger_ui = SwaggerUi::new("/docs")
        .url("/api-docs/openapi.json", ApiDoc::openapi());
    
    // Create a handler for the OpenAPI spec that returns the same JSON
    let _openapi_spec = ApiDoc::openapi();
    
    // Build the router
    let app = Router::new()
        // System endpoints
        .route("/api/health", get(routes::health_check))
        
        // Add compatibility route for Pioneer SDK kkapi detection
        .route("/spec/swagger.json", get(|| async move {
            Json(ApiDoc::openapi())
        }))
        
        // Context endpoints - commented out until full device interaction is implemented
        // .route("/api/context", get(routes::api_get_context))
        // .route("/api/context", post(routes::api_set_context))
        // .route("/api/context", delete(routes::api_clear_context))
        
        // Device management endpoints
        .route("/api/devices", get(routes::api_list_devices))
        .route("/system/info/get-features", post(routes::api_get_features))
        
        // MCP endpoint - Model Context Protocol
        .route("/mcp", post(routes::mcp_handle))
        
        // Auth endpoints
        .route("/auth/pair", get(auth::auth_verify))
        .route("/auth/pair", post(auth::auth_pair))
        
        // Address endpoints
        .route("/addresses/thorchain", post(api::addresses::thorchain_get_address))
        .route("/addresses/utxo", post(api::addresses::utxo_get_address))
        .route("/addresses/bnb", post(api::addresses::binance_get_address))
        .route("/addresses/cosmos", post(api::addresses::cosmos_get_address))
        .route("/addresses/osmosis", post(api::addresses::osmosis_get_address))
        .route("/addresses/eth", post(api::addresses::ethereum_get_address))
        .route("/addresses/tendermint", post(api::addresses::tendermint_get_address))
        .route("/addresses/mayachain", post(api::addresses::mayachain_get_address))
        .route("/addresses/xrp", post(api::addresses::xrp_get_address))
        
        // System operation endpoints
        .route("/system/ping", post(api::system::system_ping))
        .route("/system/info/get-entropy", post(api::system::get_entropy))
        .route("/system/info/get-public-key", post(api::system::get_public_key))
        .route("/system/settings/apply", post(api::system::apply_settings))
        .route("/system/clear-session", post(api::system::clear_session))
        .route("/system/wipe-device", post(api::system::wipe_device))
        .route("/system/exit", post(api::system::exit_application))
        
        // Transaction signing endpoints
        .route("/utxo/sign-transaction", post(api::transactions::utxo_sign_transaction))
        .route("/eth/signTransaction", post(api::transactions::eth_sign_transaction))
        .route("/eth/sign", post(api::transactions::eth_sign_message))
        .route("/cosmos/sign-amino", post(api::transactions::cosmos_sign_amino))
        
        // Portfolio endpoints
        .route("/api/portfolio", get(api::portfolio::get_combined_portfolio))
        .route("/api/portfolio/:device_id", get(api::portfolio::get_device_portfolio))
        .route("/api/portfolio/instant/:device_id", get(api::portfolio::get_instant_portfolio))
        .route("/api/portfolio/history/:device_id", get(api::portfolio::get_portfolio_history))
        
        // Unified portfolio endpoint for all devices
        .route("/api/v1/portfolio/all", get(portfolio_unified::get_unified_portfolio))
        
        // Cache endpoints
        .route("/api/cache/status", get(api::cache::get_cache_status))
        
        // Pubkey batch endpoints for performance optimization
        .route("/api/pubkeys/batch", post(api::pubkeys::batch_get_pubkeys))
        
        // Wallet bootstrap endpoints for offline-first architecture
        .route("/api/v1/wallet/bootstrap", post(api::wallet::wallet_bootstrap))
        .route("/api/v1/health/fast", get(api::wallet::fast_health_check))
        
        // Merge swagger UI first
        .merge(swagger_ui)
        // Then add state and middleware
        .with_state(server_state)
        .layer(
            CorsLayer::new()
                // Allow any origin with wildcard
                .allow_origin(axum::http::header::HeaderValue::from_static("*"))
                // Allow all methods
                .allow_methods(tower_http::cors::Any)
                // Allow all headers
                .allow_headers(tower_http::cors::Any)
                // Note: credentials cannot be used with wildcard origin
                .allow_credentials(false)
        );
    
    let addr = "127.0.0.1:1646";
    let listener = TcpListener::bind(addr).await?;
    
    // Start the proxy server on port 8080 - ensure it's ready before continuing
    let proxy_addr = "127.0.0.1:8080";
    let proxy_app = proxy::create_proxy_router();
    let proxy_listener = TcpListener::bind(proxy_addr).await?;
    
    info!("🚀 Starting servers:");
    info!("  📋 REST API: http://{}/api", addr);
    info!("  📚 API Documentation: http://{}/docs", addr);
    info!("  🔌 Device Management: http://{}/api/devices", addr);
    info!("  🤖 MCP Endpoint: http://{}/mcp", addr);
    info!("  🔐 Authentication: http://{}/auth/pair", addr);
    info!("  🌐 Address Generation: http://{}/address/*", addr);
    info!("  🌍 Vault Proxy: http://{} -> vault.keepkey.com", proxy_addr);
    
    // Test proxy server readiness by making a quick health check
    let proxy_health_check = async {
        let client = reqwest::Client::new();
        let mut retries = 0;
        let max_retries = 10;
        
        loop {
            if retries >= max_retries {
                return Err("Proxy server failed to start within timeout".to_string());
            }
            
            match client.get(format!("http://{}/", proxy_addr)).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        log::info!("✅ Proxy server health check passed");
                        return Ok(());
                    }
                }
                Err(e) => {
                    log::warn!("⚠️ Proxy server not ready (attempt {}/{}): {}", retries + 1, max_retries, e);
                }
            }
            
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            retries += 1;
        }
    };
    
    // Start the proxy server and wait for it to be ready
    let proxy_handle = tokio::spawn(async move {
        serve(proxy_listener, proxy_app).await
    });
    
    // Small delay to let proxy server start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Check if proxy server is ready
    match proxy_health_check.await {
        Ok(()) => {
            info!("✅ Both servers started successfully and are ready");
            
            // Emit success event to frontend only after both servers are confirmed ready
            match app_handle.emit("server:ready", serde_json::json!({
                "status": "ready",
                "rest_url": format!("http://{}/docs", addr),
                "mcp_url": format!("http://{}/mcp", addr),
                "proxy_url": format!("http://{}", proxy_addr),
                "proxy_ready": true
            })) {
                Ok(_) => log::info!("✅ server:ready event emitted successfully"),
                Err(e) => log::error!("❌ Failed to emit server:ready event: {}", e),
            }
        }
        Err(e) => {
            log::error!("❌ CRITICAL: Proxy server failed to start: {}", e);
            
            // Emit error event to frontend
            match app_handle.emit("server:error", serde_json::json!({
                "error": format!("Proxy server failed to start: {}", e),
                "critical": true
            })) {
                Ok(_) => log::info!("✅ server:error event emitted successfully"),
                Err(emit_err) => log::error!("❌ Failed to emit server:error event: {}", emit_err),
            }
            
            return Err(e.into());
        }
    }
    
    // Monitor proxy server in the background
    tokio::spawn(async move {
        if let Err(e) = proxy_handle.await {
            log::error!("❌ Proxy server task failed: {}", e);
        }
    });
    
    // Run the main API server
    serve(listener, app).await?;
    
    Ok(())
} 