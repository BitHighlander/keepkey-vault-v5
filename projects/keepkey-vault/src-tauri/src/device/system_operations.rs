use crate::commands::{DeviceRequest, DeviceResponse, parse_derivation_path};
use keepkey_rust::features::DeviceFeatures;
use keepkey_rust::device_queue::DeviceQueueHandle;
use crate::cache::{CacheManager, CachedPubkey};
use std::sync::Arc;

/// Process all system operation requests
pub async fn process_system_request(
    queue_handle: &DeviceQueueHandle,
    request: &DeviceRequest,
    request_id: &str,
    device_id: &str,
) -> Result<DeviceResponse, String> {
    let start_time = std::time::Instant::now();
    
    // Log the incoming request with details
    match request {
        DeviceRequest::GetPublicKey { path, coin_name, script_type, .. } => {
            log::info!("🔑 GetPublicKey request - Path: {}, Coin: {:?}, ScriptType: {:?}", 
                path, coin_name, script_type);
        }
        DeviceRequest::GetAddress { path, coin_name, script_type, .. } => {
            log::info!("📍 GetAddress request - Path: {}, Coin: {}, ScriptType: {:?}", 
                path, coin_name, script_type);
        }
        _ => {}
    }
    
    let result = match request {
        DeviceRequest::GetFeatures => {
            log::info!("📋 GetFeatures request");
            let msg = keepkey_rust::messages::GetFeatures {
                ..Default::default()
            };
            
            let response = queue_handle
                .send_raw(msg.into(), false)
                .await
                .map_err(|e| format!("Failed to get features: {}", e))?;
                
            match response {
                keepkey_rust::messages::Message::Features(features) => {
                    let elapsed = start_time.elapsed();
                    log::info!("✅ GetFeatures completed in {:.3}s", elapsed.as_secs_f64());
                    Ok(DeviceResponse::Features {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        features: DeviceFeatures {
                            label: features.label.clone(),
                            vendor: features.vendor.clone(),
                            model: features.model.clone(),
                            firmware_variant: features.firmware_variant.clone(),
                            device_id: features.device_id.clone(),
                            language: features.language.clone(),
                            bootloader_mode: features.bootloader_mode.unwrap_or(false),
                            version: format!(
                                "{}.{}.{}",
                                features.major_version.unwrap_or(0),
                                features.minor_version.unwrap_or(0),
                                features.patch_version.unwrap_or(0)
                            ),
                            firmware_hash: features.firmware_hash.as_ref().map(|h| hex::encode(h)),
                            bootloader_hash: features.bootloader_hash.as_ref().map(|h| hex::encode(h)),
                            bootloader_version: features.bootloader_hash.as_ref().map(|h| hex::encode(h)),
                            initialized: features.initialized.unwrap_or(false),
                            imported: features.imported,
                            no_backup: features.no_backup.unwrap_or(false),
                            pin_protection: features.pin_protection.unwrap_or(false),
                            pin_cached: features.pin_cached.unwrap_or(false),
                            passphrase_protection: features.passphrase_protection.unwrap_or(false),
                            passphrase_cached: features.passphrase_cached.unwrap_or(false),
                            wipe_code_protection: features.wipe_code_protection.unwrap_or(false),
                            auto_lock_delay_ms: features.auto_lock_delay_ms.map(|ms| ms as u64),
                            policies: features.policies.iter()
                                .filter(|p| p.enabled.unwrap_or(false))
                                .map(|p| p.policy_name.clone().unwrap_or_default())
                                .collect(),
                        },
                        success: true,
                        error: None,
                    })
                }
                _ => Err("Unexpected response from device".to_string()),
            }
        }
        
        DeviceRequest::GetAddress { path, coin_name, script_type, show_display } => {
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
            
            let msg = keepkey_rust::messages::GetAddress {
                address_n: path_parts,
                coin_name: Some(coin_name.clone()),
                script_type: script_type.as_ref().and_then(|st| match st.as_str() {
                    "p2pkh" => Some(keepkey_rust::messages::InputScriptType::Spendaddress as i32),
                    "p2sh" => Some(keepkey_rust::messages::InputScriptType::Spendmultisig as i32),
                    "p2wpkh" => Some(keepkey_rust::messages::InputScriptType::Spendwitness as i32),
                    "p2sh-p2wpkh" => Some(keepkey_rust::messages::InputScriptType::Spendp2shwitness as i32),
                    _ => None
                }),
                show_display: *show_display,
                ..Default::default()
            };
            
            let response = queue_handle
                .send_raw(msg.into(), false)
                .await
                .map_err(|e| format!("Failed to get address: {}", e))?;
                
            match response {
                keepkey_rust::messages::Message::Address(address) => {
                    let elapsed = start_time.elapsed();
                    log::info!("  ✅ GetAddress completed in {:.3}s - Address: {}...{}", 
                        elapsed.as_secs_f64(),
                        &address.address[..10],
                        &address.address[address.address.len()-6..]);
                    Ok(DeviceResponse::Address {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        path: path.to_string(),
                        address: address.address.clone(),
                        success: true,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Failure(failure) => {
                    let elapsed = start_time.elapsed();
                    log::warn!("  ❌ GetAddress failed in {:.3}s: {}", 
                        elapsed.as_secs_f64(),
                        failure.message.as_ref().unwrap_or(&"Unknown error".to_string()));
                    Ok(DeviceResponse::Address {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        path: path.to_string(),
                        address: String::new(),
                        success: false,
                        error: Some(format!("Device returned error: {}", failure.message.unwrap_or_default())),
                    })
                }
                _ => Err("Unexpected response from device for address request".to_string()),
            }
        }
        
        DeviceRequest::GetPublicKey { path, coin_name, script_type, ecdsa_curve_name, show_display } => {
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
            
            // Simple approach like kkcli-v2: use Bitcoin as default coin and let firmware decide script type
            // This prevents complex auto-detection logic from causing failures
            let msg = keepkey_rust::messages::GetPublicKey {
                address_n: path_parts,
                coin_name: Some("Bitcoin".to_string()), // Default to Bitcoin for xpub generation (like kkcli-v2)
                script_type: None, // Let firmware determine script type based on path (like kkcli-v2)
                ecdsa_curve_name: ecdsa_curve_name.clone().or_else(|| Some("secp256k1".to_string())),
                show_display: *show_display,
                ..Default::default()
            };
            
            let response = queue_handle
                .send_raw(msg.into(), false)
                .await
                .map_err(|e| format!("Failed to get public key: {}", e))?;
                
            match response {
                keepkey_rust::messages::Message::PublicKey(public_key) => {
                    let elapsed = start_time.elapsed();
                    let empty_string = String::new();
                    let xpub = public_key.xpub.as_ref().unwrap_or(&empty_string);
                    log::info!("  ✅ GetPublicKey completed in {:.3}s - xpub: {}...{}", 
                        elapsed.as_secs_f64(),
                        &xpub[..10.min(xpub.len())],
                        if xpub.len() > 10 { &xpub[xpub.len()-6..] } else { "" });
                    Ok(DeviceResponse::PublicKey {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        xpub: public_key.xpub.unwrap_or_default(),
                        node: Some(serde_json::json!({
                            "depth": public_key.node.depth,
                            "fingerprint": public_key.node.fingerprint,
                            "child_num": public_key.node.child_num,
                            "chain_code": hex::encode(&public_key.node.chain_code),
                            "public_key": public_key.node.public_key.as_ref().map(|v| hex::encode(v)),
                        })),
                        success: true,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Failure(failure) => {
                    let elapsed = start_time.elapsed();
                    log::warn!("  ❌ GetPublicKey failed in {:.3}s: {}", 
                        elapsed.as_secs_f64(),
                        failure.message.as_ref().unwrap_or(&"Unknown error".to_string()));
                    Ok(DeviceResponse::PublicKey {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        xpub: String::new(),
                        node: None,
                        success: false,
                        error: Some(format!("Device returned error: {}", failure.message.unwrap_or_default())),
                    })
                }
                _ => Err("Unexpected response from device for public key request".to_string()),
            }
        }
        
        DeviceRequest::ListCoins => {
            // KeepKey doesn't have a specific ListCoins message
            // We would need to implement this differently, perhaps by querying supported coins
            // For now, return a static list or error
            Err("ListCoins not implemented for this device".to_string())
        }
        
        DeviceRequest::ApplySettings { label, language, use_passphrase, auto_lock_delay_ms, u2f_counter } => {
            let msg = keepkey_rust::messages::ApplySettings {
                label: label.clone(),
                language: language.clone(),
                use_passphrase: *use_passphrase,
                auto_lock_delay_ms: *auto_lock_delay_ms,
                u2f_counter: *u2f_counter,
                ..Default::default()
            };
            
            let response = queue_handle
                .send_raw(msg.into(), false)
                .await
                .map_err(|e| format!("Failed to apply settings: {}", e))?;
                
            match response {
                keepkey_rust::messages::Message::Success(success) => {
                    Ok(DeviceResponse::Success {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        message: success.message,
                        success: true,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Failure(failure) => {
                    Ok(DeviceResponse::Success {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        message: None,
                        success: false,
                        error: Some(format!("Device returned error: {}", failure.message.unwrap_or_default())),
                    })
                }
                _ => Err("Unexpected response from device for apply settings request".to_string()),
            }
        }
        
        DeviceRequest::ClearSession => {
            let msg = keepkey_rust::messages::ClearSession {};
            
            let response = queue_handle
                .send_raw(msg.into(), false)
                .await
                .map_err(|e| format!("Failed to clear session: {}", e))?;
                
            match response {
                keepkey_rust::messages::Message::Success(success) => {
                    Ok(DeviceResponse::Success {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        message: success.message,
                        success: true,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Failure(failure) => {
                    Ok(DeviceResponse::Success {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        message: None,
                        success: false,
                        error: Some(format!("Device returned error: {}", failure.message.unwrap_or_default())),
                    })
                }
                _ => Err("Unexpected response from device for clear session request".to_string()),
            }
        }
        
        DeviceRequest::WipeDevice => {
            let msg = keepkey_rust::messages::WipeDevice {};
            
            let response = queue_handle
                .send_raw(msg.into(), false)
                .await
                .map_err(|e| format!("Failed to wipe device: {}", e))?;
                
            match response {
                keepkey_rust::messages::Message::Success(success) => {
                    Ok(DeviceResponse::Success {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        message: success.message,
                        success: true,
                        error: None,
                    })
                }
                keepkey_rust::messages::Message::Failure(failure) => {
                    Ok(DeviceResponse::Success {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        message: None,
                        success: false,
                        error: Some(format!("Device returned error: {}", failure.message.unwrap_or_default())),
                    })
                }
                _ => Err("Unexpected response from device for wipe device request".to_string()),
            }
        }
        
        _ => Err("Not a system operation request".to_string()),
    };
    
    result
}

/// Process system requests with cache support
pub async fn process_system_request_with_cache(
    cache: &Arc<CacheManager>,
    queue_handle: &DeviceQueueHandle,
    request: &DeviceRequest,
    request_id: &str,
    device_id: &str,
) -> Result<DeviceResponse, String> {
    let start_time = std::time::Instant::now();
    
    match request {
        DeviceRequest::GetPublicKey { path, coin_name, script_type, .. } => {
            // Check cache first
            let actual_coin = coin_name.as_deref().unwrap_or("Bitcoin");
            log::info!("🔍 [CACHE CHECK] GetPublicKey - Path: {}, Coin: {}, ScriptType: {:?}", 
                path, actual_coin, script_type);
            
            if let Some(cached) = cache.get_cached_pubkey(
                device_id,
                path,
                actual_coin,
                script_type.as_deref(),
            ).await {
                let elapsed = start_time.elapsed();
                log::info!("✅ [CACHE HIT] GetPublicKey - Path: {} - Retrieved from cache in {:.3}ms", 
                    path, elapsed.as_secs_f64() * 1000.0);
                return Ok(cached.to_device_response(request_id));
            }
            
            log::info!("❌ [CACHE MISS] GetPublicKey - Path: {} - Fetching from device...", path);
            
            // Cache miss - fetch from device
            let device_start = std::time::Instant::now();
            let response = process_system_request(queue_handle, request, request_id, device_id).await?;
            let device_elapsed = device_start.elapsed();
            
            log::info!("🔧 [DEVICE] GetPublicKey completed in {:.3}s", device_elapsed.as_secs_f64());
            
            // Save to cache
            if let Some(cached) = CachedPubkey::from_device_response(
                device_id,
                path,
                actual_coin,
                script_type.as_deref(),
                &response,
            ) {
                let cache_start = std::time::Instant::now();
                if let Err(e) = cache.save_pubkey(&cached).await {
                    log::error!("❌ [CACHE SAVE ERROR] Failed to cache public key for path {}: {}", path, e);
                } else {
                    let cache_elapsed = cache_start.elapsed();
                    log::info!("💾 [CACHE SAVED] GetPublicKey - Path: {} - Saved to cache in {:.3}ms", 
                        path, cache_elapsed.as_secs_f64() * 1000.0);
                }
            } else {
                log::warn!("⚠️ [CACHE CONVERT ERROR] Could not convert response to cacheable format for path: {}", path);
            }
            
            let total_elapsed = start_time.elapsed();
            log::info!("⏱️ [TOTAL TIME] GetPublicKey - Path: {} - Total time: {:.3}s", 
                path, total_elapsed.as_secs_f64());
            
            Ok(response)
        }
        
        DeviceRequest::GetAddress { path, coin_name, script_type, .. } => {
            // For GetAddress, we might want to cache these too
            log::info!("🔍 [CACHE CHECK] GetAddress - Path: {}, Coin: {}, ScriptType: {:?}", 
                path, coin_name, script_type);
            
            // Check if we have this address cached
            if let Some(cached) = cache.get_cached_pubkey(
                device_id,
                path,
                coin_name,
                script_type.as_deref(),
            ).await {
                if cached.address.is_some() {
                    let elapsed = start_time.elapsed();
                    log::info!("✅ [CACHE HIT] GetAddress - Path: {} - Retrieved from cache in {:.3}ms", 
                        path, elapsed.as_secs_f64() * 1000.0);
                    return Ok(DeviceResponse::Address {
                        request_id: request_id.to_string(),
                        device_id: device_id.to_string(),
                        path: path.to_string(),
                        address: cached.address.unwrap_or_default(),
                        success: true,
                        error: None,
                    });
                }
            }
            
            log::info!("❌ [CACHE MISS] GetAddress - Path: {} - Fetching from device...", path);
            
            // Cache miss - fetch from device
            let device_start = std::time::Instant::now();
            let response = process_system_request(queue_handle, request, request_id, device_id).await?;
            let device_elapsed = device_start.elapsed();
            
            log::info!("🔧 [DEVICE] GetAddress completed in {:.3}s", device_elapsed.as_secs_f64());
            
            // Save to cache
            if let DeviceResponse::Address { address, .. } = &response {
                if !address.is_empty() {
                    let cached = CachedPubkey {
                        id: None,
                        device_id: device_id.to_string(),
                        derivation_path: path.to_string(),
                        coin_name: coin_name.to_string(),
                        script_type: script_type.clone(),
                        xpub: None,
                        address: Some(address.clone()),
                        chain_code: None,
                        public_key: None,
                        cached_at: chrono::Utc::now().timestamp(),
                        last_used: chrono::Utc::now().timestamp(),
                    };
                    
                    let cache_start = std::time::Instant::now();
                    if let Err(e) = cache.save_pubkey(&cached).await {
                        log::error!("❌ [CACHE SAVE ERROR] Failed to cache address for path {}: {}", path, e);
                    } else {
                        let cache_elapsed = cache_start.elapsed();
                        log::info!("💾 [CACHE SAVED] GetAddress - Path: {} - Saved to cache in {:.3}ms", 
                            path, cache_elapsed.as_secs_f64() * 1000.0);
                    }
                }
            }
            
            let total_elapsed = start_time.elapsed();
            log::info!("⏱️ [TOTAL TIME] GetAddress - Path: {} - Total time: {:.3}s", 
                path, total_elapsed.as_secs_f64());
            
            Ok(response)
        }
        
        _ => {
            // For other requests, just pass through
            log::info!("🔄 [PASSTHROUGH] Request type: {:?} - No caching", request);
            process_system_request(queue_handle, request, request_id, device_id).await
        }
    }
} 