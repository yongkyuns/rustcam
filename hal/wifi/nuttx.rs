//! WiFi HAL for NuttX
//!
//! Uses WEXT-style socket/ioctl interface, similar to NuttX WAPI.
//! This works with ESP32S3 WiFi driver.

use super::{
    AuthMode, ConnectionStatus, IpInfo, ScanResult, StationConfig, WifiError, WifiMode, WifiResult,
};

/// Maximum ESSID size
const IW_ESSID_MAX_SIZE: usize = 32;

/// Scan buffer size
const IW_SCAN_MAX_DATA: usize = 4096;

// WEXT ioctl commands (from nuttx/wireless/wireless.h)
// These use _WLIOC macro which is _IOC(_WLIOCBASE, n)
// _WLIOCBASE = 0x8b00, so SIOCSIWSCAN = 0x8b00 + 0x18 = 0x8b18

const SIOCGIWNAME: i32 = 0x8b01;
const SIOCSIWFREQ: i32 = 0x8b04;
#[allow(dead_code)]
const SIOCGIWFREQ: i32 = 0x8b05;
const SIOCSIWMODE: i32 = 0x8b06;
const SIOCGIWMODE: i32 = 0x8b07;
const SIOCSIWAP: i32 = 0x8b14;
const SIOCGIWAP: i32 = 0x8b15;
const SIOCSIWSCAN: i32 = 0x8b18;
const SIOCGIWSCAN: i32 = 0x8b19;
const SIOCSIWESSID: i32 = 0x8b1a;
const SIOCGIWESSID: i32 = 0x8b1b;
#[allow(dead_code)]
const SIOCSIWENCODE: i32 = 0x8b2a;
#[allow(dead_code)]
const SIOCGIWENCODE: i32 = 0x8b2b;
const SIOCSIWAUTH: i32 = 0x8b32;
#[allow(dead_code)]
const SIOCGIWAUTH: i32 = 0x8b33;
const SIOCSIWENCODEEXT: i32 = 0x8b34;

// WiFi modes
const IW_MODE_AUTO: u32 = 0;
const IW_MODE_ADHOC: u32 = 1;
const IW_MODE_INFRA: u32 = 2;
const IW_MODE_MASTER: u32 = 3;
const IW_MODE_MONITOR: u32 = 6;

// ESSID flags
const IW_ESSID_ON: u16 = 1;

// Auth parameters
const IW_AUTH_WPA_VERSION: u16 = 0;
const IW_AUTH_CIPHER_PAIRWISE: u16 = 1;
const IW_AUTH_CIPHER_GROUP: u16 = 2;
const IW_AUTH_KEY_MGMT: u16 = 3;

// WPA versions
const IW_AUTH_WPA_VERSION_DISABLED: u32 = 0x01;
const IW_AUTH_WPA_VERSION_WPA: u32 = 0x02;
const IW_AUTH_WPA_VERSION_WPA2: u32 = 0x04;

// Cipher suites
const IW_AUTH_CIPHER_NONE: u32 = 0x01;
const IW_AUTH_CIPHER_TKIP: u32 = 0x04;
const IW_AUTH_CIPHER_CCMP: u32 = 0x08;

// Key management
const IW_AUTH_KEY_MGMT_PSK: u32 = 2;

// Encode algorithms
const IW_ENCODE_ALG_NONE: u16 = 0;
const IW_ENCODE_ALG_CCMP: u16 = 3;
const IW_ENCODE_ALG_PMK: u16 = 4;

// Wireless event types (for parsing scan results)
const SIOCGIWAP_EVENT: u16 = 0x8b15;
const SIOCGIWESSID_EVENT: u16 = 0x8b1b;
const SIOCGIWFREQ_EVENT: u16 = 0x8b05;
const SIOCGIWMODE_EVENT: u16 = 0x8b07;
const SIOCGIWENCODE_EVENT: u16 = 0x8b2b;
const IWEVQUAL: u16 = 0x8c01;

/// Default interface name
const DEFAULT_IFNAME: &[u8] = b"wlan0\0";

// NuttX-specific ioctl wrapper
// NuttX ioctl uses int for request, not unsigned long like Linux
extern "C" {
    fn ioctl(fd: libc::c_int, request: libc::c_int, ...) -> libc::c_int;
}

/// Get last OS error code using std::io
fn get_last_errno() -> i32 {
    std::io::Error::last_os_error()
        .raw_os_error()
        .unwrap_or(0)
}

