//! Unix BLE implementation using AF_BLUETOOTH raw sockets
//!
//! Uses HCI (Host Controller Interface) for BLE operations.
//! Works on both Linux and NuttX via Linux-compatible BlueZ socket API.
//!
//! Note: AF_BLUETOOTH is a Linux extension, not part of POSIX.
//! NuttX implements the Linux BlueZ socket API for Bluetooth support.

use super::{
    AddressType, BleAddress, BleError, BleResult, CharacteristicHandle, ConnectionHandle,
    ScanResult, Uuid,
};
use socket2::{Domain, Protocol, Socket, Type};
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::sync::Mutex;
use std::time::Duration;

// Bluetooth socket constants (Linux BlueZ extension)
const AF_BLUETOOTH: i32 = 31;
const BTPROTO_HCI: i32 = 1;
const SOL_HCI: i32 = 0;
const HCI_FILTER: i32 = 2;

// HCI channels
const HCI_CHANNEL_RAW: u16 = 0;
const HCI_CHANNEL_USER: u16 = 1; // Exclusive access, bypasses BlueZ

// HCI packet types
const HCI_COMMAND_PKT: u8 = 0x01;
const HCI_EVENT_PKT: u8 = 0x04;

// HCI commands (OGF << 10 | OCF)
const HCI_OP_RESET: u16 = 0x0C03;
const HCI_OP_SET_EVENT_MASK: u16 = 0x0C01;
const HCI_OP_LE_SET_EVENT_MASK: u16 = 0x2001;
const HCI_OP_LE_SET_RANDOM_ADDR: u16 = 0x2005;
const HCI_OP_LE_SET_ADV_PARAM: u16 = 0x2006;
const HCI_OP_LE_SET_ADV_DATA: u16 = 0x2008;
const HCI_OP_LE_SET_SCAN_RSP_DATA: u16 = 0x2009;
const HCI_OP_LE_SET_ADV_ENABLE: u16 = 0x200A;
const HCI_OP_LE_SET_SCAN_PARAM: u16 = 0x200B;
const HCI_OP_LE_SET_SCAN_ENABLE: u16 = 0x200C;

// HCI events
const HCI_EV_DISCONN_COMPLETE: u8 = 0x05;
const HCI_EV_LE_META: u8 = 0x3E;
const HCI_EV_LE_CONN_COMPLETE: u8 = 0x01;
const HCI_EV_LE_ADVERTISING_REPORT: u8 = 0x02;

// L2CAP
const L2CAP_CID_ATT: u16 = 0x0004; // ATT channel

// ATT opcodes
const ATT_OP_ERROR_RSP: u8 = 0x01;
const ATT_OP_MTU_REQ: u8 = 0x02;
const ATT_OP_MTU_RSP: u8 = 0x03;
const ATT_OP_FIND_INFO_REQ: u8 = 0x04;
const ATT_OP_FIND_INFO_RSP: u8 = 0x05;
const ATT_OP_READ_BY_TYPE_REQ: u8 = 0x08;
const ATT_OP_READ_BY_TYPE_RSP: u8 = 0x09;
const ATT_OP_READ_REQ: u8 = 0x0A;
const ATT_OP_READ_RSP: u8 = 0x0B;
const ATT_OP_READ_BY_GROUP_REQ: u8 = 0x10;
const ATT_OP_READ_BY_GROUP_RSP: u8 = 0x11;
const ATT_OP_WRITE_REQ: u8 = 0x12;
const ATT_OP_WRITE_RSP: u8 = 0x13;
const ATT_OP_WRITE_CMD: u8 = 0x52;

// ATT error codes
const ATT_ERR_ATTR_NOT_FOUND: u8 = 0x0A;

// Scan types
const LE_SCAN_ACTIVE: u8 = 0x01;

// Address types
const LE_PUBLIC_ADDRESS: u8 = 0x00;
const LE_RANDOM_ADDRESS: u8 = 0x01;

// Maximum scan results to store
const MAX_SCAN_RESULTS: usize = 32;

/// HCI socket address structure (Bluetooth-specific, not in std or socket2)
#[repr(C)]
struct SockaddrHci {
    hci_family: u16,
    hci_dev: u16,
    hci_channel: u16,
}

/// HCI filter structure (Bluetooth-specific)
#[repr(C)]
struct HciFilter {
    type_mask: u32,
    event_mask: [u32; 2],
    opcode: u16,
}

// =============================================================================
// Bluetooth-specific socket operations (still need libc for these)
// =============================================================================

mod bluetooth {
    use super::*;

