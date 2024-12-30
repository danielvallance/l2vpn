//! Share utilities between vswitch.rs and vport.rs

/// Returns string representation of passed MAC bytes
pub fn mac_string(mac: &[u8]) -> String {
    mac.iter()
        .take(6)
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<String>>()
        .join(":")
}