/// EAGAIN error code (resource temporarily unavailable)
const EAGAIN: i32 = 11;

/// E2BIG error code (argument list too long / buffer too small)
const E2BIG: i32 = 7;

/// iw_point structure for data transfer
#[repr(C)]
#[derive(Copy, Clone)]
struct IwPoint {
    pointer: *mut libc::c_void,
    length: u16,
    flags: u16,
}

/// iw_param structure for simple parameters
#[repr(C)]
#[derive(Copy, Clone)]
struct IwParam {
    value: i32,
    fixed: u8,
    disabled: u8,
    flags: u16,
}

/// iw_freq structure for frequency/channel
#[repr(C)]
#[derive(Copy, Clone)]
struct IwFreq {
    m: i32,  // Mantissa
    e: i16,  // Exponent
    i: u8,   // List index
    flags: u8,
}

/// iw_quality structure
#[repr(C)]
#[derive(Copy, Clone)]
struct IwQuality {
    qual: u8,
    level: u8,
    noise: u8,
    updated: u8,
}

/// sockaddr structure for AP address
#[repr(C)]
#[derive(Copy, Clone)]
struct SockAddr {
    sa_family: u16,
    sa_data: [u8; 14],
}

/// Union for iwreq data
#[repr(C)]
#[derive(Copy, Clone)]
union IwReqData {
    name: [libc::c_char; 16],
    essid: IwPoint,
    nwid: IwParam,
    freq: IwFreq,
    sens: IwParam,
    bitrate: IwParam,
    txpower: IwParam,
    rts: IwParam,
    frag: IwParam,
    mode: u32,
    retry: IwParam,
    encoding: IwPoint,
    power: IwParam,
    qual: IwQuality,
    ap_addr: SockAddr,
    addr: SockAddr,
    param: IwParam,
    data: IwPoint,
}

/// iwreq structure for ioctl
#[repr(C)]
struct IwReq {
    ifr_name: [libc::c_char; 16],
    u: IwReqData,
}

impl IwReq {
    fn new() -> Self {
        let mut req: IwReq = unsafe { core::mem::zeroed() };
        // Copy interface name
        for (i, &b) in DEFAULT_IFNAME.iter().enumerate() {
            if i < 16 {
                req.ifr_name[i] = b as libc::c_char;
            }
        }
        req
    }
}

/// iw_event structure for parsing scan results
#[repr(C)]
struct IwEvent {
    len: u16,
    cmd: u16,
    // Followed by union iwreq_data
}

/// iw_encode_ext structure for WPA keys
#[repr(C)]
struct IwEncodeExt {
    ext_flags: u32,
    tx_seq: [u8; 8],
    rx_seq: [u8; 8],
    addr: SockAddr,
    alg: u16,
    key_len: u16,
    // key data follows
}

/// Global state
static mut INITIALIZED: bool = false;

/// Create a socket for ioctl operations
fn make_socket() -> WifiResult<i32> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        return Err(WifiError::SocketError);
    }
    Ok(fd)
}

/// Close socket
fn close_socket(fd: i32) {
    unsafe { libc::close(fd); }
}

/// Initialize WiFi subsystem
pub fn wifi_initialize() -> WifiResult<()> {
    unsafe {
        if INITIALIZED {
            // Already initialized - this is fine
            return Ok(());
        }

        // Verify interface exists by checking if we can get its name
        let fd = make_socket()?;
        let mut req = IwReq::new();

        let ret = ioctl(fd, SIOCGIWNAME, &mut req as *mut IwReq);
        close_socket(fd);

        if ret < 0 {
            return Err(WifiError::InterfaceNotFound);
        }

        INITIALIZED = true;
        Ok(())
    }
}

/// Deinitialize WiFi subsystem
pub fn wifi_deinitialize() -> WifiResult<()> {
    unsafe {
        if !INITIALIZED {
            return Err(WifiError::NotInitialized);
        }
        INITIALIZED = false;
        Ok(())
    }
}

/// Check if WiFi is initialized
pub fn wifi_is_initialized() -> bool {
    unsafe { INITIALIZED }
}

