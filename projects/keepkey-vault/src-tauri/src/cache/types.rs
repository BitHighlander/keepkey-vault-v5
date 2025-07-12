use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedPubkey {
    pub id: Option<i64>,
    pub device_id: String,
    pub derivation_path: String,
    pub coin_name: String,
    pub script_type: Option<String>,
    pub xpub: Option<String>,
    pub address: Option<String>,
    pub chain_code: Option<Vec<u8>>,
    pub public_key: Option<Vec<u8>>,
    pub cached_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CacheMetadata {
    pub device_id: String,
    pub label: Option<String>,
    pub firmware_version: Option<String>,
    pub initialized: bool,
    pub frontload_status: FrontloadStatus,
    pub frontload_progress: i32,
    pub last_frontload: Option<i64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrontloadStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl FrontloadStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FrontloadStatus::Pending => "pending",
            FrontloadStatus::InProgress => "in_progress",
            FrontloadStatus::Completed => "completed",
            FrontloadStatus::Failed => "failed",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(FrontloadStatus::Pending),
            "in_progress" => Some(FrontloadStatus::InProgress),
            "completed" => Some(FrontloadStatus::Completed),
            "failed" => Some(FrontloadStatus::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CacheStatus {
    pub device_id: String,
    pub total_cached: i64,
    pub cache_hits: i64,
    pub cache_misses: i64,
    pub hit_rate: f64,
    pub last_frontload: Option<i64>,
    pub frontload_status: FrontloadStatus,
    pub frontload_progress: i32,
}

impl CachedPubkey {
    /// Convert from DeviceResponse to CachedPubkey
    pub fn from_device_response(
        device_id: &str,
        path: &str,
        coin_name: &str,
        script_type: Option<&str>,
        response: &crate::commands::DeviceResponse,
    ) -> Option<Self> {
        match response {
            crate::commands::DeviceResponse::PublicKey {
                xpub, node, ..
            } => {
                let (chain_code, public_key) = if let Some(node_val) = node {
                    let chain_code = node_val.get("chain_code")
                        .and_then(|v| v.as_str())
                        .and_then(|s| hex::decode(s).ok());
                    let public_key = node_val.get("public_key")
                        .and_then(|v| v.as_str())
                        .and_then(|s| hex::decode(s).ok());
                    (chain_code, public_key)
                } else {
                    (None, None)
                };

                Some(CachedPubkey {
                    id: None,
                    device_id: device_id.to_string(),
                    derivation_path: path.to_string(),
                    coin_name: coin_name.to_string(),
                    script_type: script_type.map(|s| s.to_string()),
                    xpub: Some(xpub.clone()),
                    address: None,
                    chain_code,
                    public_key,
                    cached_at: chrono::Utc::now().timestamp(),
                    last_used: chrono::Utc::now().timestamp(),
                })
            }
            crate::commands::DeviceResponse::Address {
                address, ..
            } => Some(CachedPubkey {
                id: None,
                device_id: device_id.to_string(),
                derivation_path: path.to_string(),
                coin_name: coin_name.to_string(),
                script_type: script_type.map(|s| s.to_string()),
                xpub: None,
                address: Some(address.clone()),
                chain_code: None,
                public_key: None,
                cached_at: chrono::Utc::now().timestamp(),
                last_used: chrono::Utc::now().timestamp(),
            }),
            _ => None,
        }
    }
    
    /// Convert to DeviceResponse
    pub fn to_device_response(&self, request_id: &str) -> crate::commands::DeviceResponse {
        if let Some(xpub) = &self.xpub {
            let mut node = serde_json::json!({});
            if let Some(chain_code) = &self.chain_code {
                node["chain_code"] = serde_json::json!(hex::encode(chain_code));
            }
            if let Some(public_key) = &self.public_key {
                node["public_key"] = serde_json::json!(hex::encode(public_key));
            }
            
            crate::commands::DeviceResponse::PublicKey {
                request_id: request_id.to_string(),
                device_id: self.device_id.clone(),
                xpub: xpub.clone(),
                node: Some(node),
                success: true,
                error: None,
            }
        } else if let Some(address) = &self.address {
            crate::commands::DeviceResponse::Address {
                request_id: request_id.to_string(),
                device_id: self.device_id.clone(),
                path: self.derivation_path.clone(),
                address: address.clone(),
                success: true,
                error: None,
            }
        } else {
            crate::commands::DeviceResponse::Address {
                request_id: request_id.to_string(),
                device_id: self.device_id.clone(),
                path: self.derivation_path.clone(),
                address: String::new(),
                success: false,
                error: Some("No cached data available".to_string()),
            }
        }
    }
} 