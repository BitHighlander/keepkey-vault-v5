use crate::commands::{DeviceRequest, DeviceResponse, parse_derivation_path};
use keepkey_rust::device_queue::DeviceQueueHandle;
use crate::cache::{CacheManager, types::CachedPubkey};
use std::sync::Arc;

/// Process all address generation requests
pub async fn process_address_request(
    queue_handle: &DeviceQueueHandle,
    request: &DeviceRequest,
    request_id: &str,
    device_id: &str,
) -> Result<DeviceResponse, String> {
    let start_time = std::time::Instant::now();
    
    // Log the incoming request with details
    match request {
        DeviceRequest::EthereumGetAddress { path, .. } => {
            log::info!("🔷 EthereumGetAddress request - Path: {}", path);
        }
        DeviceRequest::CosmosGetAddress { path, hrp, .. } => {
            log::info!("🌌 CosmosGetAddress request - Path: {}, HRP: {}", path, hrp);
        }
        DeviceRequest::OsmosisGetAddress { path, .. } => {
            log::info!("💧 OsmosisGetAddress request - Path: {}", path);
        }
        DeviceRequest::ThorchainGetAddress { path, testnet, .. } => {
            log::info!("⚡ ThorchainGetAddress request - Path: {}, Testnet: {}", path, testnet);
        }
        DeviceRequest::MayachainGetAddress { path, .. } => {
            log::info!("🌿 MayachainGetAddress request - Path: {}", path);
        }
        DeviceRequest::XrpGetAddress { path, .. } => {
            log::info!("💱 XrpGetAddress request - Path: {}", path);
        }
        _ => {}
    }
    
    let result = match request {
        DeviceRequest::GetAddress { path, coin_name, script_type, show_display } => {
            // Delegate to system operations for generic GetAddress requests
            return crate::device::system_operations::process_system_request(queue_handle, request, request_id, device_id).await;
        }
        DeviceRequest::EthereumGetAddress { path, show_display } => {
            let path_parts = parse_derivation_path(path)?;
            let path_str = format!("m/{}", path_parts.iter()
                .map(|&n| if n & 0x80000000 != 0 {
                    format!("{}'", n & 0x7FFFFFFF)
                } else {
                    n.to_string()
                })
                .collect::<Vec<_>>()
                .join("/"));
            
            log::info!("  → Parsed path: {}", path_str);
            
            let msg = keepkey_rust::messages::EthereumGetAddress {
                address_n: path_parts,
                show_display: *show_display,
                ..Default::default()
            };
            
            let response = queue_handle
                .send_raw(msg.into(), false)
                .await
                .map_err(|e| format!("Failed to get Ethereum address: {}", e))?;
                
            match response {
                keepkey_rust::messages::Message::EthereumAddress(address) => {
                    let elapsed = start_time.elapsed();
                    let addr = &hex::encode(&address.address);
                    log::info!("  ✅ EthereumGetAddress completed in {:.3}s - Address: 0x{}...{}", 
                        elapsed.as_secs_f64(),
                        &addr[..6.min(addr.len())],
                        if addr.len() > 6 { &addr[addr.len()-4..] } else { "" });
                    Ok(DeviceResponse::EthereumAddress {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        address: format!("0x{}", hex::encode(&address.address)),
                        path: path.to_string(),
                        success: true,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Failure(failure) => {
                    let elapsed = start_time.elapsed();
                    log::warn!("  ❌ EthereumGetAddress failed in {:.3}s: {}", 
                        elapsed.as_secs_f64(),
                        failure.message.as_ref().unwrap_or(&"Unknown error".to_string()));
                    Ok(DeviceResponse::EthereumAddress {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        address: String::new(),
                        path: path.to_string(),
                        success: false,
                        error: Some(format!("Device returned error: {}", failure.message.unwrap_or_default())),
                    })
                }
                _ => Err("Unexpected response from device for Ethereum address request".to_string()),
            }
        }
        
        DeviceRequest::CosmosGetAddress { path, hrp: _, show_display } => {
            let path_parts = parse_derivation_path(path)?;
            let path_str = format!("m/{}", path_parts.iter()
                .map(|&n| if n & 0x80000000 != 0 {
                    format!("{}'", n & 0x7FFFFFFF)
                } else {
                    n.to_string()
                })
                .collect::<Vec<_>>()
                .join("/"));
            
            log::info!("  → Parsed path: {}", path_str);
            
            let msg = keepkey_rust::messages::CosmosGetAddress {
                address_n: path_parts,
                show_display: *show_display,
                ..Default::default()
            };
            
            let response = queue_handle
                .send_raw(msg.into(), false)
                .await
                .map_err(|e| format!("Failed to get Cosmos address: {}", e))?;
                
            match response {
                keepkey_rust::messages::Message::CosmosAddress(address) => {
                    let elapsed = start_time.elapsed();
                    let default_addr = String::new();
                    let addr = address.address.as_ref().unwrap_or(&default_addr);
                    log::info!("  ✅ CosmosGetAddress completed in {:.3}s - Address: {}...{}", 
                        elapsed.as_secs_f64(),
                        &addr[..10.min(addr.len())],
                        if addr.len() > 10 { &addr[addr.len()-6..] } else { "" });
                    Ok(DeviceResponse::CosmosAddress {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        path: path.to_string(),
                        address: address.address.unwrap_or_default(),
                        success: true,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Failure(failure) => {
                    let elapsed = start_time.elapsed();
                    log::warn!("  ❌ CosmosGetAddress failed in {:.3}s: {}", 
                        elapsed.as_secs_f64(),
                        failure.message.as_ref().unwrap_or(&"Unknown error".to_string()));
                    Ok(DeviceResponse::CosmosAddress {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        path: path.to_string(),
                        address: String::new(),
                        success: false,
                        error: Some(format!("Device returned error: {}", failure.message.unwrap_or_default())),
                    })
                }
                _ => Err("Unexpected response from device for Cosmos address request".to_string()),
            }
        }
        
        DeviceRequest::OsmosisGetAddress { path, show_display } => {
            let path_parts = parse_derivation_path(path)?;
            let path_str = format!("m/{}", path_parts.iter()
                .map(|&n| if n & 0x80000000 != 0 {
                    format!("{}'", n & 0x7FFFFFFF)
                } else {
                    n.to_string()
                })
                .collect::<Vec<_>>()
                .join("/"));
            
            log::info!("  → Parsed path: {}", path_str);
            
            let msg = keepkey_rust::messages::OsmosisGetAddress {
                address_n: path_parts,
                show_display: *show_display,
                ..Default::default()
            };
            
            let response = queue_handle
                .send_raw(msg.into(), false)
                .await
                .map_err(|e| format!("Failed to get Osmosis address: {}", e))?;
                
            match response {
                keepkey_rust::messages::Message::OsmosisAddress(address) => {
                    let elapsed = start_time.elapsed();
                    let default_addr = String::new();
                    let addr = address.address.as_ref().unwrap_or(&default_addr);
                    log::info!("  ✅ OsmosisGetAddress completed in {:.3}s - Address: {}...{}", 
                        elapsed.as_secs_f64(),
                        &addr[..10.min(addr.len())],
                        if addr.len() > 10 { &addr[addr.len()-6..] } else { "" });
                    Ok(DeviceResponse::OsmosisAddress {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        path: path.to_string(),
                        address: address.address.unwrap_or_default(),
                        success: true,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Failure(failure) => {
                    let elapsed = start_time.elapsed();
                    log::warn!("  ❌ OsmosisGetAddress failed in {:.3}s: {}", 
                        elapsed.as_secs_f64(),
                        failure.message.as_ref().unwrap_or(&"Unknown error".to_string()));
                    Ok(DeviceResponse::OsmosisAddress {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        path: path.to_string(),
                        address: String::new(),
                        success: false,
                        error: Some(format!("Device returned error: {}", failure.message.unwrap_or_default())),
                    })
                }
                _ => Err("Unexpected response from device for Osmosis address request".to_string()),
            }
        }
        
        DeviceRequest::ThorchainGetAddress { path, testnet, show_display } => {
            let path_parts = parse_derivation_path(path)?;
            let path_str = format!("m/{}", path_parts.iter()
                .map(|&n| if n & 0x80000000 != 0 {
                    format!("{}'", n & 0x7FFFFFFF)
                } else {
                    n.to_string()
                })
                .collect::<Vec<_>>()
                .join("/"));
            
            log::info!("  → Parsed path: {}", path_str);
            
            let msg = keepkey_rust::messages::ThorchainGetAddress {
                address_n: path_parts,
                testnet: Some(*testnet),
                show_display: *show_display,
                ..Default::default()
            };
            
            let response = queue_handle
                .send_raw(msg.into(), false)
                .await
                .map_err(|e| format!("Failed to get Thorchain address: {}", e))?;
                
            match response {
                keepkey_rust::messages::Message::ThorchainAddress(address) => {
                    let elapsed = start_time.elapsed();
                    let default_addr = String::new();
                    let addr = address.address.as_ref().unwrap_or(&default_addr);
                    log::info!("  ✅ ThorchainGetAddress completed in {:.3}s - Address: {}...{}", 
                        elapsed.as_secs_f64(),
                        &addr[..10.min(addr.len())],
                        if addr.len() > 10 { &addr[addr.len()-6..] } else { "" });
                    Ok(DeviceResponse::ThorchainAddress {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        path: path.to_string(),
                        address: address.address.unwrap_or_default(),
                        success: true,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Failure(failure) => {
                    let elapsed = start_time.elapsed();
                    log::warn!("  ❌ ThorchainGetAddress failed in {:.3}s: {}", 
                        elapsed.as_secs_f64(),
                        failure.message.as_ref().unwrap_or(&"Unknown error".to_string()));
                    Ok(DeviceResponse::ThorchainAddress {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        path: path.to_string(),
                        address: String::new(),
                        success: false,
                        error: Some(format!("Device returned error: {}", failure.message.unwrap_or_default())),
                    })
                }
                _ => Err("Unexpected response from device for Thorchain address request".to_string()),
            }
        }
        
        DeviceRequest::MayachainGetAddress { path, show_display } => {
            let path_parts = parse_derivation_path(path)?;
            let path_str = format!("m/{}", path_parts.iter()
                .map(|&n| if n & 0x80000000 != 0 {
                    format!("{}'", n & 0x7FFFFFFF)
                } else {
                    n.to_string()
                })
                .collect::<Vec<_>>()
                .join("/"));
            
            log::info!("  → Parsed path: {}", path_str);
            
            let msg = keepkey_rust::messages::MayachainGetAddress {
                address_n: path_parts,
                show_display: *show_display,
                ..Default::default()
            };
            
            let response = queue_handle
                .send_raw(msg.into(), false)
                .await
                .map_err(|e| format!("Failed to get Mayachain address: {}", e))?;
                
            match response {
                keepkey_rust::messages::Message::MayachainAddress(address) => {
                    let elapsed = start_time.elapsed();
                    let default_addr = String::new();
                    let addr = address.address.as_ref().unwrap_or(&default_addr);
                    log::info!("  ✅ MayachainGetAddress completed in {:.3}s - Address: {}...{}", 
                        elapsed.as_secs_f64(),
                        &addr[..10.min(addr.len())],
                        if addr.len() > 10 { &addr[addr.len()-6..] } else { "" });
                    Ok(DeviceResponse::MayachainAddress {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        path: path.to_string(),
                        address: address.address.unwrap_or_default(),
                        success: true,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Failure(failure) => {
                    let elapsed = start_time.elapsed();
                    log::warn!("  ❌ MayachainGetAddress failed in {:.3}s: {}", 
                        elapsed.as_secs_f64(),
                        failure.message.as_ref().unwrap_or(&"Unknown error".to_string()));
                    Ok(DeviceResponse::MayachainAddress {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        path: path.to_string(),
                        address: String::new(),
                        success: false,
                        error: Some(format!("Device returned error: {}", failure.message.unwrap_or_default())),
                    })
                }
                _ => Err("Unexpected response from device for Mayachain address request".to_string()),
            }
        }
        
        DeviceRequest::XrpGetAddress { path, show_display } => {
            let path_parts = parse_derivation_path(path)?;
            let path_str = format!("m/{}", path_parts.iter()
                .map(|&n| if n & 0x80000000 != 0 {
                    format!("{}'", n & 0x7FFFFFFF)
                } else {
                    n.to_string()
                })
                .collect::<Vec<_>>()
                .join("/"));
            
            log::info!("  → Parsed path: {}", path_str);
            
            let msg = keepkey_rust::messages::RippleGetAddress {
                address_n: path_parts,
                show_display: *show_display,
                ..Default::default()
            };
            
            let response = queue_handle
                .send_raw(msg.into(), false)
                .await
                .map_err(|e| format!("Failed to get Ripple address: {}", e))?;
                
            match response {
                keepkey_rust::messages::Message::RippleAddress(address) => {
                    let elapsed = start_time.elapsed();
                    let default_addr = String::new();
                    let addr = address.address.as_ref().unwrap_or(&default_addr);
                    log::info!("  ✅ XrpGetAddress completed in {:.3}s - Address: {}...{}", 
                        elapsed.as_secs_f64(),
                        &addr[..10.min(addr.len())],
                        if addr.len() > 10 { &addr[addr.len()-6..] } else { "" });
                    Ok(DeviceResponse::XrpAddress {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        path: path.to_string(),
                        address: address.address.unwrap_or_default(),
                        success: true,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Failure(failure) => {
                    let elapsed = start_time.elapsed();
                    log::warn!("  ❌ XrpGetAddress failed in {:.3}s: {}", 
                        elapsed.as_secs_f64(),
                        failure.message.as_ref().unwrap_or(&"Unknown error".to_string()));
                    Ok(DeviceResponse::XrpAddress {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        path: path.to_string(),
                        address: String::new(),
                        success: false,
                        error: Some(format!("Device returned error: {}", failure.message.unwrap_or_default())),
                    })
                }
                _ => Err("Unexpected response from device for Ripple address request".to_string()),
            }
        }
        
        _ => Err("Not an address operation request".to_string()),
    };
    
    result
}

/// Process address requests with cache support
pub async fn process_address_request_with_cache(
    cache: &Arc<CacheManager>,
    queue_handle: &DeviceQueueHandle,
    request: &DeviceRequest,
    request_id: &str,
    device_id: &str,
) -> Result<DeviceResponse, String> {
    let start_time = std::time::Instant::now();
    
    // Extract path and coin type for cache lookup
    let (path, coin_name, script_type) = match request {
        DeviceRequest::GetAddress { path, coin_name, script_type, .. } => (path.as_str(), coin_name.as_str(), script_type.as_deref()),
        DeviceRequest::EthereumGetAddress { path, .. } => (path.as_str(), "ethereum", Some("ethereum")),
        DeviceRequest::CosmosGetAddress { path, .. } => (path.as_str(), "cosmos", Some("cosmos")),
        DeviceRequest::OsmosisGetAddress { path, .. } => (path.as_str(), "osmosis", Some("bech32")),
        DeviceRequest::ThorchainGetAddress { path, .. } => (path.as_str(), "thorchain", Some("thorchain")),
        DeviceRequest::MayachainGetAddress { path, .. } => (path.as_str(), "mayachain", Some("mayachain")),
        DeviceRequest::XrpGetAddress { path, .. } => (path.as_str(), "ripple", Some("ripple")),
        _ => return process_address_request(queue_handle, request, request_id, device_id).await,
    };
    
    log::info!("🔍 [CACHE CHECK] {} Address - Path: {}", coin_name, path);
    
    // Check cache first
    if let Some(cached) = cache.get_cached_pubkey(device_id, path, coin_name, script_type).await {
        if cached.address.is_some() {
            let elapsed = start_time.elapsed();
            log::info!("✅ [CACHE HIT] {} Address - Path: {} - Retrieved from cache in {:.3}ms", 
                coin_name, path, elapsed.as_secs_f64() * 1000.0);
            
            // Convert cached data to appropriate response type
            return Ok(match request {
                DeviceRequest::GetAddress { path, .. } => DeviceResponse::Address {
                    request_id: request_id.to_string(),
                    device_id: device_id.to_string(),
                    path: path.to_string(),
                    address: cached.address.unwrap_or_default(),
                    success: true,
                    error: None,
                },
                DeviceRequest::EthereumGetAddress { path, .. } => DeviceResponse::EthereumAddress {
                    request_id: request_id.to_string(),
                    device_id: device_id.to_string(),
                    path: path.to_string(),
                    address: cached.address.unwrap_or_default(),
                    success: true,
                    error: None,
                },
                DeviceRequest::CosmosGetAddress { path, .. } => DeviceResponse::CosmosAddress {
                    request_id: request_id.to_string(),
                    device_id: device_id.to_string(),
                    path: path.to_string(),
                    address: cached.address.unwrap_or_default(),
                    success: true,
                    error: None,
                },
                DeviceRequest::OsmosisGetAddress { path, .. } => DeviceResponse::OsmosisAddress {
                    request_id: request_id.to_string(),
                    device_id: device_id.to_string(),
                    path: path.to_string(),
                    address: cached.address.unwrap_or_default(),
                    success: true,
                    error: None,
                },
                DeviceRequest::ThorchainGetAddress { path, .. } => DeviceResponse::ThorchainAddress {
                    request_id: request_id.to_string(),
                    device_id: device_id.to_string(),
                    path: path.to_string(),
                    address: cached.address.unwrap_or_default(),
                    success: true,
                    error: None,
                },
                DeviceRequest::MayachainGetAddress { path, .. } => DeviceResponse::MayachainAddress {
                    request_id: request_id.to_string(),
                    device_id: device_id.to_string(),
                    path: path.to_string(),
                    address: cached.address.unwrap_or_default(),
                    success: true,
                    error: None,
                },
                DeviceRequest::XrpGetAddress { path, .. } => DeviceResponse::XrpAddress {
                    request_id: request_id.to_string(),
                    device_id: device_id.to_string(),
                    path: path.to_string(),
                    address: cached.address.unwrap_or_default(),
                    success: true,
                    error: None,
                },
                _ => unreachable!(),
            });
        }
    }
    
    log::info!("❌ [CACHE MISS] {} Address - Path: {} - Fetching from device...", coin_name, path);
    
    // Cache miss - fetch from device
    let device_start = std::time::Instant::now();
    let response = process_address_request(queue_handle, request, request_id, device_id).await?;
    let device_elapsed = device_start.elapsed();
    
    log::info!("🔧 [DEVICE] {} Address completed in {:.3}s", coin_name, device_elapsed.as_secs_f64());
    
    // Extract address from response and save to cache
    let address = match &response {
        DeviceResponse::Address { address, .. } => Some(address.clone()),
        DeviceResponse::EthereumAddress { address, .. } => Some(address.clone()),
        DeviceResponse::CosmosAddress { address, .. } => Some(address.clone()),
        DeviceResponse::OsmosisAddress { address, .. } => Some(address.clone()),
        DeviceResponse::ThorchainAddress { address, .. } => Some(address.clone()),
        DeviceResponse::MayachainAddress { address, .. } => Some(address.clone()),
        DeviceResponse::XrpAddress { address, .. } => Some(address.clone()),
        _ => None,
    };
    
    if let Some(addr) = address {
        if !addr.is_empty() {
            let cached = CachedPubkey {
                id: None,
                device_id: device_id.to_string(),
                derivation_path: path.to_string(),
                coin_name: coin_name.to_string(),
                script_type: script_type.map(|s| s.to_string()),
                xpub: None,
                address: Some(addr),
                chain_code: None,
                public_key: None,
                cached_at: chrono::Utc::now().timestamp(),
                last_used: chrono::Utc::now().timestamp(),
            };
            
            let cache_start = std::time::Instant::now();
            if let Err(e) = cache.save_pubkey(&cached).await {
                log::error!("❌ [CACHE SAVE ERROR] Failed to cache {} address for path {}: {}", coin_name, path, e);
            } else {
                let cache_elapsed = cache_start.elapsed();
                log::info!("💾 [CACHE SAVED] {} Address - Path: {} - Saved to cache in {:.3}ms", 
                    coin_name, path, cache_elapsed.as_secs_f64() * 1000.0);
            }
        }
    }
    
    let total_elapsed = start_time.elapsed();
    log::info!("⏱️ [TOTAL TIME] {} Address - Path: {} - Total time: {:.3}s", 
        coin_name, path, total_elapsed.as_secs_f64());
    
    Ok(response)
} 