/// Set WiFi operating mode
pub fn wifi_set_mode(mode: WifiMode) -> WifiResult<()> {
    let fd = make_socket()?;
    let mut req = IwReq::new();

    let iw_mode = match mode {
        WifiMode::Auto => IW_MODE_AUTO,
        WifiMode::AdHoc => IW_MODE_ADHOC,
        WifiMode::Station => IW_MODE_INFRA,
        WifiMode::AccessPoint => IW_MODE_MASTER,
        WifiMode::Monitor => IW_MODE_MONITOR,
    };

    req.u.mode = iw_mode;

    let ret = unsafe { ioctl(fd, SIOCSIWMODE, &mut req as *mut IwReq) };
    close_socket(fd);

    if ret < 0 {
        return Err(WifiError::ConfigurationError);
    }

    Ok(())
}

/// Get WiFi operating mode
pub fn wifi_get_mode() -> WifiResult<WifiMode> {
    let fd = make_socket()?;
    let mut req = IwReq::new();

    let ret = unsafe { ioctl(fd, SIOCGIWMODE, &mut req as *mut IwReq) };
    close_socket(fd);

    if ret < 0 {
        return Err(WifiError::ConfigurationError);
    }

    let mode = unsafe { req.u.mode };
    Ok(match mode {
        IW_MODE_AUTO => WifiMode::Auto,
        IW_MODE_ADHOC => WifiMode::AdHoc,
        IW_MODE_INFRA => WifiMode::Station,
        IW_MODE_MASTER => WifiMode::AccessPoint,
        IW_MODE_MONITOR => WifiMode::Monitor,
        _ => WifiMode::Auto,
    })
}

/// Start WiFi scan
pub fn wifi_start_scan() -> WifiResult<()> {
    let fd = make_socket()?;
    let mut req = IwReq::new();

    // Set up scan request with no specific ESSID (scan all)
    req.u.data = IwPoint {
        pointer: core::ptr::null_mut(),
        length: 0,
        flags: 0,
    };

    let ret = unsafe { ioctl(fd, SIOCSIWSCAN, &mut req as *mut IwReq) };
    close_socket(fd);

    if ret < 0 {
        return Err(WifiError::ScanFailed);
    }

    Ok(())
}

/// Check if scan is complete
/// Returns Ok(true) if results are ready, Ok(false) if still scanning
pub fn wifi_scan_is_complete() -> WifiResult<bool> {
    let fd = make_socket()?;
    let mut req = IwReq::new();

    // Use a 1-byte buffer - we just want to check status, not get results
    // If data is ready but buffer is too small, we get E2BIG which means success
    let mut buffer = [0u8; 1];

    req.u.data = IwPoint {
        pointer: buffer.as_mut_ptr() as *mut libc::c_void,
        length: buffer.len() as u16,
        flags: 0,
    };

    let ret = unsafe { ioctl(fd, SIOCGIWSCAN, &mut req as *mut IwReq) };
    let errno_val = if ret < 0 { get_last_errno() } else { 0 };
    close_socket(fd);

    if ret < 0 {
        // E2BIG means data is ready but buffer too small - this is expected success
        if errno_val == E2BIG {
            return Ok(true);
        }
        // EAGAIN means scan is still in progress
        if errno_val == EAGAIN {
            return Ok(false);
        }
        return Err(WifiError::ScanFailed);
    }

    // ioctl succeeded - data is ready
    Ok(true)
}

