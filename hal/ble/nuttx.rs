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

    /// Get last received GATT command
    fn rust_ble_wrapper_gatt_get_command(buf: *mut u8, buf_len: c_int) -> c_int;

    /// Check if GATT command is available
    fn rust_ble_wrapper_gatt_has_command() -> c_int;

    /// Set GATT read response message
    fn rust_ble_wrapper_gatt_set_read_msg(msg: *const c_char) -> c_int;

    /// Print debug status information
    fn rust_ble_wrapper_debug_print_status();

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
///
/// This starts advertising and waits for connections. When a client connects
/// and writes to the write characteristic (UUID 0x1236), the command is
/// printed. The read characteristic (UUID 0x1235) returns "Hello from RustCam!"
/// by default.
///
/// # Arguments
/// * `name` - Device name for advertising
/// * `timeout_ms` - Maximum time to run (0 for no timeout)
///
/// # Returns
/// Ok(()) when timeout expires or error occurs
pub fn ble_run_gatt_server(name: &str, timeout_ms: u32) -> BleResult<()> {
    // Set the read message
    let c_hello = CString::new("Hello from RustCam!").map_err(|_| BleError::InvalidParameter)?;
    unsafe { rust_ble_wrapper_gatt_set_read_msg(c_hello.as_ptr()); }

    // Start advertising
    ble_start_advertising(name)?;

    // Print debug info after advertising starts
    ble_debug_print_status();

    // Poll loop for connection and commands
    let iterations = if timeout_ms == 0 { u32::MAX } else { timeout_ms / 100 };
    let mut command_buffer = [0u8; 64];

    for i in 0..iterations {
        unsafe { usleep(100_000); }  // 100ms

        let connected = unsafe { rust_ble_wrapper_is_connected() };

        // Check for received commands
        if unsafe { rust_ble_wrapper_gatt_has_command() } != 0 {
            let len = unsafe {
                rust_ble_wrapper_gatt_get_command(
                    command_buffer.as_mut_ptr(),
                    command_buffer.len() as c_int,
                )
            };

            if len > 0 {
                // Print received command using FFI debug print
                extern "C" {
                    fn rust_debug_print(msg: *const u8);
                }
                // Format a simple message
                let mut msg = [0u8; 80];
                let prefix = b"GATT command received: ";
                msg[..prefix.len()].copy_from_slice(prefix);
                let copy_len = core::cmp::min(len as usize, msg.len() - prefix.len() - 1);
                msg[prefix.len()..prefix.len() + copy_len]
                    .copy_from_slice(&command_buffer[..copy_len]);
                msg[prefix.len() + copy_len] = 0;
                unsafe { rust_debug_print(msg.as_ptr()); }
            }
        }

        // Log connection status periodically (every 5 seconds)
        if i % 50 == 0 && connected != 0 {
            extern "C" {
                fn rust_debug_print(msg: *const u8);
            }
            unsafe { rust_debug_print(b"Client connected\0".as_ptr()); }
        }
    }

    // Stop advertising
    ble_stop_advertising()?;

    Ok(())
}

/// Set the message returned when the read characteristic is read
pub fn gatt_set_read_message(msg: &str) -> BleResult<()> {
    let c_msg = CString::new(msg).map_err(|_| BleError::InvalidParameter)?;
    unsafe { rust_ble_wrapper_gatt_set_read_msg(c_msg.as_ptr()); }
    Ok(())
}

/// Check if there's a pending GATT command
pub fn gatt_has_command() -> bool {
    unsafe { rust_ble_wrapper_gatt_has_command() != 0 }
}

/// Get the last received GATT command (clears the command buffer)
pub fn gatt_get_command() -> Option<Vec<u8>> {
    let mut buffer = [0u8; 64];
    let len = unsafe {
        rust_ble_wrapper_gatt_get_command(buffer.as_mut_ptr(), buffer.len() as c_int)
    };
    if len > 0 {
        Some(buffer[..len as usize].to_vec())
    } else {
        None
    }
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

/// Print debug status information for troubleshooting GATT issues
pub fn ble_debug_print_status() {
    unsafe { rust_ble_wrapper_debug_print_status(); }
}
