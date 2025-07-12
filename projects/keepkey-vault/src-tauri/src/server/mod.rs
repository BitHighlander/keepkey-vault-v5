pub mod routes;
pub mod context;
pub mod auth;
pub mod api;

use axum::{
    Router,
    serve,
    routing::{get, post},
};

use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::info;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

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
        api::transactions::utxo_sign_transaction,
        api::transactions::eth_sign_transaction,
        api::transactions::eth_sign_message,
        api::transactions::cosmos_sign_amino,
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
        )
    ),
    tags(
        (name = "system", description = "System health and status endpoints"),
        (name = "device", description = "Device management endpoints"),
        (name = "mcp", description = "Model Context Protocol endpoints"),
        (name = "auth", description = "Authentication and pairing endpoints"),
        (name = "addresses", description = "Address generation endpoints"),
        (name = "Transaction", description = "Transaction signing endpoints")
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
        app_handle,
        cache_manager,
    });
    
    // Create Swagger UI
    let swagger_ui = SwaggerUi::new("/docs")
        .url("/api-docs/openapi.json", ApiDoc::openapi());
    
    // Build the router
    let app = Router::new()
        // System endpoints
        .route("/api/health", get(routes::health_check))
        
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
        
        // Transaction signing endpoints
        .route("/utxo/sign-transaction", post(api::transactions::utxo_sign_transaction))
        .route("/eth/signTransaction", post(api::transactions::eth_sign_transaction))
        .route("/eth/sign", post(api::transactions::eth_sign_message))
        .route("/cosmos/sign-amino", post(api::transactions::cosmos_sign_amino))
        
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
    
    info!("🚀 Server started successfully:");
    info!("  📋 REST API: http://{}/api", addr);
    info!("  📚 API Documentation: http://{}/docs", addr);
    info!("  🔌 Device Management: http://{}/api/devices", addr);
    info!("  🤖 MCP Endpoint: http://{}/mcp", addr);
    info!("  🔐 Authentication: http://{}/auth/pair", addr);
    info!("  🌐 Address Generation: http://{}/address/*", addr);
    
    // Spawn the server
    serve(listener, app).await?;
    
    Ok(())
} 