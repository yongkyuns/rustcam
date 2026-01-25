//! WiFi HAL for Linux
//!
//! TODO: Implement using nl80211 netlink API.
//! For now, this is a stub that returns NotSupported.

use super::{
    ConnectionStatus, IpInfo, ScanResult, StationConfig, WifiError, WifiMode, WifiResult,
};

/// Initialize WiFi subsystem
pub fn wifi_initialize() -> WifiResult<()> {
    // TODO: Enumerate WiFi interfaces using nl80211
    Err(WifiError::NotSupported)
}

/// Deinitialize WiFi subsystem
pub fn wifi_deinitialize() -> WifiResult<()> {
    Err(WifiError::NotSupported)
}

/// Check if WiFi is initialized
pub fn wifi_is_initialized() -> bool {
    false
}

/// Set WiFi operating mode
pub fn wifi_set_mode(_mode: WifiMode) -> WifiResult<()> {
    Err(WifiError::NotSupported)
}

/// Get WiFi operating mode
pub fn wifi_get_mode() -> WifiResult<WifiMode> {
    Err(WifiError::NotSupported)
}

/// Start WiFi scan
pub fn wifi_start_scan() -> WifiResult<()> {
    Err(WifiError::NotSupported)
}

/// Check if scan is complete
pub fn wifi_scan_is_complete() -> WifiResult<bool> {
    Err(WifiError::NotSupported)
}

/// Get scan results
pub fn wifi_get_scan_results() -> WifiResult<([ScanResult; 16], usize)> {
    Err(WifiError::NotSupported)
}

/// Connect to WiFi network
pub fn wifi_connect(_config: &StationConfig) -> WifiResult<()> {
    Err(WifiError::NotSupported)
}

/// Disconnect from WiFi network
pub fn wifi_disconnect() -> WifiResult<()> {
    Err(WifiError::NotSupported)
}

/// Get current connection status
pub fn wifi_get_connection_status() -> WifiResult<ConnectionStatus> {
    Err(WifiError::NotSupported)
}

/// Get current ESSID
pub fn wifi_get_essid() -> WifiResult<([u8; 32], usize)> {
    Err(WifiError::NotSupported)
}

/// Get IP information
pub fn wifi_get_ip_info() -> WifiResult<IpInfo> {
    Err(WifiError::NotSupported)
}

/// Get signal strength
pub fn wifi_get_rssi() -> WifiResult<i8> {
    Err(WifiError::NotSupported)
}

/// Get MAC address
pub fn wifi_get_mac_address() -> WifiResult<[u8; 6]> {
    Err(WifiError::NotSupported)
}
