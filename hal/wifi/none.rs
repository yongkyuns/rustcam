//! WiFi HAL stub for unsupported platforms

use super::{
    ConnectionStatus, IpInfo, ScanResult, StationConfig, WifiError, WifiMode, WifiResult,
};

pub fn wifi_initialize() -> WifiResult<()> {
    Err(WifiError::NotSupported)
}

pub fn wifi_deinitialize() -> WifiResult<()> {
    Err(WifiError::NotSupported)
}

pub fn wifi_is_initialized() -> bool {
    false
}

pub fn wifi_set_mode(_mode: WifiMode) -> WifiResult<()> {
    Err(WifiError::NotSupported)
}

pub fn wifi_get_mode() -> WifiResult<WifiMode> {
    Err(WifiError::NotSupported)
}

pub fn wifi_start_scan() -> WifiResult<()> {
    Err(WifiError::NotSupported)
}

pub fn wifi_scan_is_complete() -> WifiResult<bool> {
    Err(WifiError::NotSupported)
}

pub fn wifi_get_scan_results() -> WifiResult<([ScanResult; 16], usize)> {
    Err(WifiError::NotSupported)
}

pub fn wifi_connect(_config: &StationConfig) -> WifiResult<()> {
    Err(WifiError::NotSupported)
}

pub fn wifi_disconnect() -> WifiResult<()> {
    Err(WifiError::NotSupported)
}

pub fn wifi_get_connection_status() -> WifiResult<ConnectionStatus> {
    Err(WifiError::NotSupported)
}

pub fn wifi_get_essid() -> WifiResult<([u8; 32], usize)> {
    Err(WifiError::NotSupported)
}

pub fn wifi_get_ip_info() -> WifiResult<IpInfo> {
    Err(WifiError::NotSupported)
}

pub fn wifi_get_rssi() -> WifiResult<i8> {
    Err(WifiError::NotSupported)
}

pub fn wifi_get_mac_address() -> WifiResult<[u8; 6]> {
    Err(WifiError::NotSupported)
}
