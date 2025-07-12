use semver::{Version};

// Compares two version strings. Returns Ok(true) if current_version < required_version.
// Returns Err if parsing fails.
pub fn is_version_older(current_version_str: &str, required_version_str: &str) -> Result<bool, String> {
    let current = Version::parse(current_version_str)
        .map_err(|e| format!("Failed to parse current version '{}': {}", current_version_str, e))?;
    let required = Version::parse(required_version_str)
        .map_err(|e| format!("Failed to parse required version '{}': {}", required_version_str, e))?;
    Ok(current < required)
}

/// Parse a derivation path string like "m/44'/0'/0'/0/0" into a Vec<u32>
/// Returns the path components as hardened (0x80000000 | index) or non-hardened values
pub fn parse_derivation_path(path: &str) -> Result<Vec<u32>, String> {
    if !path.starts_with('m') {
        return Err("Derivation path must start with 'm'".to_string());
    }
    
    let path = &path[1..]; // Remove 'm'
    if path.is_empty() {
        return Ok(vec![]);
    }
    
    if !path.starts_with('/') {
        return Err("Invalid derivation path format".to_string());
    }
    
    let path = &path[1..]; // Remove leading '/'
    if path.is_empty() {
        return Ok(vec![]);
    }
    
    let mut result = Vec::new();
    
    for component in path.split('/') {
        if component.is_empty() {
            continue;
        }
        
        let (is_hardened, number_str) = if component.ends_with('\'') || component.ends_with('h') {
            (true, &component[..component.len() - 1])
        } else {
            (false, component)
        };
        
        let number: u32 = number_str.parse()
            .map_err(|_| format!("Invalid number in derivation path: '{}'", component))?;
        
        let value = if is_hardened {
            0x80000000 | number
        } else {
            number
        };
        
        result.push(value);
    }
    
    Ok(result)
}

// You can add other utility functions here as needed.