    /// Bind socket to HCI device (socket2 doesn't know about sockaddr_hci)
    pub fn bind_hci(socket: &Socket, dev_id: u16, channel: u16) -> std::io::Result<()> {
        let addr = SockaddrHci {
            hci_family: AF_BLUETOOTH as u16,
            hci_dev: dev_id,
            hci_channel: channel,
        };
        // SAFETY: bind() with valid fd and properly sized sockaddr struct
        let ret = unsafe {
            libc::bind(
                socket.as_raw_fd(),
                &addr as *const SockaddrHci as *const libc::sockaddr,
                std::mem::size_of::<SockaddrHci>() as libc::socklen_t,
            )
        };
        if ret < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Set HCI filter on socket (socket2 doesn't know about HCI filters)
    pub fn set_hci_filter(socket: &Socket, filter: &HciFilter) -> std::io::Result<()> {
        // SAFETY: setsockopt with valid fd and properly sized filter struct
        let ret = unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                SOL_HCI,
                HCI_FILTER,
                filter as *const HciFilter as *const libc::c_void,
                std::mem::size_of::<HciFilter>() as libc::socklen_t,
            )
        };
        if ret < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

// =============================================================================
// HCI Socket wrapper using socket2
// =============================================================================

/// HCI socket wrapper using socket2 for safe socket management
struct HciSocket {
    socket: Socket,
    channel: u16,
}

impl HciSocket {
    /// Create and bind an HCI socket to the specified device
    fn new(dev_id: u16) -> BleResult<Self> {
        // Create Bluetooth HCI socket using socket2
        let domain = Domain::from(AF_BLUETOOTH);
        let socket = Socket::new(domain, Type::RAW, Some(Protocol::from(BTPROTO_HCI)))
            .map_err(|e| {
                if e.raw_os_error() == Some(libc::EPERM) || e.raw_os_error() == Some(libc::EACCES) {
                    BleError::PermissionDenied
                } else {
                    BleError::SocketError
                }
            })?;

        // Try HCI_CHANNEL_USER first (exclusive access, bypasses BlueZ)
        // Falls back to HCI_CHANNEL_RAW if USER channel fails (adapter must be down for USER)
        if bluetooth::bind_hci(&socket, dev_id, HCI_CHANNEL_USER).is_ok() {
            eprintln!("  [DEBUG] Using HCI_CHANNEL_USER (exclusive access)");
            let mut hci = Self { socket, channel: HCI_CHANNEL_USER };
            // Initialize controller for USER channel
            hci.init_user_channel()?;
            return Ok(hci);
        }

        // Retry with new socket for RAW channel
        drop(socket);
        let socket = Socket::new(domain, Type::RAW, Some(Protocol::from(BTPROTO_HCI)))
            .map_err(|_| BleError::SocketError)?;
        bluetooth::bind_hci(&socket, dev_id, HCI_CHANNEL_RAW).map_err(|_| BleError::NoAdapter)?;
        eprintln!("  [DEBUG] Using HCI_CHANNEL_RAW (shared with BlueZ)");

        // Set up HCI filter for RAW channel (not needed for USER channel)
        let filter = HciFilter {
            type_mask: 1 << HCI_EVENT_PKT,
            event_mask: [0xFFFFFFFF, 0xFFFFFFFF],
            opcode: 0,
        };
        bluetooth::set_hci_filter(&socket, &filter).map_err(|_| BleError::SocketError)?;

        Ok(Self { socket, channel: HCI_CHANNEL_RAW })
    }

    /// Set read timeout using socket2's API
    fn set_read_timeout(&self, timeout: Duration) -> BleResult<()> {
        self.socket
            .set_read_timeout(Some(timeout))
            .map_err(|_| BleError::SocketError)
    }

    /// Write data using std::io::Write
    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        (&self.socket).write_all(buf)
    }

    /// Read data using std::io::Read
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        (&self.socket).read(buf)
    }

    /// Initialize controller for USER channel (reset + set event masks)
    fn init_user_channel(&mut self) -> BleResult<()> {
        // Send HCI Reset
        self.send_cmd_wait(HCI_OP_RESET, &[])?;

        // Set Event Mask - enable LE Meta Event (bit 61)
        // Mask: 0x20_00_00_00_00_00_00_00 for LE Meta only, but we enable common events too
        let event_mask: [u8; 8] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x3F];
        self.send_cmd_wait(HCI_OP_SET_EVENT_MASK, &event_mask)?;

        // Set LE Event Mask - enable advertising report (bit 1)
        let le_event_mask: [u8; 8] = [0x1F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        self.send_cmd_wait(HCI_OP_LE_SET_EVENT_MASK, &le_event_mask)?;

        Ok(())
    }

    /// Send HCI command and wait for command complete
    fn send_cmd_wait(&mut self, opcode: u16, params: &[u8]) -> BleResult<()> {
        // Build command packet
        let mut buf = [0u8; 260];
        buf[0] = HCI_COMMAND_PKT;
        buf[1] = (opcode & 0xFF) as u8;
        buf[2] = (opcode >> 8) as u8;
        buf[3] = params.len() as u8;
        buf[4..4 + params.len()].copy_from_slice(params);

        self.write_all(&buf[..4 + params.len()]).map_err(|_| BleError::SocketError)?;

        // Wait for command complete with short timeout
        self.socket.set_read_timeout(Some(Duration::from_millis(1000)))
            .map_err(|_| BleError::SocketError)?;

        let mut resp = [0u8; 260];
        for _ in 0..10 {
            match self.read(&mut resp) {
                Ok(len) if len >= 7 && resp[0] == HCI_EVENT_PKT && resp[1] == 0x0E => {
                    let resp_opcode = u16::from_le_bytes([resp[4], resp[5]]);
                    if resp_opcode == opcode {
                        let status = resp[6];
                        if status == 0 {
                            return Ok(());
                        } else {
                            eprintln!("  [DEBUG] Command 0x{:04X} failed with status 0x{:02X}", opcode, status);
                            return Err(BleError::SocketError);
                        }
                    }
                }
                Ok(_) => continue, // Not command complete, keep waiting
                Err(_) => break,
            }
        }
        Err(BleError::Timeout)
    }
}

