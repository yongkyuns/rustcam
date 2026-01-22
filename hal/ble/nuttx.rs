//! NuttX BLE implementation using NimBLE via C wrapper
//!
//! This implementation calls into a C wrapper (ble_wrapper.c) that handles
//! all the NimBLE interactions. This simplifies FFI and avoids complex
//! callback handling in Rust.

use super::{
    BleAddress, BleError, BleResult, CharacteristicHandle, ConnectionHandle, ScanResult, Uuid,
};
use core::ffi::{c_char, c_int};
use std::ffi::CString;

// ============================================================================
// C Wrapper FFI Bindings
// ============================================================================

extern "C" {
    /// Initialize BLE subsystem
    fn rust_ble_wrapper_init() -> c_int;

    /// Deinitialize BLE subsystem
    fn rust_ble_wrapper_deinit() -> c_int;

    /// Start BLE advertising with device name
    fn rust_ble_wrapper_start_advertising(name: *const c_char) -> c_int;

    /// Stop BLE advertising
    fn rust_ble_wrapper_stop_advertising() -> c_int;

    /// Check if connected
    fn rust_ble_wrapper_is_connected() -> c_int;

    /// Run BLE host task (blocking)
    fn rust_ble_wrapper_run();

    /// Sleep in microseconds
    fn usleep(usec: u32) -> c_int;
}

// ============================================================================
// Public API Implementation
// ============================================================================

/// Initialize BLE subsystem
pub fn ble_initialize() -> BleResult<()> {
    let rc = unsafe { rust_ble_wrapper_init() };
    if rc == 0 {
        // Note: Cannot use eprintln! on NuttX due to Rust std IO issues
        Ok(())
    } else if rc == -libc::EALREADY {
        Err(BleError::AlreadyInitialized)
    } else {
        Err(BleError::SocketError)
    }
}

/// Deinitialize BLE subsystem
pub fn ble_deinitialize() -> BleResult<()> {
    let rc = unsafe { rust_ble_wrapper_deinit() };
    if rc == 0 {
        Ok(())
    } else if rc == -libc::ENODEV {
        Err(BleError::NotInitialized)
    } else {
        Err(BleError::SocketError)
    }
}

/// Start BLE scanning
pub fn ble_start_scan(_timeout_ms: u32) -> BleResult<()> {
    // Scanning requires central role - not yet implemented in wrapper
    Err(BleError::NotSupported)
}

/// Stop BLE scanning
pub fn ble_stop_scan() -> BleResult<()> {
    Err(BleError::NotSupported)
}

/// Get scan results
pub fn ble_get_scan_results() -> BleResult<Vec<ScanResult>> {
    Err(BleError::NotSupported)
}

/// Start BLE advertising
pub fn ble_start_advertising(name: &str) -> BleResult<()> {
    let c_name = CString::new(name).map_err(|_| BleError::InvalidParameter)?;
    let rc = unsafe { rust_ble_wrapper_start_advertising(c_name.as_ptr()) };

    if rc == 0 {
        Ok(())
    } else if rc == -libc::ENODEV {
        Err(BleError::NotInitialized)
    } else if rc == -libc::ENOTSUP {
        Err(BleError::NotSupported)
    } else {
        Err(BleError::SocketError)
    }
}

/// Stop BLE advertising
pub fn ble_stop_advertising() -> BleResult<()> {
    let rc = unsafe { rust_ble_wrapper_stop_advertising() };
    if rc == 0 {
        Ok(())
    } else {
        Err(BleError::SocketError)
    }
}

/// Run a simple GATT server
pub fn ble_run_gatt_server(name: &str, timeout_ms: u32) -> BleResult<()> {
    // Start advertising
    ble_start_advertising(name)?;

    // Note: Cannot use eprintln! on NuttX due to Rust std IO issues
    // Wait for timeout, checking connection status
    let iterations = timeout_ms / 100;
    for _ in 0..iterations {
        unsafe { usleep(100_000); }  // 100ms

        let _connected = unsafe { rust_ble_wrapper_is_connected() };
        // Connection status is available but we can't print it
    }

    // Stop advertising
    ble_stop_advertising()?;

    Ok(())
}

/// Connect to a BLE device (central role - not supported)
pub fn ble_connect(_address: &BleAddress, _timeout_ms: u32) -> BleResult<ConnectionHandle> {
    Err(BleError::NotSupported)
}

/// Disconnect from a BLE device
pub fn ble_disconnect(_handle: ConnectionHandle) -> BleResult<()> {
    Err(BleError::NotSupported)
}

/// Discover GATT services (central role - not supported)
pub fn gatt_discover_services(_handle: ConnectionHandle) -> BleResult<Vec<Uuid>> {
    Err(BleError::NotSupported)
}

/// Read a GATT characteristic (central role - not supported)
pub fn gatt_read_characteristic(_char: CharacteristicHandle) -> BleResult<Vec<u8>> {
    Err(BleError::NotSupported)
}

/// Write to a GATT characteristic (central role - not supported)
pub fn gatt_write_characteristic(_char: CharacteristicHandle, _data: &[u8]) -> BleResult<()> {
    Err(BleError::NotSupported)
}
