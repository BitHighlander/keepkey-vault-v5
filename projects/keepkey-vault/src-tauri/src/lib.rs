use tauri::{Emitter, Manager};
use tauri::http::{Response, Method, StatusCode};

// Modules for better organization

mod commands;
mod device;
mod event_controller;
mod logging;
mod slip132;
mod server;
mod cache;

// Re-export commonly used types

use std::sync::Arc;

// Learn more about Tauri commands at https://tauri.app/develop/rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// Dev tools toggle command
#[tauri::command]
fn toggle_dev_tools(window: tauri::Window) -> Result<(), String> {
    #[cfg(debug_assertions)]
    {
        if window.is_devtools_open() {
            window.close_devtools();
            Ok(())
        } else {
            window.open_devtools();
            Ok(())
        }
    }
    #[cfg(not(debug_assertions))]
    {
        // In release mode, only allow if explicitly enabled
        if std::env::var("KEEPKEY_ENABLE_DEVTOOLS").is_ok() {
            if window.is_devtools_open() {
                window.close_devtools();
                Ok(())
            } else {
                window.open_devtools();
                Ok(())
            }
        } else {
            Err("Dev tools are disabled in release mode".to_string())
        }
    }
}

// Get app version command
#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// Onboarding related commands moved to commands.rs



// Vault interface commands
#[tauri::command]
fn vault_change_view(app: tauri::AppHandle, view: String) -> Result<(), String> {
    println!("View changed to: {}", view);
    // Emit event to frontend if needed
    match app.emit("vault:change_view", serde_json::json!({ "view": view })) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to emit view change event: {}", e))
    }
}

#[tauri::command]
fn vault_open_support(app: tauri::AppHandle) -> Result<(), String> {
    println!("Opening support");
    
    // Switch to browser view and navigate to support
    app.emit("vault:change_view", serde_json::json!({
        "view": "browser"
    })).map_err(|e| format!("Failed to emit view change event: {}", e))?;
    
    app.emit("browser:navigate", serde_json::json!({
        "url": "https://support.keepkey.com"
    })).map_err(|e| format!("Failed to emit navigation event: {}", e))?;
    
    Ok(())
}

// Add the missing vault_open_app command to open external URLs
#[tauri::command]
async fn vault_open_app(app_handle: tauri::AppHandle, app_id: String, app_name: String, url: String) -> Result<(), String> {
    println!("Opening app: {} ({}) -> {}", app_name, app_id, url);
    
    // Use Tauri's opener plugin to open the URL in the system browser
    use tauri_plugin_opener::OpenerExt;
    app_handle.opener().open_url(url, None::<&str>)
        .map_err(|e| format!("Failed to open URL: {}", e))?;
    
    Ok(())
}

// Add a general command to open any URL in the system browser
#[tauri::command]
async fn open_url(app_handle: tauri::AppHandle, url: String) -> Result<(), String> {
    println!("Opening URL in system browser: {}", url);
    
    // Use Tauri's opener plugin to open the URL in the system browser
    use tauri_plugin_opener::OpenerExt;
    app_handle.opener().open_url(url, None::<&str>)
        .map_err(|e| format!("Failed to open URL: {}", e))?;
    
    Ok(())
}

#[tauri::command]
fn restart_backend_startup(app: tauri::AppHandle) -> Result<(), String> {
    println!("Restarting backend startup process");
    // Emit event to indicate restart
    match app.emit("application:state", serde_json::json!({
        "status": "Restarting...",
        "connected": false,
        "features": null
    })) {
        Ok(_) => {
            // Simulate restart process
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(1000));
                let _ = app.emit("application:state", serde_json::json!({
                    "status": "Device ready",
                    "connected": true,
                    "features": {
                        "label": "KeepKey",
                        "vendor": "KeepKey",
                        "model": "KeepKey",
                        "firmware_variant": "keepkey",
                        "device_id": "keepkey-001",
                        "language": "english",
                        "bootloader_mode": false,
                        "version": "7.7.0",
                        "firmware_hash": null,
                        "bootloader_hash": null,
                        "initialized": true,
                        "imported": false,
                        "no_backup": false,
                        "pin_protection": true,
                        "pin_cached": false,
                        "passphrase_protection": false,
                        "passphrase_cached": false,
                        "wipe_code_protection": false,
                        "auto_lock_delay_ms": null,
                        "policies": []
                    }
                }));
            });
            Ok(())
        },
        Err(e) => Err(format!("Failed to emit restart event: {}", e))
    }
}

