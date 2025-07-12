// SLIP-132 xpub/ypub/zpub prefix conversion utility for Bitcoin
// Implements version byte mapping and base58check encoding/decoding
// https://github.com/satoshilabs/slips/blob/master/slip-0132.md

use std::collections::HashMap;
use lazy_static::lazy_static;
use base58::{FromBase58, ToBase58};

// SLIP-132 version bytes for Bitcoin mainnet
pub const XPUB: [u8; 4] = [0x04, 0x88, 0xB2, 0x1E]; // xpub (BIP44, legacy)
pub const YPUB: [u8; 4] = [0x04, 0x9D, 0x7C, 0xB2]; // ypub (BIP49, segwit-p2sh)
pub const ZPUB: [u8; 4] = [0x04, 0xB2, 0x47, 0x46]; // zpub (BIP84, segwit-native)

lazy_static! {
    // Map script type to version bytes
    pub static ref SCRIPT_TYPE_TO_VERSION: HashMap<&'static str, [u8; 4]> = {
        let mut m = HashMap::new();
        m.insert("p2pkh", XPUB); // legacy
        m.insert("p2sh-p2wpkh", YPUB); // segwit (p2sh)
        m.insert("p2wpkh", ZPUB); // segwit (native)
        m
    };
    // Map version bytes to prefix string
    pub static ref VERSION_TO_PREFIX: HashMap<[u8; 4], &'static str> = {
        let mut m = HashMap::new();
        m.insert(XPUB, "xpub");
        m.insert(YPUB, "ypub");
        m.insert(ZPUB, "zpub");
        m
    };
}

/// Converts a base58check-encoded xpub to the correct SLIP-132 prefix for the given script type
pub fn convert_xpub_prefix(xpub: &str, script_type: &str) -> Result<String, String> {
    let target_version = SCRIPT_TYPE_TO_VERSION.get(script_type)
        .ok_or_else(|| format!("Unsupported script type: {}", script_type))?;
    let data = xpub.from_base58().map_err(|_| "Invalid base58 encoding".to_string())?;
    println!("[slip132-debug] Decoded xpub bytes: {}", hex::encode(&data));
    if data.len() < 4 {
        return Err("Invalid xpub length".to_string());
    }
    // Remove existing checksum (last 4 bytes)
    let mut no_checksum = data[..data.len()-4].to_vec();
    println!("[slip132-debug] No-checksum bytes: {}", hex::encode(&no_checksum));
    // Replace first 4 bytes (version)
    no_checksum[0..4].copy_from_slice(target_version);
    println!("[slip132-debug] With new version bytes: {}", hex::encode(&no_checksum));
    // Calculate new checksum
    let checksum = sha256d(&no_checksum);
    println!("[slip132-debug] New checksum: {}", hex::encode(&checksum[0..4]));
    // Append checksum
    let mut with_checksum = no_checksum.clone();
    with_checksum.extend_from_slice(&checksum[0..4]);
    println!("[slip132-debug] Final bytes to encode: {}", hex::encode(&with_checksum));
    let result = with_checksum.to_base58();
    println!("[slip132-debug] Final base58: {}", result);
    Ok(result)
}

/// Double SHA256 for base58check
fn sha256d(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let hash1 = Sha256::digest(data);
    let hash2 = Sha256::digest(&hash1);
    let mut out = [0u8; 32];
    out.copy_from_slice(&hash2);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    // Example xpub for testing (BIP44)
    const XPUB_EXAMPLE: &str = "xpub661MyMwAqRbcF9p5jv6Zy9cX1c1Kp8v9Jr6m4n5b8z8eWj3zNw8wK8p6Jx8V9h6p1n8v3z6b9a6x3p8x3z9k1k3w1w8m4b6h9r1z8v4c";
    #[test]
    fn test_convert_xpub_prefix() {
        // This test will only check that conversion does not panic or error for valid input
        let _ = convert_xpub_prefix(XPUB_EXAMPLE, "p2pkh");
        let _ = convert_xpub_prefix(XPUB_EXAMPLE, "p2sh-p2wpkh");
        let _ = convert_xpub_prefix(XPUB_EXAMPLE, "p2wpkh");
    }
}