/// Get scan results
/// Call wifi_start_scan() first and wait for wifi_scan_is_complete() to return true
pub fn wifi_get_scan_results() -> WifiResult<([ScanResult; 16], usize)> {
    let fd = make_socket()?;
    let mut req = IwReq::new();
    let mut buffer = [0u8; IW_SCAN_MAX_DATA];

    req.u.data = IwPoint {
        pointer: buffer.as_mut_ptr() as *mut libc::c_void,
        length: buffer.len() as u16,
        flags: 0,
    };

    let ret = unsafe { ioctl(fd, SIOCGIWSCAN, &mut req as *mut IwReq) };
    let errno_val = if ret < 0 { get_last_errno() } else { 0 };
    close_socket(fd);

    if ret < 0 {
        if errno_val == EAGAIN {
            return Err(WifiError::ScanInProgress);
        }
        return Err(WifiError::ScanFailed);
    }

    // Parse scan results from iw_event stream
    let data_len = unsafe { req.u.data.length } as usize;
    let mut results: [ScanResult; 16] = unsafe { core::mem::zeroed() };
    let mut count = 0;

    let mut offset = 0;
    let mut current_result: ScanResult = unsafe { core::mem::zeroed() };
    let mut has_result = false;

    while offset + 4 <= data_len && count < 16 {
        // Read iw_event header (len and cmd)
        let len = u16::from_ne_bytes([buffer[offset], buffer[offset + 1]]) as usize;
        let cmd = u16::from_ne_bytes([buffer[offset + 2], buffer[offset + 3]]);

        if len < 4 || offset + len > data_len {
            break;
        }

        let event_data = &buffer[offset + 4..offset + len];

        match cmd {
            SIOCGIWAP_EVENT => {
                // New AP - save previous result if any
                if has_result && current_result.ssid_len > 0 {
                    results[count] = current_result;
                    count += 1;
                }
                // Start new result
                current_result = unsafe { core::mem::zeroed() };
                has_result = true;

                // Extract BSSID from sockaddr (skip sa_family)
                if event_data.len() >= 8 {
                    current_result.bssid.copy_from_slice(&event_data[2..8]);
                }
            }
            SIOCGIWESSID_EVENT => {
                // ESSID - extract from iw_point
                if event_data.len() >= 8 {
                    let essid_len = u16::from_ne_bytes([event_data[4], event_data[5]]) as usize;
                    let essid_len = core::cmp::min(essid_len, 32);

                    // The actual ESSID follows the iw_point structure
                    if offset + 4 + 8 + essid_len <= data_len {
                        let essid_start = offset + 4 + 8;
                        let essid_data = &buffer[essid_start..essid_start + essid_len];
                        current_result.ssid[..essid_len].copy_from_slice(essid_data);
                        current_result.ssid_len = essid_len;
                    }
                }
            }
            SIOCGIWFREQ_EVENT => {
                // Frequency/channel
                if event_data.len() >= 8 {
                    let m = i32::from_ne_bytes([
                        event_data[0], event_data[1], event_data[2], event_data[3]
                    ]);
                    let e = i16::from_ne_bytes([event_data[4], event_data[5]]);

                    // Convert frequency to channel
                    let freq_mhz = if e == 0 {
                        m as u32 // Already a channel number
                    } else {
                        // Calculate actual frequency in MHz
                        let mut freq = m as f64;
                        for _ in 0..e.abs() {
                            if e > 0 {
                                freq *= 10.0;
                            } else {
                                freq /= 10.0;
                            }
                        }
                        (freq / 1_000_000.0) as u32
                    };

                    // Frequency to channel conversion (2.4 GHz band)
                    current_result.channel = if freq_mhz < 15 {
                        freq_mhz as u8 // Already a channel
                    } else if freq_mhz >= 2412 && freq_mhz <= 2484 {
                        if freq_mhz == 2484 {
                            14
                        } else {
                            ((freq_mhz - 2412) / 5 + 1) as u8
                        }
                    } else {
                        0
                    };
                }
            }
            IWEVQUAL => {
                // Quality/signal level
                if event_data.len() >= 4 {
                    // level is second byte, typically in dBm when DBM flag is set
                    // Cast u8 to i8 directly - values 0-127 stay positive, 128-255 become negative
                    current_result.rssi = event_data[1] as i8;
                }
            }
            SIOCGIWENCODE_EVENT => {
                // Encoding (indicates encryption)
                if event_data.len() >= 6 {
                    let flags = u16::from_ne_bytes([event_data[6], event_data[7]]);
                    // Check if encoding is disabled
                    if flags & 0x8000 != 0 {
                        current_result.auth_mode = AuthMode::Open;
                    } else {
                        current_result.auth_mode = AuthMode::Wpa2Psk; // Assume WPA2 for now
                    }
                }
            }
            _ => {}
        }

        offset += len;
    }

    // Don't forget the last result
    if has_result && current_result.ssid_len > 0 && count < 16 {
        results[count] = current_result;
        count += 1;
    }

    Ok((results, count))
}

/// Set authentication parameters
fn set_auth_param(fd: i32, idx: u16, value: u32) -> WifiResult<()> {
    let mut req = IwReq::new();

    req.u.param = IwParam {
        value: value as i32,
        fixed: 0,
        disabled: 0,
        flags: idx,
    };

    let ret = unsafe { ioctl(fd, SIOCSIWAUTH, &mut req as *mut IwReq) };
    if ret < 0 {
        return Err(WifiError::ConfigurationError);
    }
    Ok(())
}

