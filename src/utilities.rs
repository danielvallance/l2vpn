//! Share utilities between vswitch.rs and vport.rs

/// Returns string representation of passed MAC bytes
pub fn mac_string(mac: &[u8]) -> String {
    mac.iter()
        .take(6)
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<String>>()
        .join(":")
}

/// Returns log message with details of frame
pub fn get_frame_log_msg(frame: &[u8]) -> String {
    let dst_mac = mac_string(&frame[0..6]);
    let src_mac = mac_string(&frame[6..12]);
    let ether_type = ((frame[12] as u16) << 8) + frame[13] as u16;
    format!(
        "dst_mac={}, src_mac={}, type={}, size={}",
        dst_mac,
        src_mac,
        ether_type,
        frame.len()
    )
}