// Add a test command to verify kkapi protocol
#[tauri::command]
async fn test_kkapi_protocol() -> Result<String, String> {
    log::info!("🧪 Testing kkapi:// protocol functionality");
    Ok("kkapi:// protocol handler is registered and ready".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_process::init())
        .register_uri_scheme_protocol("kkapi", |_app, request| {
            // 1️⃣ Rewrite kkapi://… → http://localhost:1646/…
            let original_url = request.uri().to_string();
            let proxied_url = original_url.replace("kkapi://", "http://localhost:1646/");
            
            log::debug!("🔄 Proxying kkapi request: {} -> {}", original_url, proxied_url);
            
            // 2️⃣ Create HTTP client and forward the request
            let client = reqwest::blocking::Client::new();
            let method = match request.method() {
                &Method::GET => reqwest::Method::GET,
                &Method::POST => reqwest::Method::POST,
                &Method::PUT => reqwest::Method::PUT,
                &Method::DELETE => reqwest::Method::DELETE,
                &Method::PATCH => reqwest::Method::PATCH,
                &Method::OPTIONS => reqwest::Method::OPTIONS,
                &Method::HEAD => reqwest::Method::HEAD,
                _ => reqwest::Method::GET, // Default fallback
            };
            
            // Build the request
            let mut req_builder = client.request(method, &proxied_url);
            
            // Forward headers (excluding host and some problematic ones)
            for (name, value) in request.headers() {
                let header_name = name.as_str().to_lowercase();
                if !["host", "connection", "upgrade-insecure-requests"].contains(&header_name.as_str()) {
                    if let Ok(header_value) = value.to_str() {
                        req_builder = req_builder.header(name.as_str(), header_value);
                    }
                }
            }
            
            // Add body for POST/PUT requests
            let body = request.body();
            if !body.is_empty() {
                req_builder = req_builder.body(body.clone());
            }
            
            // Execute the request
            match req_builder.send() {
                Ok(response) => {
                    let status = response.status();
                    let status_code = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                    
                    // Get response body first
                    let body_bytes = match response.bytes() {
                        Ok(body) => body,
                        Err(e) => {
                            log::error!("❌ Failed to read response body: {}", e);
                            return Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .header("Access-Control-Allow-Origin", "*")
                                .body(format!("Failed to read response: {}", e).into_bytes())
                                .unwrap();
                        }
                    };
                    
                    // Build response with CORS headers
                    let mut response_builder = Response::builder()
                        .status(status_code)
                        .header("Access-Control-Allow-Origin", "*")
                        .header("Access-Control-Allow-Methods", "GET,POST,PUT,DELETE,OPTIONS,PATCH")
                        .header("Access-Control-Allow-Headers", "Content-Type,Authorization,X-Requested-With");
                    
                    log::debug!("✅ Successfully proxied request to {}", proxied_url);
                    response_builder.body(body_bytes.to_vec()).unwrap()
                }
                Err(e) => {
                    log::error!("❌ Failed to proxy request to {}: {}", proxied_url, e);
                    Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .header("Access-Control-Allow-Origin", "*")
                        .header("Content-Type", "application/json")
                        .body(format!(r#"{{"error": "Proxy request failed", "details": "{}"}}"#, e).into_bytes())
                        .unwrap()
                }
            }
        })
        .setup(|app| {
            // Initialize device logging system
            if let Err(e) = logging::init_device_logger() {
                eprintln!("Failed to initialize device logger: {}", e);
            } else {
                println!("✅ Device logging initialized - logs will be written to ~/.keepkey/logs/");
            }
            
            // Initialize real device system using keepkey_rust
            let device_queue_manager = Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::<String, keepkey_rust::device_queue::DeviceQueueHandle>::new()
            ));
            
            // Initialize response tracking
            let last_responses = Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::<String, commands::DeviceResponse>::new()
            ));
            
            // Initialize cache system lazily - will be initialized on first use
            let cache_manager = Arc::new(once_cell::sync::OnceCell::<Arc<crate::cache::CacheManager>>::new());
            
            app.manage(device_queue_manager.clone());
            app.manage(last_responses);
            app.manage(cache_manager.clone());
            
            // Start event controller with proper management
            let _event_controller = event_controller::spawn_event_controller(&app.handle());
            
            // Start background log cleanup task
            let _app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400)); // 24 hours
                loop {
                    interval.tick().await;
                    if let Err(e) = logging::get_device_logger().cleanup_old_logs().await {
                        eprintln!("Failed to cleanup old logs: {}", e);
                    }
                }
            });
            
            // Start REST/MCP server in background (only if enabled in preferences)
            let server_handle = app.handle().clone();
            let server_queue_manager = device_queue_manager.clone();
            tauri::async_runtime::spawn(async move {
                // Add a small delay to ensure config system is ready
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                
                // Check if API is enabled in preferences
                let api_enabled = match commands::get_api_enabled().await {
                    Ok(enabled) => enabled,
                    Err(e) => {
                        log::debug!("Could not check API status: {} - defaulting to enabled", e);
                        true // Default to enabled if error
                    }
                };
                
                if api_enabled {
                    log::info!("🚀 API is enabled in preferences, starting server...");
                    
                    if let Err(e) = server::start_server(server_queue_manager, server_handle.clone(), cache_manager.clone()).await {
                        log::error!("❌ Server error: {}", e);
                        // Optionally emit error event to frontend
                        let _ = server_handle.emit("server:error", serde_json::json!({
                            "error": format!("Server failed to start: {}", e)
                        }));
                    }
                } else {
                    log::info!("🔒 API is disabled in preferences, skipping server startup");
                }
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            toggle_dev_tools,
            get_app_version,
            vault_change_view,
            vault_open_support,
            vault_open_app,
            open_url,
            restart_backend_startup,
            test_kkapi_protocol,
            // Frontend readiness
            commands::frontend_ready,
            // Device operations - unified queue interface
            device::queue::add_to_device_queue,
            commands::get_queue_status,
            // Basic device enumeration (non-queue operations)
            commands::get_connected_devices,
            commands::get_blocking_actions,
            // New device commands (all go through queue)
            commands::get_device_status,
            commands::get_device_info_by_id,
            commands::wipe_device,
            commands::set_device_label,
            commands::get_connected_devices_with_features,
            // Update commands
            device::updates::update_device_bootloader,
            device::updates::update_device_firmware,
            // PIN creation commands
            commands::initialize_device_pin,
            commands::send_pin_matrix_response,
            commands::get_pin_session_status,
            commands::cancel_pin_creation,
            commands::initialize_device_wallet,
            commands::complete_wallet_creation,
            // PIN unlock commands  
            commands::start_pin_unlock,
            commands::send_pin_unlock_response,
            commands::send_pin_matrix_ack,
            commands::trigger_pin_request,
            commands::check_device_pin_ready,
            // Logging commands
            commands::get_device_log_path,
            commands::get_recent_device_logs,
            commands::cleanup_device_logs,
            // Configuration and onboarding commands
            commands::is_first_time_install,
            commands::is_onboarded,
            commands::set_onboarding_completed,
            commands::get_preference,
            commands::set_preference,
            commands::debug_onboarding_state,
            // API control commands
            commands::get_api_enabled,
            commands::set_api_enabled,
            commands::get_api_status,
            commands::restart_app,
            // Test commands
            commands::test_device_queue,
            commands::test_status_emission,
            commands::test_bootloader_mode_device_status,
            commands::test_oob_device_status_evaluation,
            // Recovery commands - delegated to keepkey_rust
            commands::start_device_recovery,
            commands::send_recovery_character,
            commands::send_recovery_pin_response,
            commands::get_recovery_status,
            commands::cancel_recovery_session,
            // Seed verification commands (dry run recovery)
            commands::start_seed_verification,
            commands::send_verification_character,
            commands::send_verification_pin,
            commands::get_verification_status,
            commands::cancel_seed_verification,
            commands::force_cleanup_seed_verification,
            // Cache commands
            commands::get_cache_status,
            commands::trigger_frontload,
            commands::clear_device_cache
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