/// Set WPA key using SIOCSIWENCODEEXT
fn set_key_ext(fd: i32, alg: u16, key: &[u8]) -> WifiResult<()> {
    // Create buffer for iw_encode_ext + key
    let mut buf = [0u8; 128];

    // Fill in iw_encode_ext structure
    let ext = unsafe { &mut *(buf.as_mut_ptr() as *mut IwEncodeExt) };
    ext.ext_flags = 0;
    ext.alg = alg;
    ext.key_len = key.len() as u16;

    // Set broadcast address
    ext.addr.sa_family = 1; // ARPHRD_ETHER
    for i in 0..6 {
        ext.addr.sa_data[i] = 0xff;
    }

    // Copy key after the structure
    let key_offset = core::mem::size_of::<IwEncodeExt>();
    let key_len = core::cmp::min(key.len(), buf.len() - key_offset);
    buf[key_offset..key_offset + key_len].copy_from_slice(&key[..key_len]);

    let mut req = IwReq::new();
    req.u.encoding = IwPoint {
        pointer: buf.as_mut_ptr() as *mut libc::c_void,
        length: (key_offset + key_len) as u16,
        flags: 0,
    };

    let ret = unsafe { ioctl(fd, SIOCSIWENCODEEXT, &mut req as *mut IwReq) };
    if ret < 0 {
        return Err(WifiError::ConfigurationError);
    }
    Ok(())
}

// Debug print helper for NuttX
#[cfg(feature = "platform-nuttx")]
fn wifi_debug(msg: &[u8]) {
    extern "C" {
        fn puts(s: *const u8) -> i32;
    }
    unsafe { puts(msg.as_ptr()); }
}

#[cfg(not(feature = "platform-nuttx"))]
fn wifi_debug(_msg: &[u8]) {}

