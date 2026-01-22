//! Stub BLE implementation
//!
//! Used when no platform-specific implementation is available.
//! All functions return NotSupported error.

use super::{
    BleAddress, BleError, BleResult, CharacteristicHandle, ConnectionHandle, ScanResult, Uuid,
};

/// Initialize BLE subsystem (stub: returns NotSupported)
pub fn ble_initialize() -> BleResult<()> {
    Err(BleError::NotSupported)
}

/// Deinitialize BLE subsystem (stub: returns NotSupported)
pub fn ble_deinitialize() -> BleResult<()> {
    Err(BleError::NotSupported)
}

/// Start BLE scanning (stub: returns NotSupported)
pub fn ble_start_scan(_timeout_ms: u32) -> BleResult<()> {
    Err(BleError::NotSupported)
}

/// Stop BLE scanning (stub: returns NotSupported)
pub fn ble_stop_scan() -> BleResult<()> {
    Err(BleError::NotSupported)
}

/// Get scan results (stub: returns NotSupported)
pub fn ble_get_scan_results() -> BleResult<Vec<ScanResult>> {
    Err(BleError::NotSupported)
}

/// Connect to a BLE device (stub: returns NotSupported)
pub fn ble_connect(_address: &BleAddress, _timeout_ms: u32) -> BleResult<ConnectionHandle> {
    Err(BleError::NotSupported)
}

/// Disconnect from a BLE device (stub: returns NotSupported)
pub fn ble_disconnect(_handle: ConnectionHandle) -> BleResult<()> {
    Err(BleError::NotSupported)
}

/// Discover GATT services (stub: returns NotSupported)
pub fn gatt_discover_services(_handle: ConnectionHandle) -> BleResult<Vec<Uuid>> {
    Err(BleError::NotSupported)
}

/// Read a GATT characteristic (stub: returns NotSupported)
pub fn gatt_read_characteristic(_char: CharacteristicHandle) -> BleResult<Vec<u8>> {
    Err(BleError::NotSupported)
}

/// Write to a GATT characteristic (stub: returns NotSupported)
pub fn gatt_write_characteristic(_char: CharacteristicHandle, _data: &[u8]) -> BleResult<()> {
    Err(BleError::NotSupported)
}

/// Start BLE advertising (stub: returns NotSupported)
pub fn ble_start_advertising(_name: &str) -> BleResult<()> {
    Err(BleError::NotSupported)
}

/// Stop BLE advertising (stub: returns NotSupported)
pub fn ble_stop_advertising() -> BleResult<()> {
    Err(BleError::NotSupported)
}

/// Run a GATT server (stub: returns NotSupported)
pub fn ble_run_gatt_server(_name: &str, _timeout_ms: u32) -> BleResult<()> {
    Err(BleError::NotSupported)
}
