//! BLE (Bluetooth Low Energy) HAL
//!
//! Provides BLE functionality using AF_BLUETOOTH raw sockets (Linux BlueZ API).
//! Implementation is selected at compile time based on platform feature.
//!
//! Note: Bluetooth sockets are a Linux extension, not POSIX standard.

// Platform-specific implementations

// Linux uses BlueZ raw HCI sockets (requires socket2)
#[cfg(feature = "platform-linux")]
mod unix;
#[cfg(feature = "platform-linux")]
pub use unix::*;

// NuttX uses Apache NimBLE stack
#[cfg(feature = "platform-nuttx")]
mod nuttx;
#[cfg(feature = "platform-nuttx")]
pub use nuttx::*;

// Fallback stub for other platforms
#[cfg(not(any(feature = "platform-linux", feature = "platform-nuttx")))]
mod none;
#[cfg(not(any(feature = "platform-linux", feature = "platform-nuttx")))]
pub use none::*;

use core::fmt;

/// BLE error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BleError {
    /// BLE not initialized
    NotInitialized,
    /// Already initialized
    AlreadyInitialized,
    /// Socket creation failed
    SocketError,
    /// Bind failed
    BindError,
    /// Scan failed
    ScanError,
    /// Connection failed
    ConnectionError,
    /// Disconnection failed
    DisconnectionError,
    /// GATT operation failed
    GattError,
    /// Timeout occurred
    Timeout,
    /// Invalid parameter
    InvalidParameter,
    /// Operation not supported on this platform
    NotSupported,
    /// Permission denied
    PermissionDenied,
    /// Device not found
    DeviceNotFound,
    /// No adapter available
    NoAdapter,
}

impl fmt::Display for BleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BleError::NotInitialized => write!(f, "BLE not initialized"),
            BleError::AlreadyInitialized => write!(f, "BLE already initialized"),
            BleError::SocketError => write!(f, "Socket creation failed"),
            BleError::BindError => write!(f, "Bind failed"),
            BleError::ScanError => write!(f, "Scan failed"),
            BleError::ConnectionError => write!(f, "Connection failed"),
            BleError::DisconnectionError => write!(f, "Disconnection failed"),
            BleError::GattError => write!(f, "GATT operation failed"),
            BleError::Timeout => write!(f, "Timeout occurred"),
            BleError::InvalidParameter => write!(f, "Invalid parameter"),
            BleError::NotSupported => write!(f, "Not supported on this platform"),
            BleError::PermissionDenied => write!(f, "Permission denied"),
            BleError::DeviceNotFound => write!(f, "Device not found"),
            BleError::NoAdapter => write!(f, "No Bluetooth adapter available"),
        }
    }
}

/// Result type for BLE operations
pub type BleResult<T> = Result<T, BleError>;

/// Bluetooth address (6 bytes, big-endian)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BleAddress {
    pub bytes: [u8; 6],
}

impl BleAddress {
    /// Create a new BLE address from bytes
    pub fn new(bytes: [u8; 6]) -> Self {
        Self { bytes }
    }

    /// Create a BLE address from a string like "AA:BB:CC:DD:EE:FF"
    pub fn from_str(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 6 {
            return None;
        }

        let mut bytes = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            bytes[i] = u8::from_str_radix(part, 16).ok()?;
        }
        Some(Self { bytes })
    }
}

impl fmt::Display for BleAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.bytes[0],
            self.bytes[1],
            self.bytes[2],
            self.bytes[3],
            self.bytes[4],
            self.bytes[5]
        )
    }
}

/// Address type for BLE devices
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressType {
    /// Public device address
    Public,
    /// Random device address
    Random,
}

/// BLE scan result
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// Device address
    pub address: BleAddress,
    /// Address type
    pub address_type: AddressType,
    /// RSSI (signal strength) in dBm
    pub rssi: i8,
    /// Device name (if available from advertising data)
    pub name: Option<[u8; 32]>,
    /// Name length (valid bytes in name array)
    pub name_len: usize,
}

impl ScanResult {
    /// Get the device name as a string slice
    pub fn name_str(&self) -> Option<&str> {
        self.name.as_ref().and_then(|n| {
            core::str::from_utf8(&n[..self.name_len]).ok()
        })
    }
}

/// Handle to a BLE connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionHandle(pub u16);

/// UUID for GATT services and characteristics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uuid {
    /// UUID bytes (16 bytes for 128-bit UUID, first 2 for 16-bit)
    pub bytes: [u8; 16],
    /// True if this is a 16-bit UUID
    pub is_16bit: bool,
}

impl Uuid {
    /// Create a 16-bit UUID
    pub fn from_u16(uuid: u16) -> Self {
        let mut bytes = [0u8; 16];
        bytes[0] = (uuid >> 8) as u8;
        bytes[1] = (uuid & 0xFF) as u8;
        // Standard Bluetooth Base UUID
        bytes[2] = 0x00;
        bytes[3] = 0x00;
        bytes[4] = 0x00;
        bytes[5] = 0x00;
        bytes[6] = 0x10;
        bytes[7] = 0x00;
        bytes[8] = 0x80;
        bytes[9] = 0x00;
        bytes[10] = 0x00;
        bytes[11] = 0x80;
        bytes[12] = 0x5F;
        bytes[13] = 0x9B;
        bytes[14] = 0x34;
        bytes[15] = 0xFB;
        Self { bytes, is_16bit: true }
    }

    /// Create a 128-bit UUID
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self { bytes, is_16bit: false }
    }
}

/// Handle to a GATT characteristic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacteristicHandle {
    /// Connection this characteristic belongs to
    pub connection: ConnectionHandle,
    /// Attribute handle
    pub handle: u16,
    /// Value handle
    pub value_handle: u16,
}
