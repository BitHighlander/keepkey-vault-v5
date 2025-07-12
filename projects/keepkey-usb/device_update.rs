use serde::{Deserialize, Serialize};
use anyhow::{anyhow, Result};
use std::cmp::Ordering;
use crate::features::DeviceFeatures;
use std::collections::HashMap;

const TAG: &str = " | device_update | ";

// Latest versions - in production these would come from a manifest file or API
// Remove all hardcoded constants, we'll read from releases.json

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareReleases {
    pub latest: LatestVersions,
    pub hashes: HashMappings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersions {
    pub firmware: VersionReleaseInfo,
    pub bootloader: VersionReleaseInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionReleaseInfo {
    pub version: String,
    pub url: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashMappings {
    pub bootloader: HashMap<String, String>,
    pub firmware: HashMap<String, String>,
}

/// Load firmware releases from JSON file
pub fn load_firmware_releases() -> Result<FirmwareReleases> {
    // Debug: Log current working directory
    let cwd = std::env::current_dir().unwrap_or_default();
    log::info!("{TAG} Current working directory: {:?}", cwd);
    
    // Get executable location for bundled app paths
    let exe_path = std::env::current_exe().ok();
    let exe_dir = exe_path.as_ref().and_then(|p| p.parent());
    
    if let Some(exe_dir) = &exe_dir {
        log::info!("{TAG} Executable directory: {:?}", exe_dir);
    }
    
    // Build comprehensive list of possible paths
    let mut possible_paths = vec![
        // Development paths
        "../../keepkey-rust/firmware/releases.json",  // From vault-v2/src-tauri
        "../firmware/releases.json",  // From vault/src-tauri
        "firmware/releases.json",  // From keepkey-rust root
        "./firmware/releases.json",
        "../../firmware/releases.json",
        "projects/keepkey-desktop-v5/firmware/releases.json",
    ];
    
    // Add executable-relative paths for bundled apps
    if let Some(exe_dir) = exe_dir {
        possible_paths.push(exe_dir.join("firmware/releases.json").to_string_lossy().to_string());
        possible_paths.push(exe_dir.join("../Resources/firmware/releases.json").to_string_lossy().to_string()); // macOS
        possible_paths.push(exe_dir.join("../firmware/releases.json").to_string_lossy().to_string());
        possible_paths.push(exe_dir.join("../../firmware/releases.json").to_string_lossy().to_string());
        possible_paths.push(exe_dir.join("../../keepkey-rust/firmware/releases.json").to_string_lossy().to_string());
        possible_paths.push(exe_dir.join("../../../keepkey-rust/firmware/releases.json").to_string_lossy().to_string());
    }
    
    // Add absolute path from cwd
    possible_paths.push(cwd.join("firmware/releases.json").to_string_lossy().to_string());
    
    log::info!("{TAG} Trying to find releases.json. Checking paths:");
    for (i, releases_path) in possible_paths.iter().enumerate() {
        let path = std::path::Path::new(releases_path);
        let exists = path.exists();
        log::info!("{TAG}   Path {}: {:?} - exists: {}", i + 1, releases_path, exists);
        
        if exists {
            match std::fs::read_to_string(releases_path) {
                Ok(content) => {
                    log::info!("{TAG} Successfully loaded releases.json from: {}", releases_path);
                    let releases: FirmwareReleases = serde_json::from_str(&content)?;
                    return Ok(releases);
                }
                Err(e) => {
                    log::error!("{TAG} Failed to read releases.json from {}: {}", releases_path, e);
                }
            }
        }
    }
    
    // Log which paths were tried
    log::error!("{TAG} Could not find releases.json. Tried {} paths", possible_paths.len());
    
    Err(anyhow!("Could not find releases.json in any expected location"))
}

pub fn get_latest_firmware_version() -> Result<String> {
    let tag = " | get_latest_firmware_version | ";
    
    let release_data = get_releases_data()?;
    
    if let Some(latest_firmware) = release_data["latest"]["firmware"]["version"].as_str() {
        let version = latest_firmware.trim_start_matches('v');
        log::info!("{tag} Latest firmware version: {}", version);
        Ok(version.to_string())
    } else {
        log::error!("{tag} Could not parse firmware version from releases.json");
        Err(anyhow!("Could not parse firmware version from releases.json"))
    }
}

pub fn get_releases_data() -> Result<serde_json::Value> {
    // Use the same logic as load_firmware_releases for consistency
    match load_firmware_releases() {
        Ok(releases) => {
            // Convert back to serde_json::Value
            let json_str = serde_json::to_string(&releases)?;
            let json_data: serde_json::Value = serde_json::from_str(&json_str)?;
            Ok(json_data)
        }
        Err(e) => Err(e)
    }
}

pub fn get_latest_bootloader_version() -> Result<String> {
    let tag = " | get_latest_bootloader_version | ";
    
    let release_data = get_releases_data()?;
    
    if let Some(latest_bootloader) = release_data["latest"]["bootloader"]["version"].as_str() {
        let version = latest_bootloader.trim_start_matches('v');
        log::info!("{tag} Latest bootloader version: {}", version);
        Ok(version.to_string())
    } else {
        log::error!("{tag} Could not parse bootloader version from releases.json");
        Err(anyhow!("Could not parse bootloader version from releases.json"))
    }
}

pub fn get_latest_bootloader_hash() -> Result<String> {
    let tag = " | get_latest_bootloader_hash | ";
    
    let release_data = get_releases_data()?;
    
    if let Some(hash) = release_data["latest"]["bootloader"]["hash"].as_str() {
        log::info!("{tag} Latest bootloader hash: {}", hash);
        Ok(hash.to_string())
    } else {
        log::error!("{tag} Could not parse bootloader hash from releases.json");
        Err(anyhow!("Could not parse bootloader hash from releases.json"))
    }
}

/// Look up bootloader version from hash
pub fn bootloader_version_from_hash(hash: &str) -> Option<String> {
    match load_firmware_releases() {
        Ok(releases) => {
            if let Some(version) = releases.hashes.bootloader.get(hash) {
                // Remove 'v' prefix if present for consistency
                let clean_version = version.trim_start_matches('v');
                log::info!("{TAG} Found bootloader version {} for hash {}", clean_version, hash);
                Some(clean_version.to_string())
            } else {
                log::warn!("{TAG} No bootloader version found for hash {}", hash);
                None
            }
        }
        Err(e) => {
            log::error!("{TAG} Failed to load releases.json: {}", e);
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl VersionInfo {
    pub fn from_string(version: &str) -> Result<Self> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow!("Invalid version format: {}", version));
        }
        
        Ok(VersionInfo {
            major: parts[0].parse()?,
            minor: parts[1].parse()?,
            patch: parts[2].parse()?,
        })
    }
    
    pub fn to_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PartialOrd for VersionInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VersionInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major.cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
    }
}

impl PartialEq for VersionInfo {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major && self.minor == other.minor && self.patch == other.patch
    }
}

impl Eq for VersionInfo {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VersionComparison {
    Current,
    PatchBehind,
    MinorBehind,
    MajorBehind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootloaderCheck {
    pub current_version: String,
    pub latest_version: String,
    pub target_version: String,
    pub is_outdated: bool,
    pub is_critical: bool,
    pub comparison: VersionComparison,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareCheck {
    pub current_version: String,
    pub latest_version: String,
    pub is_outdated: bool,
    pub release_notes: Vec<String>,
    pub security_update: bool,
    pub comparison: VersionComparison,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializationCheck {
    pub initialized: bool,
    pub has_backup: bool,
    pub imported: bool,
    pub needs_setup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub device_id: String,
    pub bootloader_check: Option<BootloaderCheck>,
    pub firmware_check: Option<FirmwareCheck>,
    pub initialization_check: InitializationCheck,
    pub needs_bootloader_update: bool,
    pub needs_firmware_update: bool,
    pub needs_initialization: bool,
}

/// Compare two version strings and return the comparison result
pub fn compare_versions(current: &str, latest: &str) -> Result<VersionComparison> {
    let current_ver = VersionInfo::from_string(current)?;
    let latest_ver = VersionInfo::from_string(latest)?;
    
    Ok(match current_ver.cmp(&latest_ver) {
        Ordering::Equal => VersionComparison::Current,
        Ordering::Less => {
            if current_ver.major < latest_ver.major {
                VersionComparison::MajorBehind
            } else if current_ver.minor < latest_ver.minor {
                VersionComparison::MinorBehind
            } else {
                VersionComparison::PatchBehind
            }
        },
        Ordering::Greater => {
            log::warn!("{TAG} Device version {} is newer than latest known version {}", current, latest);
            VersionComparison::Current
        }
    })
}

/// Check if bootloader needs update
pub fn check_bootloader_status(features: &DeviceFeatures) -> Option<BootloaderCheck> {
    // Only check bootloader if device is in bootloader mode
    if !features.bootloader_mode {
        return None;
    }
    
    let current_version = features.version.clone();
    let latest_version = get_latest_bootloader_version().ok()?;
    
    let comparison = match compare_versions(&current_version, &latest_version) {
        Ok(comp) => comp,
        Err(e) => {
            log::error!("{TAG} Failed to compare bootloader versions: {}", e);
            return None;
        }
    };
    
    let is_outdated = !matches!(comparison, VersionComparison::Current);
    let is_critical = matches!(comparison, VersionComparison::MajorBehind);
    
    Some(BootloaderCheck {
        current_version,
        latest_version: latest_version.clone(),
        target_version: latest_version,
        is_outdated,
        is_critical,
        comparison,
    })
}

/// Check if firmware needs update
pub fn check_firmware_status(features: &DeviceFeatures) -> Option<FirmwareCheck> {
    // Only check firmware if device is NOT in bootloader mode
    if features.bootloader_mode {
        return None;
    }
    
    let current_version = features.version.clone();
            let latest_version = get_latest_firmware_version().ok()?;
    
    let comparison = match compare_versions(&current_version, &latest_version) {
        Ok(comp) => comp,
        Err(e) => {
            log::error!("{TAG} Failed to compare firmware versions: {}", e);
            return None;
        }
    };
    
    let is_outdated = !matches!(comparison, VersionComparison::Current);
    let security_update = matches!(comparison, VersionComparison::MajorBehind | VersionComparison::MinorBehind);
    
    // TODO: Load these from a manifest file
    let release_notes = if is_outdated {
        vec![
            "Bug fixes and performance improvements".to_string(),
            "Enhanced security features".to_string(),
            "New coin support".to_string(),
        ]
    } else {
        vec![]
    };
    
    Some(FirmwareCheck {
        current_version,
        latest_version,
        is_outdated,
        release_notes,
        security_update,
        comparison,
    })
}

/// Check device initialization status
pub fn check_initialization_status(features: &DeviceFeatures) -> InitializationCheck {
    InitializationCheck {
        initialized: features.initialized,
        has_backup: !features.no_backup,
        imported: features.imported.unwrap_or(false),
        needs_setup: !features.initialized,
    }
}

/// Evaluate complete device status and determine required actions
pub fn evaluate_device_status(device_id: String, features: &DeviceFeatures) -> DeviceStatus {
    let bootloader_check = check_bootloader_status(features);
    let firmware_check = check_firmware_status(features);
    let initialization_check = check_initialization_status(features);
    
    let needs_bootloader_update = bootloader_check
        .as_ref()
        .map(|check| check.is_outdated)
        .unwrap_or(false);
    
    let needs_firmware_update = firmware_check
        .as_ref()
        .map(|check| check.is_outdated)
        .unwrap_or(false);
    
    let needs_initialization = initialization_check.needs_setup;
    
    DeviceStatus {
        device_id,
        bootloader_check,
        firmware_check,
        initialization_check,
        needs_bootloader_update,
        needs_firmware_update,
        needs_initialization,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_version_comparison() {
        assert_eq!(
            compare_versions("1.0.0", "2.0.0").unwrap(),
            VersionComparison::MajorBehind
        );
        assert_eq!(
            compare_versions("2.0.0", "2.1.0").unwrap(),
            VersionComparison::MinorBehind
        );
        assert_eq!(
            compare_versions("2.1.0", "2.1.1").unwrap(),
            VersionComparison::PatchBehind
        );
        assert_eq!(
            compare_versions("2.1.1", "2.1.1").unwrap(),
            VersionComparison::Current
        );
    }
} 