/// Connect to WiFi network
pub fn wifi_connect(config: &StationConfig) -> WifiResult<()> {
    wifi_debug(b"[WIFI] wifi_connect starting\0");

    let fd = make_socket()?;
    let mut req = IwReq::new();

    // 1. Set mode to infrastructure (station)
    wifi_debug(b"[WIFI] Setting mode to INFRA\0");
    req.u.mode = IW_MODE_INFRA;
    let ret = unsafe { ioctl(fd, SIOCSIWMODE, &mut req as *mut IwReq) };
    if ret < 0 {
        wifi_debug(b"[WIFI] SIOCSIWMODE failed\0");
        close_socket(fd);
        return Err(WifiError::ConfigurationError);
    }
    wifi_debug(b"[WIFI] Mode set OK\0");

    // 2. Set authentication parameters based on auth mode
    let (wpa_version, cipher) = match config.auth_mode {
        AuthMode::Open => (IW_AUTH_WPA_VERSION_DISABLED, IW_AUTH_CIPHER_NONE),
        AuthMode::Wep => (IW_AUTH_WPA_VERSION_DISABLED, IW_AUTH_CIPHER_NONE),
        AuthMode::WpaPsk => (IW_AUTH_WPA_VERSION_WPA, IW_AUTH_CIPHER_TKIP),
        AuthMode::Wpa2Psk | AuthMode::WpaWpa2Psk | AuthMode::Wpa3Psk => {
            (IW_AUTH_WPA_VERSION_WPA2, IW_AUTH_CIPHER_CCMP)
        }
        AuthMode::Unknown => (IW_AUTH_WPA_VERSION_WPA2, IW_AUTH_CIPHER_CCMP),
    };

    // Set WPA version
    wifi_debug(b"[WIFI] Setting WPA version\0");
    if let Err(e) = set_auth_param(fd, IW_AUTH_WPA_VERSION, wpa_version) {
        wifi_debug(b"[WIFI] WPA version FAILED\0");
        close_socket(fd);
        return Err(e);
    }
    wifi_debug(b"[WIFI] WPA version OK\0");

    // Set cipher for pairwise and group
    if cipher != IW_AUTH_CIPHER_NONE {
        wifi_debug(b"[WIFI] Setting ciphers\0");
        let _ = set_auth_param(fd, IW_AUTH_CIPHER_PAIRWISE, cipher);
        let _ = set_auth_param(fd, IW_AUTH_CIPHER_GROUP, cipher);
        let _ = set_auth_param(fd, IW_AUTH_KEY_MGMT, IW_AUTH_KEY_MGMT_PSK);
        wifi_debug(b"[WIFI] Ciphers set\0");
    }

    // 3. Set passphrase if using WPA/WPA2
    if config.password_len > 0 && config.auth_mode != AuthMode::Open {
        let alg = if cipher == IW_AUTH_CIPHER_CCMP {
            IW_ENCODE_ALG_CCMP
        } else {
            IW_ENCODE_ALG_PMK
        };

        wifi_debug(b"[WIFI] Setting passphrase\0");
        if let Err(e) = set_key_ext(fd, alg, &config.password[..config.password_len]) {
            wifi_debug(b"[WIFI] Passphrase FAILED\0");
            close_socket(fd);
            return Err(e);
        }
        wifi_debug(b"[WIFI] Passphrase OK\0");
    }

    // 4. Set channel if specified
    if let Some(channel) = config.channel {
        wifi_debug(b"[WIFI] Setting channel\0");
        req.u.freq = IwFreq {
            m: channel as i32,
            e: 0,
            i: 0,
            flags: 0,
        };
        let _ = unsafe { ioctl(fd, SIOCSIWFREQ, &mut req as *mut IwReq) };
    }

    // 5. Set BSSID if specified
    if let Some(bssid) = config.bssid {
        wifi_debug(b"[WIFI] Setting BSSID\0");
        req.u.ap_addr = SockAddr {
            sa_family: 1, // ARPHRD_ETHER
            sa_data: [0; 14],
        };
        unsafe {
            req.u.ap_addr.sa_data[..6].copy_from_slice(&bssid);
        }
        let _ = unsafe { ioctl(fd, SIOCSIWAP, &mut req as *mut IwReq) };
    }

    // 6. Set ESSID (this triggers the connection)
    wifi_debug(b"[WIFI] Setting ESSID\0");
    let mut essid_buf = [0u8; IW_ESSID_MAX_SIZE + 1];
    essid_buf[..config.ssid_len].copy_from_slice(&config.ssid[..config.ssid_len]);

    req.u.essid = IwPoint {
        pointer: essid_buf.as_mut_ptr() as *mut libc::c_void,
        length: config.ssid_len as u16,
        flags: IW_ESSID_ON,
    };

    let ret = unsafe { ioctl(fd, SIOCSIWESSID, &mut req as *mut IwReq) };
    let errno_val = if ret < 0 { get_last_errno() } else { 0 };
    close_socket(fd);

    if ret < 0 {
        unsafe {
            extern "C" {
                fn printf(format: *const u8, ...) -> i32;
            }
            printf(b"[WIFI] ESSID set FAILED, errno=%d\n\0".as_ptr(), errno_val);
        }
        return Err(WifiError::ConnectionFailed);
    }

    wifi_debug(b"[WIFI] Connection initiated OK\0");
    Ok(())
}

/// Disconnect from WiFi network
pub fn wifi_disconnect() -> WifiResult<()> {
    let fd = make_socket()?;
    let mut req = IwReq::new();

    // Set ESSID with flag=0 (off) to disconnect
    let mut essid_buf = [0u8; IW_ESSID_MAX_SIZE + 1];
    req.u.essid = IwPoint {
        pointer: essid_buf.as_mut_ptr() as *mut libc::c_void,
        length: 0,
        flags: 0, // IW_ESSID_OFF
    };

    let ret = unsafe { ioctl(fd, SIOCSIWESSID, &mut req as *mut IwReq) };
    close_socket(fd);

    if ret < 0 {
        return Err(WifiError::ConnectionFailed);
    }

    Ok(())
}

/// Get current connection status
pub fn wifi_get_connection_status() -> WifiResult<ConnectionStatus> {
    let fd = make_socket()?;
    let mut req = IwReq::new();

    // Get current AP address
    let ret = unsafe { ioctl(fd, SIOCGIWAP, &mut req as *mut IwReq) };
    close_socket(fd);

    if ret < 0 {
        return Ok(ConnectionStatus::Disconnected);
    }

    // Check if we have a valid AP address (not all zeros or all ones)
    let ap_addr = unsafe { &req.u.ap_addr.sa_data[..6] };
    let all_zero = ap_addr.iter().all(|&b| b == 0);
    let all_ones = ap_addr.iter().all(|&b| b == 0xff);

    if all_zero || all_ones {
        Ok(ConnectionStatus::Disconnected)
    } else {
        Ok(ConnectionStatus::Connected)
    }
}