// Socket automatically closes when dropped - no manual cleanup needed!

// =============================================================================
// Global state with safe Mutex
// =============================================================================

struct BleState {
    socket: Option<HciSocket>,
    scanning: bool,
    advertising: bool,
    scan_results: Vec<ScanResult>,
}

impl BleState {
    const fn new() -> Self {
        Self {
            socket: None,
            scanning: false,
            advertising: false,
            scan_results: Vec::new(),
        }
    }
}

static STATE: Mutex<BleState> = Mutex::new(BleState::new());

// =============================================================================
// Public API
// =============================================================================

/// Initialize BLE subsystem
pub fn ble_initialize() -> BleResult<()> {
    let mut state = STATE.lock().map_err(|_| BleError::SocketError)?;

    if state.socket.is_some() {
        return Err(BleError::AlreadyInitialized);
    }

    // Try hci0 first, then hci1 (adapter may re-enumerate after reset)
    state.socket = Some(
        HciSocket::new(0)
            .or_else(|_| HciSocket::new(1))
            .map_err(|_| BleError::NoAdapter)?
    );
    Ok(())
}

/// Deinitialize BLE subsystem
pub fn ble_deinitialize() -> BleResult<()> {
    let mut state = STATE.lock().map_err(|_| BleError::SocketError)?;

    if state.socket.is_none() {
        return Err(BleError::NotInitialized);
    }

    // Stop scanning if active
    if state.scanning {
        if let Some(ref mut socket) = state.socket {
            let _ = send_hci_cmd(socket, HCI_OP_LE_SET_SCAN_ENABLE, &[0x00, 0x00]);
        }
        state.scanning = false;
    }

    state.socket = None; // Socket automatically closes
    Ok(())
}

