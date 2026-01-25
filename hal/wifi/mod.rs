//! WiFi HAL
//!
//! Provides WiFi functionality for scanning, connecting, and managing wireless networks.
//! Implementation is selected at compile time based on platform feature.
//!
//! - Linux: Uses nl80211 netlink API
//! - NuttX: Uses WEXT-style socket/ioctl (same as WAPI)

// Platform-specific implementations

// NuttX uses WEXT-style socket/ioctl
#[cfg(feature = "platform-nuttx")]
mod nuttx;
#[cfg(feature = "platform-nuttx")]
pub use nuttx::*;

// Linux uses nl80211 (stub for now)
#[cfg(feature = "platform-linux")]
mod linux;
#[cfg(feature = "platform-linux")]
pub use linux::*;

// Fallback stub for other platforms
#[cfg(not(any(feature = "platform-linux", feature = "platform-nuttx")))]
mod none;
#[cfg(not(any(feature = "platform-linux", feature = "platform-nuttx")))]
pub use none::*;

use core::fmt;

/// WiFi operation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiError {
    /// WiFi not initialized
    NotInitialized,
    /// Already initialized
    AlreadyInitialized,
    /// Socket creation failed
    SocketError,
    /// Interface not found
    InterfaceNotFound,
    /// Scan failed
    ScanFailed,
    /// Scan still in progress
    ScanInProgress,
    /// Connection failed
    ConnectionFailed,
    /// Authentication failed
    AuthenticationFailed,
    /// Network not found
    NetworkNotFound,
    /// Invalid password
    InvalidPassword,
    /// Timeout occurred
    Timeout,
    /// Configuration error
    ConfigurationError,
    /// Operation not supported on this platform
    NotSupported,
    /// System error with errno
    SystemError(i32),
}

impl fmt::Display for WifiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WifiError::NotInitialized => write!(f, "WiFi not initialized"),
            WifiError::AlreadyInitialized => write!(f, "WiFi already initialized"),
            WifiError::SocketError => write!(f, "Socket creation failed"),
            WifiError::InterfaceNotFound => write!(f, "Interface not found"),
            WifiError::ScanFailed => write!(f, "Scan failed"),
            WifiError::ScanInProgress => write!(f, "Scan in progress"),
            WifiError::ConnectionFailed => write!(f, "Connection failed"),
            WifiError::AuthenticationFailed => write!(f, "Authentication failed"),
            WifiError::NetworkNotFound => write!(f, "Network not found"),
            WifiError::InvalidPassword => write!(f, "Invalid password"),
            WifiError::Timeout => write!(f, "Timeout"),
            WifiError::ConfigurationError => write!(f, "Configuration error"),
            WifiError::NotSupported => write!(f, "Not supported on this platform"),
            WifiError::SystemError(e) => write!(f, "System error: {}", e),
        }
    }
}

/// Result type for WiFi operations
pub type WifiResult<T> = Result<T, WifiError>;

/// WiFi operating mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiMode {
    /// Automatically select mode
    Auto = 0,
    /// Ad-hoc (IBSS) mode
    AdHoc = 1,
    /// Infrastructure (managed/station) mode
    Station = 2,
    /// Access Point (master) mode
    AccessPoint = 3,
    /// Monitor mode
    Monitor = 6,
}

/// WiFi authentication mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthMode {
    /// Open (no authentication)
    #[default]
    Open,
    /// WEP (deprecated)
    Wep,
    /// WPA-PSK
    WpaPsk,
    /// WPA2-PSK
    Wpa2Psk,
    /// WPA3-PSK
    Wpa3Psk,
    /// WPA/WPA2 mixed
    WpaWpa2Psk,
    /// Unknown
    Unknown,
}

/// WiFi scan result
#[derive(Debug, Clone, Default)]
pub struct ScanResult {
    /// SSID (network name)
    pub ssid: [u8; 32],
    /// SSID length
    pub ssid_len: usize,
    /// BSSID (AP MAC address)
    pub bssid: [u8; 6],
    /// Channel number
    pub channel: u8,
    /// Signal strength in dBm
    pub rssi: i8,
    /// Authentication mode
    pub auth_mode: AuthMode,
}

impl ScanResult {
    /// Get SSID as string
    pub fn ssid_str(&self) -> Option<&str> {
        core::str::from_utf8(&self.ssid[..self.ssid_len]).ok()
    }

    /// Format BSSID as MAC address string
    pub fn bssid_str(&self) -> [u8; 17] {
        let mut buf = [0u8; 17];
        let hex = b"0123456789ABCDEF";
        for i in 0..6 {
            buf[i * 3] = hex[(self.bssid[i] >> 4) as usize];
            buf[i * 3 + 1] = hex[(self.bssid[i] & 0xF) as usize];
            if i < 5 {
                buf[i * 3 + 2] = b':';
            }
        }
        buf
    }
}

/// Station mode configuration
#[derive(Debug, Clone)]
pub struct StationConfig {
    /// SSID (network name)
    pub ssid: [u8; 32],
    /// SSID length
    pub ssid_len: usize,
    /// Password/passphrase
    pub password: [u8; 64],
    /// Password length
    pub password_len: usize,
    /// Optional specific BSSID
    pub bssid: Option<[u8; 6]>,
    /// Optional channel
    pub channel: Option<u8>,
    /// Authentication mode
    pub auth_mode: AuthMode,
}

impl StationConfig {
    /// Create a new station config from SSID and password strings
    pub fn new(ssid: &str, password: &str) -> Self {
        let mut config = Self {
            ssid: [0; 32],
            ssid_len: 0,
            password: [0; 64],
            password_len: 0,
            bssid: None,
            channel: None,
            auth_mode: AuthMode::Wpa2Psk,
        };

        let ssid_bytes = ssid.as_bytes();
        let len = core::cmp::min(ssid_bytes.len(), 32);
        config.ssid[..len].copy_from_slice(&ssid_bytes[..len]);
        config.ssid_len = len;

        let pwd_bytes = password.as_bytes();
        let len = core::cmp::min(pwd_bytes.len(), 64);
        config.password[..len].copy_from_slice(&pwd_bytes[..len]);
        config.password_len = len;

        config
    }
}

/// Connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Not connected
    Disconnected,
    /// Connecting in progress
    Connecting,
    /// Connected to AP
    Connected,
    /// Connection failed
    Failed,
}

/// IP configuration
#[derive(Debug, Clone, Copy)]
pub struct IpInfo {
    /// IP address
    pub ip: [u8; 4],
    /// Subnet mask
    pub netmask: [u8; 4],
    /// Gateway address
    pub gateway: [u8; 4],
}

impl fmt::Display for IpInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}.{}", self.ip[0], self.ip[1], self.ip[2], self.ip[3])
    }
}