/// Get current ESSID (connected network name)
pub fn wifi_get_essid() -> WifiResult<([u8; 32], usize)> {
    let fd = make_socket()?;
    let mut req = IwReq::new();
    let mut essid_buf = [0u8; IW_ESSID_MAX_SIZE + 1];

    req.u.essid = IwPoint {
        pointer: essid_buf.as_mut_ptr() as *mut libc::c_void,
        length: IW_ESSID_MAX_SIZE as u16,
        flags: 0,
    };

    let ret = unsafe { ioctl(fd, SIOCGIWESSID, &mut req as *mut IwReq) };
    close_socket(fd);

    if ret < 0 {
        return Err(WifiError::NotInitialized);
    }

    let len = unsafe { req.u.essid.length } as usize;
    let len = core::cmp::min(len, 32);

    let mut result = [0u8; 32];
    result[..len].copy_from_slice(&essid_buf[..len]);

    Ok((result, len))
}

/// Get IP information (requires DHCP to have completed)
pub fn wifi_get_ip_info() -> WifiResult<IpInfo> {
    // Use SIOCGIFADDR to get IP address
    let fd = make_socket()?;

    #[repr(C)]
    struct SockAddrIn {
        sin_family: u16,
        sin_port: u16,
        sin_addr: [u8; 4],
        sin_zero: [u8; 8],
    }

    #[repr(C)]
    struct IfReq {
        ifr_name: [libc::c_char; 16],
        ifr_addr: SockAddrIn,
    }

    let mut req: IfReq = unsafe { core::mem::zeroed() };
    for (i, &b) in DEFAULT_IFNAME.iter().enumerate() {
        if i < 16 {
            req.ifr_name[i] = b as libc::c_char;
        }
    }

    // Standard network ioctls
    const SIOCGIFADDR: i32 = 0x8915;
    const SIOCGIFNETMASK: i32 = 0x891b;

    let mut info = IpInfo {
        ip: [0; 4],
        netmask: [0; 4],
        gateway: [0; 4],
    };

    // Get IP address
    let ret = unsafe { ioctl(fd, SIOCGIFADDR, &mut req as *mut IfReq) };
    if ret >= 0 {
        info.ip = req.ifr_addr.sin_addr;
    }

    // Get netmask
    let ret = unsafe { ioctl(fd, SIOCGIFNETMASK, &mut req as *mut IfReq) };
    if ret >= 0 {
        info.netmask = req.ifr_addr.sin_addr;
    }

    close_socket(fd);

    // Gateway would require reading routing table, skip for now

    Ok(info)
}

/// Get signal strength (RSSI) of current connection
pub fn wifi_get_rssi() -> WifiResult<i8> {
    // This would require getting link quality stats
    // For now return a placeholder
    Err(WifiError::NotSupported)
}

/// Get MAC address of WiFi interface
pub fn wifi_get_mac_address() -> WifiResult<[u8; 6]> {
    let fd = make_socket()?;

    // Use ifreq struct matching NuttX's definition
    // struct ifreq is typically 32 bytes: 16 for name + 16 for union
    #[repr(C)]
    struct IfReq {
        ifr_name: [u8; 16],
        ifr_hwaddr: SockAddr,
    }

    let mut req: IfReq = unsafe { core::mem::zeroed() };

    // Copy interface name (without null terminator length issues)
    let ifname = b"wlan0";
    for (i, &b) in ifname.iter().enumerate() {
        req.ifr_name[i] = b;
    }

    // Standard network ioctl for hardware address
    const SIOCGIFHWADDR: i32 = 0x8927;

    let ret = unsafe { ioctl(fd, SIOCGIFHWADDR, &mut req as *mut IfReq) };
    let errno_val = if ret < 0 { get_last_errno() } else { 0 };
    close_socket(fd);

    if ret < 0 {
        // ENODEV (19) or ENXIO (6) means device not found
        // ENOTTY (25) means ioctl not supported
        if errno_val == 19 || errno_val == 6 {
            return Err(WifiError::InterfaceNotFound);
        }
        if errno_val == 25 {
            return Err(WifiError::NotSupported);
        }
        return Err(WifiError::InterfaceNotFound);
    }

    let mut mac = [0u8; 6];
    mac.copy_from_slice(&req.ifr_hwaddr.sa_data[..6]);
    Ok(mac)
}