/// Start BLE scanning
pub fn ble_start_scan(timeout_ms: u32) -> BleResult<()> {
    let mut state = STATE.lock().map_err(|_| BleError::SocketError)?;

    if state.socket.is_none() {
        return Err(BleError::NotInitialized);
    }

    if state.scanning {
        return Ok(()); // Already scanning
    }

    // Clear previous scan results
    state.scan_results.clear();

    // Get mutable reference to socket
    let socket = state.socket.as_mut().unwrap();

    // Set scan parameters: active scan, 10ms interval, 10ms window
    let scan_params = [
        LE_SCAN_ACTIVE,
        0x10, 0x00,         // Interval: 16 * 0.625ms = 10ms
        0x10, 0x00,         // Window: 16 * 0.625ms = 10ms
        LE_PUBLIC_ADDRESS,
        0x00,               // Accept all advertisements
    ];
    send_hci_cmd(socket, HCI_OP_LE_SET_SCAN_PARAM, &scan_params)?;

    std::thread::sleep(Duration::from_millis(10)); // Wait for command to complete

    // Enable scanning
    send_hci_cmd(socket, HCI_OP_LE_SET_SCAN_ENABLE, &[0x01, 0x00])?;

    // Use short socket timeout for non-blocking reads, track elapsed time ourselves
    socket.set_read_timeout(Duration::from_millis(100))?;
    let scan_start = std::time::Instant::now();
    let scan_duration = Duration::from_millis(timeout_ms as u64);

    // Read advertising reports until timeout, collect locally first
    let mut buf = [0u8; 258];
    let mut local_results: Vec<ScanResult> = Vec::new();
    let mut event_count = 0u32;
    loop {
        // Check if scan duration has elapsed
        if scan_start.elapsed() >= scan_duration {
            eprintln!("  [DEBUG] Scan complete after {:?}", scan_start.elapsed());
            break;
        }

        match socket.read(&mut buf) {
            Ok(len) if len < 4 => continue,
            Ok(len) => {
                event_count += 1;
                // Debug: show what we're receiving
                if event_count <= 10 {
                    eprintln!(
                        "  [DEBUG] Event {}: len={}, type=0x{:02X}, evt=0x{:02X}",
                        event_count, len, buf[0], buf[1]
                    );
                    // For command complete (0x0E), show opcode and status
                    if buf[1] == 0x0E && len >= 7 {
                        let opcode = u16::from_le_bytes([buf[4], buf[5]]);
                        let status = buf[6];
                        eprintln!(
                            "         CMD_COMPLETE: opcode=0x{:04X}, status=0x{:02X} ({})",
                            opcode, status,
                            if status == 0 { "success" } else { "FAILED" }
                        );
                    }
                }

                if buf[0] == HCI_EVENT_PKT
                    && buf[1] == HCI_EV_LE_META
                    && buf[3] == HCI_EV_LE_ADVERTISING_REPORT
                {
                    if let Some(result) = parse_advertising_report(&buf[4..len]) {
                        // Check for duplicate in local results
                        if !local_results.iter().any(|r| r.address == result.address) {
                            if local_results.len() < MAX_SCAN_RESULTS {
                                eprintln!("  [DEBUG] Found device: {}", result.address);
                                local_results.push(result);
                            }
                        }
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                eprintln!("  [DEBUG] Timeout after {} events", event_count);
                break;
            }
            Err(e) => {
                eprintln!("  [DEBUG] Read error: {:?}", e.kind());
                continue;
            }
        }
    }

    // Disable scanning
    let _ = send_hci_cmd(socket, HCI_OP_LE_SET_SCAN_ENABLE, &[0x00, 0x00]);

    // Transfer local results to state (socket borrow ended)
    state.scan_results = local_results;

    // Update state (socket borrow ended, can mutate state again)
    state.scanning = false;

    Ok(())
}

/// Send an HCI command
fn send_hci_cmd(socket: &mut HciSocket, opcode: u16, params: &[u8]) -> BleResult<()> {
    let mut buf = [0u8; 256];
    buf[0] = HCI_COMMAND_PKT;
    buf[1] = (opcode & 0xFF) as u8;
    buf[2] = (opcode >> 8) as u8;
    buf[3] = params.len() as u8;
    buf[4..4 + params.len()].copy_from_slice(params);

    let len = 4 + params.len();
    socket.write_all(&buf[..len]).map_err(|_| BleError::SocketError)
}

/// Parse advertising report and return ScanResult if valid
fn parse_advertising_report(data: &[u8]) -> Option<ScanResult> {
    if data.len() < 10 {
        return None;
    }

    let num_reports = data[0];
    if num_reports == 0 {
        return None;
    }

    // Debug: show raw advertising report data
    let event_type = data[1];
    let addr_type = data[2];

    // Debug: print packets that contain name AD type (0x08 or 0x09)
    let has_name_type = data[10..].windows(2).any(|w| w[0] > 1 && (w[1] == 0x08 || w[1] == 0x09));
    if has_name_type {
        eprint!("  [DEBUG] Adv with name ({} bytes): ", data.len());
        for b in data.iter().take(40) {
            eprint!("{:02X} ", b);
        }
        eprintln!();
    }
    let addr_bytes: [u8; 6] = [
        data[8], data[7], data[6], data[5], data[4], data[3],
    ];

    let data_len = data[9] as usize;
    let rssi_offset = 10 + data_len;
    let rssi = if rssi_offset < data.len() {
        data[rssi_offset] as i8
    } else {
        -127
    };

    // Parse advertising data for device name
    let mut name: Option<[u8; 32]> = None;
    let mut name_len = 0;

    if data_len > 0 && data.len() >= 10 + data_len {
        let ad_data = &data[10..10 + data_len];
        let mut i = 0;
        while i + 1 < ad_data.len() {
            let len = ad_data[i] as usize;
            if len == 0 || i + len >= ad_data.len() {
                break;
            }
            let ad_type = ad_data[i + 1];
            if (ad_type == 0x09 || ad_type == 0x08) && len > 1 {
                let name_data = &ad_data[i + 2..i + 1 + len];
                let copy_len = std::cmp::min(name_data.len(), 32);
                let mut name_buf = [0u8; 32];
                name_buf[..copy_len].copy_from_slice(&name_data[..copy_len]);
                name = Some(name_buf);
                name_len = copy_len;
                break;
            }
            i += len + 1;
        }
    }

    Some(ScanResult {
        address: BleAddress::new(addr_bytes),
        address_type: if addr_type == LE_RANDOM_ADDRESS {
            AddressType::Random
        } else {
            AddressType::Public
        },
        rssi,
        name,
        name_len,
    })
}

/// Stop BLE scanning
pub fn ble_stop_scan() -> BleResult<()> {
    let mut state = STATE.lock().map_err(|_| BleError::SocketError)?;

    if state.socket.is_none() {
        return Err(BleError::NotInitialized);
    }

    if !state.scanning {
        return Ok(());
    }

    if let Some(ref mut socket) = state.socket {
        send_hci_cmd(socket, HCI_OP_LE_SET_SCAN_ENABLE, &[0x00, 0x00])?;
    }
    state.scanning = false;
    Ok(())
}

/// Get scan results
pub fn ble_get_scan_results() -> BleResult<Vec<ScanResult>> {
    let state = STATE.lock().map_err(|_| BleError::SocketError)?;

    if state.socket.is_none() {
        return Err(BleError::NotInitialized);
    }

    Ok(state.scan_results.clone())
}

/// Start BLE advertising with the given device name
pub fn ble_start_advertising(name: &str) -> BleResult<()> {
    let mut state = STATE.lock().map_err(|_| BleError::SocketError)?;

    if state.socket.is_none() {
        return Err(BleError::NotInitialized);
    }

    if state.advertising {
        return Ok(()); // Already advertising
    }

    let socket = state.socket.as_mut().unwrap();

    // Generate and set a static random address
    // Static random address: two MSBs of the address must be '11'
    let random_addr: [u8; 6] = [0xC0, 0xDE, 0xCA, 0xFE, 0xBE, 0xEF]; // C0:DE:CA:FE:BE:EF
    socket.send_cmd_wait(HCI_OP_LE_SET_RANDOM_ADDR, &random_addr)?;

    // Set advertising parameters
    // - Interval: 100ms (0x00A0 = 160 * 0.625ms)
    // - Type: ADV_IND (connectable undirected)
    // - Own address type: Random
    // - Channel map: All channels (37, 38, 39)
    let adv_params = [
        0xA0, 0x00, // Min interval: 160 * 0.625ms = 100ms
        0xA0, 0x00, // Max interval: 160 * 0.625ms = 100ms
        0x00,       // Type: ADV_IND (connectable undirected)
        0x01,       // Own address type: Random
        0x00,       // Peer address type: Public (not used for ADV_IND)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Peer address (not used)
        0x07,       // Channel map: all channels (37, 38, 39)
        0x00,       // Filter policy: allow any
    ];
    socket.send_cmd_wait(HCI_OP_LE_SET_ADV_PARAM, &adv_params)?;

    // Build advertising data
    // Format: [length, type, data...]
    let mut adv_data = [0u8; 32];
    let mut pos = 0;

    // Flags: LE General Discoverable, BR/EDR Not Supported
    adv_data[pos] = 0x02; // Length
    adv_data[pos + 1] = 0x01; // Type: Flags
    adv_data[pos + 2] = 0x06; // Flags: LE General Discoverable + BR/EDR Not Supported
    pos += 3;

    // Complete Local Name
    let name_bytes = name.as_bytes();
    let name_len = std::cmp::min(name_bytes.len(), 28 - pos); // Leave room
    adv_data[pos] = (name_len + 1) as u8; // Length (type + name)
    adv_data[pos + 1] = 0x09; // Type: Complete Local Name
    adv_data[pos + 2..pos + 2 + name_len].copy_from_slice(&name_bytes[..name_len]);
    pos += 2 + name_len;

    // Set advertising data (first byte is total length)
    let mut adv_cmd = [0u8; 32];
    adv_cmd[0] = pos as u8; // Length of advertising data
    adv_cmd[1..1 + pos].copy_from_slice(&adv_data[..pos]);
    socket.send_cmd_wait(HCI_OP_LE_SET_ADV_DATA, &adv_cmd)?;

    // Enable advertising
    socket.send_cmd_wait(HCI_OP_LE_SET_ADV_ENABLE, &[0x01])?;

    state.advertising = true;
    eprintln!("  [DEBUG] Advertising started as \"{}\"", name);

    Ok(())
}

/// Stop BLE advertising
pub fn ble_stop_advertising() -> BleResult<()> {
    let mut state = STATE.lock().map_err(|_| BleError::SocketError)?;

    if state.socket.is_none() {
        return Err(BleError::NotInitialized);
    }

    if !state.advertising {
        return Ok(()); // Not advertising
    }

    let socket = state.socket.as_mut().unwrap();

    // Disable advertising
    let _ = socket.send_cmd_wait(HCI_OP_LE_SET_ADV_ENABLE, &[0x00]);

    state.advertising = false;
    eprintln!("  [DEBUG] Advertising stopped");

    Ok(())
}

/// Run a simple GATT server
/// This starts advertising, waits for a connection, and handles ATT requests
pub fn ble_run_gatt_server(name: &str, timeout_ms: u32) -> BleResult<()> {
    let mut state = STATE.lock().map_err(|_| BleError::SocketError)?;

    if state.socket.is_none() {
        return Err(BleError::NotInitialized);
    }

    let socket = state.socket.as_mut().unwrap();

    // Start advertising (reuse existing logic but inline here for socket borrow)
    // Set random address
    let random_addr: [u8; 6] = [0xC0, 0xDE, 0xCA, 0xFE, 0xBE, 0xEF];
    socket.send_cmd_wait(HCI_OP_LE_SET_RANDOM_ADDR, &random_addr)?;

    // Set advertising parameters
    let adv_params = [
        0xA0, 0x00, 0xA0, 0x00, 0x00, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x00,
    ];
    socket.send_cmd_wait(HCI_OP_LE_SET_ADV_PARAM, &adv_params)?;

    // Build and set advertising data
    let mut adv_data = [0u8; 32];
    adv_data[0] = 0x02; adv_data[1] = 0x01; adv_data[2] = 0x06; // Flags
    let name_bytes = name.as_bytes();
    let name_len = std::cmp::min(name_bytes.len(), 20);
    adv_data[3] = (name_len + 1) as u8;
    adv_data[4] = 0x09; // Complete Local Name
    adv_data[5..5 + name_len].copy_from_slice(&name_bytes[..name_len]);
    let total_len = 5 + name_len;
    let mut adv_cmd = [0u8; 32];
    adv_cmd[0] = total_len as u8;
    adv_cmd[1..1 + total_len].copy_from_slice(&adv_data[..total_len]);
    socket.send_cmd_wait(HCI_OP_LE_SET_ADV_DATA, &adv_cmd)?;

    // Enable advertising
    socket.send_cmd_wait(HCI_OP_LE_SET_ADV_ENABLE, &[0x01])?;
    eprintln!("  [GATT] Advertising as '{}', waiting for connection...", name);

    // Simple GATT database (inline)
    // Handle 1: Primary Service (0x2800) = Custom Service UUID
    // Handle 2: Characteristic Declaration (0x2803)
    // Handle 3: Characteristic Value - "Hello from RustCam!"
    // Handle 4: Characteristic Declaration (0x2803)
    // Handle 5: Characteristic Value - writable command buffer
    let mut command_buffer: [u8; 32] = [0; 32];
    let mut command_len: usize = 0;
    let hello_msg = b"Hello from RustCam!";

    // Wait for connection and handle ATT requests
    socket.set_read_timeout(Duration::from_millis(timeout_ms as u64))?;
    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(timeout_ms as u64);

    let mut conn_handle: Option<u16> = None;
    let mut buf = [0u8; 512];

    loop {
        if start.elapsed() >= timeout {
            eprintln!("  [GATT] Timeout waiting for connection/data");
            break;
        }

        match socket.read(&mut buf) {
            Ok(len) if len >= 3 => {
                let pkt_type = buf[0];

                // HCI Event packet
                if pkt_type == HCI_EVENT_PKT {
                    let event_code = buf[1];

                    // LE Meta Event
                    if event_code == HCI_EV_LE_META && len >= 4 {
                        let subevent = buf[3];

                        // Connection Complete
                        if subevent == HCI_EV_LE_CONN_COMPLETE && len >= 7 {
                            let status = buf[4];
                            if status == 0 {
                                conn_handle = Some(u16::from_le_bytes([buf[5], buf[6]]));
                                eprintln!("  [GATT] Connected! Handle: 0x{:04X}", conn_handle.unwrap());
                            }
                        }
                    }
                    // Disconnection Complete
                    else if event_code == HCI_EV_DISCONN_COMPLETE && len >= 5 {
                        eprintln!("  [GATT] Disconnected");
                        conn_handle = None;
                        break;
                    }
                }
                // HCI ACL Data packet (0x02)
                else if pkt_type == 0x02 && conn_handle.is_some() && len >= 9 {
                    // ACL header: handle(2) + length(2) + L2CAP header: length(2) + CID(2)
                    let l2cap_cid = u16::from_le_bytes([buf[7], buf[8]]);

                    // ATT channel
                    if l2cap_cid == L2CAP_CID_ATT && len >= 10 {
                        let att_opcode = buf[9];
                        let handle = conn_handle.unwrap();

                        match att_opcode {
                            ATT_OP_MTU_REQ => {
                                eprintln!("  [GATT] MTU Request");
                                let response = build_att_mtu_response(handle, 23);
                                send_acl_data(socket, &response)?;
                            }
                            ATT_OP_READ_BY_GROUP_REQ => {
                                eprintln!("  [GATT] Read By Group Type Request (Service Discovery)");
                                // Return our custom service at handles 1-5
                                let response = build_read_by_group_response(handle, &buf[10..len]);
                                send_acl_data(socket, &response)?;
                            }
                            ATT_OP_READ_BY_TYPE_REQ => {
                                eprintln!("  [GATT] Read By Type Request (Characteristic Discovery)");
                                let response = build_read_by_type_response(handle, &buf[10..len]);
                                send_acl_data(socket, &response)?;
                            }
                            ATT_OP_FIND_INFO_REQ => {
                                eprintln!("  [GATT] Find Information Request");
                                let response = build_find_info_response(handle, &buf[10..len]);
                                send_acl_data(socket, &response)?;
                            }
                            ATT_OP_READ_REQ => {
                                if len >= 12 {
                                    let attr_handle = u16::from_le_bytes([buf[10], buf[11]]);
                                    eprintln!("  [GATT] Read Request for handle {}", attr_handle);
                                    let response = build_read_response(handle, attr_handle, hello_msg, &command_buffer[..command_len]);
                                    send_acl_data(socket, &response)?;
                                }
                            }
                            ATT_OP_WRITE_REQ | ATT_OP_WRITE_CMD => {
                                if len >= 12 {
                                    let attr_handle = u16::from_le_bytes([buf[10], buf[11]]);
                                    let data_start = 12;
                                    let data_len = len - data_start;
                                    eprintln!("  [GATT] Write to handle {}: {:?}", attr_handle, &buf[data_start..len]);

                                    // Handle 5 is our writable characteristic
                                    if attr_handle == 5 && data_len <= 32 {
                                        command_buffer[..data_len].copy_from_slice(&buf[data_start..len]);
                                        command_len = data_len;
                                        eprintln!("  [GATT] Command received: {:?}",
                                            std::str::from_utf8(&command_buffer[..command_len]).unwrap_or("<binary>"));
                                    }

                                    // Send write response for WRITE_REQ
                                    if att_opcode == ATT_OP_WRITE_REQ {
                                        let response = build_write_response(handle);
                                        send_acl_data(socket, &response)?;
                                    }
                                }
                            }
                            _ => {
                                eprintln!("  [GATT] Unknown ATT opcode: 0x{:02X}", att_opcode);
                            }
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(_) => {
                continue;
            }
        }
    }

    // Stop advertising
    let _ = socket.send_cmd_wait(HCI_OP_LE_SET_ADV_ENABLE, &[0x00]);
    eprintln!("  [GATT] Server stopped");

    Ok(())
}

// Helper functions for building ATT responses
fn build_att_mtu_response(conn_handle: u16, mtu: u16) -> Vec<u8> {
    let mut pkt = vec![
        0x02, // ACL data
        (conn_handle & 0xFF) as u8, ((conn_handle >> 8) & 0x0F) as u8,
        0x07, 0x00, // ACL length
        0x03, 0x00, // L2CAP length
        0x04, 0x00, // ATT CID
        ATT_OP_MTU_RSP,
        (mtu & 0xFF) as u8, (mtu >> 8) as u8,
    ];
    pkt
}

fn build_read_by_group_response(conn_handle: u16, req_data: &[u8]) -> Vec<u8> {
    // Request format: start_handle(2) + end_handle(2) + uuid(2 or 16)
    // Our service is at handles 1-5. If start_handle > 5, return error.
    if req_data.len() >= 2 {
        let start_handle = u16::from_le_bytes([req_data[0], req_data[1]]);
        eprintln!("  [GATT] Service discovery from handle {}", start_handle);

        if start_handle > 5 {
            // No more services - return Attribute Not Found
            return build_error_response(conn_handle, ATT_OP_READ_BY_GROUP_REQ, start_handle, ATT_ERR_ATTR_NOT_FOUND);
        }
    }

    // Return one primary service: handles 1-5, UUID = 0x1234 (custom)
    vec![
        0x02,
        (conn_handle & 0xFF) as u8, ((conn_handle >> 8) & 0x0F) as u8,
        0x0C, 0x00, // ACL length
        0x08, 0x00, // L2CAP length
        0x04, 0x00, // ATT CID
        ATT_OP_READ_BY_GROUP_RSP,
        0x06, // Length of each entry (2+2+2)
        0x01, 0x00, // Start handle: 1
        0x05, 0x00, // End handle: 5
        0x34, 0x12, // Service UUID: 0x1234
    ]
}

fn build_read_by_type_response(conn_handle: u16, req_data: &[u8]) -> Vec<u8> {
    // Request format: start_handle(2) + end_handle(2) + uuid(2 or 16)
    if req_data.len() >= 6 {
        let start_handle = u16::from_le_bytes([req_data[0], req_data[1]]);
        let _end_handle = u16::from_le_bytes([req_data[2], req_data[3]]);
        let uuid = u16::from_le_bytes([req_data[4], req_data[5]]);

        eprintln!("  [GATT] Read By Type from handle {} UUID 0x{:04X}", start_handle, uuid);

        // Characteristic declaration (0x2803)
        if uuid == 0x2803 {
            // Our characteristics:
            // - Handle 2: Read characteristic, value at handle 3, UUID 0x1235
            // - Handle 4: Write characteristic, value at handle 5, UUID 0x1236

            if start_handle <= 2 {
                // Return first characteristic (handle 2)
                // ATT payload: opcode(1) + length(1) + handle(2) + props(1) + value_handle(2) + uuid(2) = 9
                return vec![
                    0x02,
                    (conn_handle & 0xFF) as u8, ((conn_handle >> 8) & 0x0F) as u8,
                    0x0D, 0x00, // ACL length = 13 (L2CAP header 4 + ATT payload 9)
                    0x09, 0x00, // L2CAP length = 9
                    0x04, 0x00, // ATT CID
                    ATT_OP_READ_BY_TYPE_RSP,
                    0x07, // Length of each entry: handle(2) + props(1) + value_handle(2) + uuid(2) = 7
                    0x02, 0x00, // Handle: 2
                    0x02,       // Properties: Read
                    0x03, 0x00, // Value handle: 3
                    0x35, 0x12, // UUID: 0x1235
                ];
            } else if start_handle <= 4 {
                // Return second characteristic (handle 4)
                return vec![
                    0x02,
                    (conn_handle & 0xFF) as u8, ((conn_handle >> 8) & 0x0F) as u8,
                    0x0D, 0x00, // ACL length = 13
                    0x09, 0x00, // L2CAP length = 9
                    0x04, 0x00, // ATT CID
                    ATT_OP_READ_BY_TYPE_RSP,
                    0x07, // Length of each entry
                    0x04, 0x00, // Handle: 4
                    0x0A,       // Properties: Write (0x08) + Write Without Response (0x02) = 0x0A
                    0x05, 0x00, // Value handle: 5
                    0x36, 0x12, // UUID: 0x1236
                ];
            }
            // No more characteristics after handle 4
            return build_error_response(conn_handle, ATT_OP_READ_BY_TYPE_REQ, start_handle, ATT_ERR_ATTR_NOT_FOUND);
        }
    }

    // Attribute not found for unknown UUID or invalid request
    let start_handle = if req_data.len() >= 2 {
        u16::from_le_bytes([req_data[0], req_data[1]])
    } else {
        0x0001
    };
    build_error_response(conn_handle, ATT_OP_READ_BY_TYPE_REQ, start_handle, ATT_ERR_ATTR_NOT_FOUND)
}

fn build_find_info_response(conn_handle: u16, req_data: &[u8]) -> Vec<u8> {
    // Request format: start_handle(2) + end_handle(2)
    if req_data.len() >= 4 {
        let start_handle = u16::from_le_bytes([req_data[0], req_data[1]]);
        let _end_handle = u16::from_le_bytes([req_data[2], req_data[3]]);

        eprintln!("  [GATT] Find Info from handle {}", start_handle);

        // Our attribute handles:
        // 1: Primary Service
        // 2: Characteristic Declaration (read)
        // 3: Characteristic Value (read)
        // 4: Characteristic Declaration (write)
        // 5: Characteristic Value (write)

        if start_handle == 1 {
            return vec![
                0x02,
                (conn_handle & 0xFF) as u8, ((conn_handle >> 8) & 0x0F) as u8,
                0x0A, 0x00, // ACL length
                0x06, 0x00, // L2CAP length
                0x04, 0x00, // ATT CID
                ATT_OP_FIND_INFO_RSP,
                0x01, // Format: 16-bit UUIDs
                0x01, 0x00, // Handle: 1
                0x00, 0x28, // UUID: 0x2800 (Primary Service)
            ];
        } else if start_handle == 2 {
            return vec![
                0x02,
                (conn_handle & 0xFF) as u8, ((conn_handle >> 8) & 0x0F) as u8,
                0x0A, 0x00, // ACL length
                0x06, 0x00, // L2CAP length
                0x04, 0x00, // ATT CID
                ATT_OP_FIND_INFO_RSP,
                0x01, // Format: 16-bit UUIDs
                0x02, 0x00, // Handle: 2
                0x03, 0x28, // UUID: 0x2803 (Characteristic Declaration)
            ];
        } else if start_handle == 3 {
            return vec![
                0x02,
                (conn_handle & 0xFF) as u8, ((conn_handle >> 8) & 0x0F) as u8,
                0x0A, 0x00, // ACL length
                0x06, 0x00, // L2CAP length
                0x04, 0x00, // ATT CID
                ATT_OP_FIND_INFO_RSP,
                0x01, // Format: 16-bit UUIDs
                0x03, 0x00, // Handle: 3
                0x35, 0x12, // UUID: 0x1235 (Characteristic Value)
            ];
        } else if start_handle == 4 {
            return vec![
                0x02,
                (conn_handle & 0xFF) as u8, ((conn_handle >> 8) & 0x0F) as u8,
                0x0A, 0x00, // ACL length
                0x06, 0x00, // L2CAP length
                0x04, 0x00, // ATT CID
                ATT_OP_FIND_INFO_RSP,
                0x01, // Format: 16-bit UUIDs
                0x04, 0x00, // Handle: 4
                0x03, 0x28, // UUID: 0x2803 (Characteristic Declaration)
            ];
        } else if start_handle == 5 {
            return vec![
                0x02,
                (conn_handle & 0xFF) as u8, ((conn_handle >> 8) & 0x0F) as u8,
                0x0A, 0x00, // ACL length
                0x06, 0x00, // L2CAP length
                0x04, 0x00, // ATT CID
                ATT_OP_FIND_INFO_RSP,
                0x01, // Format: 16-bit UUIDs
                0x05, 0x00, // Handle: 5
                0x36, 0x12, // UUID: 0x1236 (Characteristic Value)
            ];
        }
        // Handle not found
        return build_error_response(conn_handle, ATT_OP_FIND_INFO_REQ, start_handle, ATT_ERR_ATTR_NOT_FOUND);
    }

    // Invalid request
    build_error_response(conn_handle, ATT_OP_FIND_INFO_REQ, 0x0001, ATT_ERR_ATTR_NOT_FOUND)
}

fn build_read_response(conn_handle: u16, attr_handle: u16, hello_msg: &[u8], command_buf: &[u8]) -> Vec<u8> {
    let data = match attr_handle {
        3 => hello_msg, // Readable characteristic value
        5 => command_buf, // Writable characteristic value
        _ => b"Unknown",
    };

    let l2cap_len = 1 + data.len();
    let acl_len = l2cap_len + 4;

    let mut pkt = vec![
        0x02,
        (conn_handle & 0xFF) as u8, ((conn_handle >> 8) & 0x0F) as u8,
        (acl_len & 0xFF) as u8, (acl_len >> 8) as u8,
        (l2cap_len & 0xFF) as u8, (l2cap_len >> 8) as u8,
        0x04, 0x00, // ATT CID
        ATT_OP_READ_RSP,
    ];
    pkt.extend_from_slice(data);
    pkt
}

fn build_write_response(conn_handle: u16) -> Vec<u8> {
    vec![
        0x02,
        (conn_handle & 0xFF) as u8, ((conn_handle >> 8) & 0x0F) as u8,
        0x05, 0x00, // ACL length
        0x01, 0x00, // L2CAP length
        0x04, 0x00, // ATT CID
        ATT_OP_WRITE_RSP,
    ]
}

fn build_error_response(conn_handle: u16, req_opcode: u8, handle: u16, error: u8) -> Vec<u8> {
    vec![
        0x02,
        (conn_handle & 0xFF) as u8, ((conn_handle >> 8) & 0x0F) as u8,
        0x09, 0x00, // ACL length
        0x05, 0x00, // L2CAP length
        0x04, 0x00, // ATT CID
        ATT_OP_ERROR_RSP,
        req_opcode,
        (handle & 0xFF) as u8, (handle >> 8) as u8,
        error,
    ]
}

fn send_acl_data(socket: &mut HciSocket, data: &[u8]) -> BleResult<()> {
    socket.write_all(data).map_err(|_| BleError::SocketError)
}

/// Connect to a BLE device
pub fn ble_connect(address: &BleAddress, _timeout_ms: u32) -> BleResult<ConnectionHandle> {
    let state = STATE.lock().map_err(|_| BleError::SocketError)?;

    if state.socket.is_none() {
        return Err(BleError::NotInitialized);
    }

    let _ = address;
    // TODO: Implement full L2CAP connection
    Err(BleError::NotSupported)
}

/// Disconnect from a BLE device
pub fn ble_disconnect(handle: ConnectionHandle) -> BleResult<()> {
    let state = STATE.lock().map_err(|_| BleError::SocketError)?;

    if state.socket.is_none() {
        return Err(BleError::NotInitialized);
    }

    let _ = handle;
    Ok(())
}

/// Discover GATT services
pub fn gatt_discover_services(_handle: ConnectionHandle) -> BleResult<Vec<Uuid>> {
    Err(BleError::NotSupported)
}

/// Read a GATT characteristic
pub fn gatt_read_characteristic(_char: CharacteristicHandle) -> BleResult<Vec<u8>> {
    Err(BleError::NotSupported)
}

/// Write to a GATT characteristic
pub fn gatt_write_characteristic(_char: CharacteristicHandle, _data: &[u8]) -> BleResult<()> {
    Err(BleError::NotSupported)
}
