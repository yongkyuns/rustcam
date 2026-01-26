//! Rust std Demo - Memory Profiler and Thread Demo
//!
//! This example demonstrates Rust std features and measures their runtime
//! memory usage. Portable across NuttX and native Linux/POSIX systems.
//!
//! Platform abstraction follows the godevice HAL pattern:
//! - Interface defined in hal/mod.rs
//! - Implementations in hal/<module>/<platform>.rs
//! - Selection via Cargo features (platform-linux, platform-nuttx)

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

// Hardware Abstraction Layer (shared crate)
use hal::{get_heap_stats, get_heap_used};
use hal::ble;
use hal::wifi;
use hal::camera;

// ============================================================================
// Common types
// ============================================================================

/// Measurement result for a single allocation
struct Measurement {
    name: &'static str,
    heap_before: i32,
    heap_after: i32,
}

impl Measurement {
    fn allocated(&self) -> i32 {
        self.heap_after - self.heap_before
    }
}

/// Thread instance with stop flag and join handle
struct ThreadInstance {
    id: u32,
    stop_flag: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

// ============================================================================
// Main application logic
// ============================================================================

/// Run the demo - portable entry point
pub fn run() -> i32 {
    let mut measurements: Vec<Measurement> = Vec::with_capacity(8);

    let baseline = get_heap_used();

    // Vec
    let heap_before = get_heap_used();
    let vec_data: Vec<i32> = (1..=100).collect();
    let heap_after = get_heap_used();
    measurements.push(Measurement {
        name: "Vec<i32> (100 items)",
        heap_before,
        heap_after,
    });

    // String
    let heap_before = get_heap_used();
    let string_data = String::from("Hello from Rust std!");
    let heap_after = get_heap_used();
    measurements.push(Measurement {
        name: "String (20 chars)",
        heap_before,
        heap_after,
    });

    // Box
    let heap_before = get_heap_used();
    let box_data: Box<[u8; 256]> = Box::new([0u8; 256]);
    let heap_after = get_heap_used();
    measurements.push(Measurement {
        name: "Box<[u8; 256]>",
        heap_before,
        heap_after,
    });

    // HashMap empty
    let heap_before = get_heap_used();
    let hashmap_empty: HashMap<i32, i32> = HashMap::new();
    let heap_after = get_heap_used();
    measurements.push(Measurement {
        name: "HashMap (empty)",
        heap_before,
        heap_after,
    });

    // HashMap with entries
    let heap_before = get_heap_used();
    let mut hashmap_data: HashMap<i32, i32> = HashMap::new();
    for i in 0..10 {
        hashmap_data.insert(i, i * 10);
    }
    let heap_after = get_heap_used();
    measurements.push(Measurement {
        name: "HashMap (10 i32,i32)",
        heap_before,
        heap_after,
    });

    // Arc
    let heap_before = get_heap_used();
    let arc_data: Arc<[u8; 128]> = Arc::new([0u8; 128]);
    let heap_after = get_heap_used();
    measurements.push(Measurement {
        name: "Arc<[u8; 128]>",
        heap_before,
        heap_after,
    });

    // Arc<AtomicBool>
    let heap_before = get_heap_used();
    let atomic_data = Arc::new(AtomicBool::new(false));
    let heap_after = get_heap_used();
    measurements.push(Measurement {
        name: "Arc<AtomicBool>",
        heap_before,
        heap_after,
    });

    let total_with_all = get_heap_used();

    // Drop all allocations
    drop(vec_data);
    drop(string_data);
    drop(box_data);
    drop(hashmap_empty);
    drop(hashmap_data);
    drop(arc_data);
    drop(atomic_data);
    let after_drop = get_heap_used();

    // Thread measurement
    let before_thread = get_heap_used();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop_flag);

    let handle = thread::spawn(move || {
        while !stop_clone.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(100));
        }
    });

    thread::sleep(Duration::from_millis(50));
    let after_spawn = get_heap_used();

    stop_flag.store(true, Ordering::Relaxed);
    let _ = handle.join();
    let after_join = get_heap_used();

    // Print results
    println!("=== Rust std Memory Profiler ===\n");
    println!("Baseline heap: {} bytes\n", baseline);

    println!("Memory usage by feature:");
    println!("---------------------------------------------");

    let mut total_allocated = 0i32;
    for m in &measurements {
        let alloc = m.allocated();
        total_allocated += alloc;
        println!(
            "  {:22} {:+6} bytes  [heap: {} -> {}]",
            m.name, alloc, m.heap_before, m.heap_after
        );
    }

    println!("---------------------------------------------");
    println!(
        "Total allocated: {} bytes (heap: {} -> {})\n",
        total_allocated, baseline, total_with_all
    );
    println!(
        "After dropping all: {:+} bytes freed (heap: {} -> {})\n",
        total_with_all - after_drop,
        total_with_all,
        after_drop
    );

    println!("Thread memory usage:");
    println!("---------------------------------------------");
    println!(
        "  thread::spawn:         {:+6} bytes  [heap: {} -> {}]",
        after_spawn - before_thread,
        before_thread,
        after_spawn
    );
    println!(
        "  after join:            {:+6} bytes freed  [heap: {} -> {}]",
        after_spawn - after_join,
        after_spawn,
        after_join
    );
    println!("---------------------------------------------\n");

    // Interactive demo
    println!("=== Interactive Demo ===");
    println!("Commands: s=spawn, t=terminate, m=memory, b=ble scan, a=advertise, g=gatt server, w=wifi, c=camera, q=quit\n");

    let mut threads: Vec<ThreadInstance> = Vec::new();
    let mut next_id: u32 = 1;

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        let _ = stdout.flush();

        let mut input = String::new();
        if stdin.lock().read_line(&mut input).is_err() {
            break;
        }

        match input.trim() {
            "s" => {
                let heap_before = get_heap_used();
                let id = next_id;
                next_id += 1;

                let stop_flag = Arc::new(AtomicBool::new(false));
                let stop_flag_clone = Arc::clone(&stop_flag);
                let thread_start = Instant::now();

                let handle = thread::spawn(move || {
                    let mut tick: u64 = 0;
                    while !stop_flag_clone.load(Ordering::Relaxed) {
                        thread::sleep(Duration::from_secs(1));
                        tick += 1;
                        if stop_flag_clone.load(Ordering::Relaxed) {
                            break;
                        }
                        let elapsed = thread_start.elapsed();
                        println!(
                            "[Thread {}] Tick {}: {}.{:03}s",
                            id,
                            tick,
                            elapsed.as_secs(),
                            elapsed.subsec_millis()
                        );
                    }
                });

                thread::sleep(Duration::from_millis(50));
                let heap_after = get_heap_used();

                threads.push(ThreadInstance {
                    id,
                    stop_flag,
                    handle: Some(handle),
                });

                println!(
                    "Spawned thread {} (+{} bytes, total threads: {})",
                    id,
                    heap_after - heap_before,
                    threads.len()
                );
            }

            "t" => {
                if let Some(mut instance) = threads.pop() {
                    let heap_before = get_heap_used();
                    instance.stop_flag.store(true, Ordering::Relaxed);
                    if let Some(handle) = instance.handle.take() {
                        let _ = handle.join();
                    }
                    let heap_after = get_heap_used();
                    println!(
                        "Terminated thread {} (+{} bytes freed, remaining: {})",
                        instance.id,
                        heap_before - heap_after,
                        threads.len()
                    );
                } else {
                    println!("No threads to stop");
                }
            }

            "m" => {
                if let Some(info) = get_heap_stats() {
                    println!("Heap stats:");
                    println!("  Arena (total):  {} bytes", info.arena);
                    println!("  Used:           {} bytes", info.uordblks);
                    println!("  Free:           {} bytes", info.fordblks);
                    println!("  Free chunks:    {}", info.ordblks);
                    println!("  Largest free:   {} bytes", info.mxordblk);
                    println!("  Active threads: {}", threads.len());
                } else {
                    println!("Heap stats not available on this platform");
                    println!("  Active threads: {}", threads.len());
                }
            }

            "b" => {
                println!("Initializing BLE...");
                match ble::ble_initialize() {
                    Ok(()) => println!("  BLE initialized"),
                    Err(ble::BleError::AlreadyInitialized) => println!("  BLE already initialized"),
                    Err(e) => {
                        println!("  BLE init failed: {}", e);
                        println!("  (Try running with sudo for raw socket access)");
                        continue;
                    }
                }

                println!("Scanning for BLE devices (3 seconds)...");
                match ble::ble_start_scan(3000) {
                    Ok(()) => {
                        match ble::ble_get_scan_results() {
                            Ok(results) => {
                                if results.is_empty() {
                                    println!("  No devices found");
                                } else {
                                    println!("  Found {} device(s):", results.len());
                                    for result in &results {
                                        let name = result.name_str().unwrap_or("<unknown>");
                                        println!(
                                            "    {} ({:?}) RSSI: {} dBm  Name: {}",
                                            result.address, result.address_type, result.rssi, name
                                        );
                                    }
                                }
                            }
                            Err(e) => println!("  Failed to get results: {}", e),
                        }
                    }
                    Err(e) => println!("  Scan failed: {}", e),
                }

                let _ = ble::ble_deinitialize();
                println!("  BLE deinitialized");
            }

            "a" => {
                println!("Initializing BLE for advertising...");
                match ble::ble_initialize() {
                    Ok(()) => println!("  BLE initialized"),
                    Err(ble::BleError::AlreadyInitialized) => println!("  BLE already initialized"),
                    Err(e) => {
                        println!("  BLE init failed: {}", e);
                        println!("  (Try running with sudo for raw socket access)");
                        continue;
                    }
                }

                println!("Starting advertising as 'RustCam'...");
                match ble::ble_start_advertising("RustCam") {
                    Ok(()) => {
                        println!("  Advertising started! Your phone should see 'RustCam'");
                        println!("  Press Enter to stop advertising...");
                        let _ = stdout.flush();
                        let mut dummy = String::new();
                        let _ = stdin.lock().read_line(&mut dummy);
                        let _ = ble::ble_stop_advertising();
                        println!("  Advertising stopped");
                    }
                    Err(e) => println!("  Advertising failed: {}", e),
                }

                let _ = ble::ble_deinitialize();
                println!("  BLE deinitialized");
            }

            "g" => {
                println!("Starting GATT server...");
                match ble::ble_initialize() {
                    Ok(()) => println!("  BLE initialized"),
                    Err(ble::BleError::AlreadyInitialized) => println!("  BLE already initialized"),
                    Err(e) => {
                        println!("  BLE init failed: {}", e);
                        continue;
                    }
                }

                println!("  Running GATT server as 'RustCam' (60 seconds timeout)");
                println!("  Connect from your phone using nRF Connect!");
                println!("  Service UUID: 0x1234");
                println!("  - Read characteristic (handle 3): Returns 'Hello from RustCam!'");
                println!("  - Write characteristic (handle 5): Send commands");
                println!();

                match ble::ble_run_gatt_server("RustCam", 60000) {
                    Ok(()) => println!("  GATT server finished"),
                    Err(e) => println!("  GATT server error: {}", e),
                }

                let _ = ble::ble_deinitialize();
                println!("  BLE deinitialized");
            }

            "w" => {
                println!("WiFi Test");
                println!("=========");

                // Initialize WiFi
                println!("Initializing WiFi...");
                match wifi::wifi_initialize() {
                    Ok(()) => println!("  WiFi initialized"),
                    Err(e) => {
                        println!("  WiFi init failed: {:?}", e);
                        continue;
                    }
                }

                // Scan first to find the network
                println!("Scanning for networks...");
                match wifi::wifi_start_scan() {
                    Ok(()) => println!("  Scan started"),
                    Err(e) => {
                        println!("  Scan failed: {:?}", e);
                        continue;
                    }
                }

                // Wait for scan to complete
                for i in 0..20 {
                    thread::sleep(Duration::from_millis(300));
                    match wifi::wifi_scan_is_complete() {
                        Ok(true) => {
                            println!("  Scan complete after {}ms", (i + 1) * 300);
                            break;
                        }
                        Ok(false) => {
                            if i == 19 {
                                println!("  Timeout waiting for scan");
                            }
                        }
                        Err(e) => {
                            println!("  Scan error: {:?}", e);
                            break;
                        }
                    }
                }

                // Get scan results
                match wifi::wifi_get_scan_results() {
                    Ok((results, count)) => {
                        println!("  Found {} networks:", count);
                        for i in 0..count {
                            let r = &results[i];
                            let ssid = r.ssid_str().unwrap_or("<hidden>");
                            println!(
                                "    {:2}. {:32} ch{:2} {:3}dBm",
                                i + 1, ssid, r.channel, r.rssi
                            );
                        }
                    }
                    Err(e) => println!("  Failed to get results: {:?}", e),
                }

                // Connect to eduheim
                println!("\nConnecting to 'eduheim' with WPA2...");
                let config = wifi::StationConfig::new("eduheim", "10220727");
                match wifi::wifi_connect(&config) {
                    Ok(()) => println!("  Connection initiated"),
                    Err(e) => {
                        println!("  Connection failed: {:?}", e);
                        continue;
                    }
                }

                // Wait for connection
                println!("Waiting for connection...");
                for i in 0..30 {
                    thread::sleep(Duration::from_millis(500));
                    match wifi::wifi_get_connection_status() {
                        Ok(wifi::ConnectionStatus::Connected) => {
                            println!("  Connected after {}ms!", (i + 1) * 500);

                            // Get IP info
                            match wifi::wifi_get_ip_info() {
                                Ok(ip) => {
                                    println!("  IP: {}.{}.{}.{}", ip.ip[0], ip.ip[1], ip.ip[2], ip.ip[3]);
                                    println!("  Netmask: {}.{}.{}.{}", ip.netmask[0], ip.netmask[1], ip.netmask[2], ip.netmask[3]);
                                }
                                Err(_) => println!("  (IP info not available yet)"),
                            }
                            break;
                        }
                        Ok(wifi::ConnectionStatus::Connecting) => {
                            if i % 4 == 0 {
                                println!("  Still connecting...");
                            }
                        }
                        Ok(wifi::ConnectionStatus::Disconnected) => {
                            if i >= 10 {
                                println!("  Connection not established after 5s");
                                // Check ESSID to see if it was set
                                if let Ok((essid, len)) = wifi::wifi_get_essid() {
                                    if len > 0 {
                                        if let Ok(s) = core::str::from_utf8(&essid[..len]) {
                                            println!("  ESSID set to: {}", s);
                                        }
                                    }
                                }
                                break;
                            }
                        }
                        Ok(wifi::ConnectionStatus::Failed) => {
                            println!("  Connection failed");
                            break;
                        }
                        Err(e) => {
                            println!("  Status error: {:?}", e);
                            break;
                        }
                    }
                }

                println!("WiFi test done\n");
            }

            "c" => {
                println!("Camera Test");
                println!("===========");

                // Initialize camera with VGA JPEG
                println!("Initializing camera (VGA JPEG)...");
                let config = camera::CameraConfig::new(
                    camera::PixelFormat::Jpeg,
                    camera::Resolution::Vga,
                );

                match camera::camera_initialize(config) {
                    Ok(()) => println!("  Camera initialized"),
                    Err(e) => {
                        println!("  Camera init failed: {}", e);
                        continue;
                    }
                }

                // Capture a few frames
                println!("Capturing 3 frames...");
                for i in 1..=3 {
                    match camera::camera_capture_frame() {
                        Ok(frame) => {
                            println!(
                                "  Frame {}: {}x{} {:?}, {} bytes",
                                i, frame.width, frame.height, frame.format, frame.len()
                            );
                        }
                        Err(e) => {
                            println!("  Frame {} capture failed: {}", i, e);
                        }
                    }
                    thread::sleep(Duration::from_millis(100));
                }

                // Get settings
                println!("Camera settings:");
                match camera::camera_get_settings() {
                    Ok(settings) => {
                        println!("  Brightness: {}", settings.brightness);
                        println!("  Contrast: {}", settings.contrast);
                        println!("  Saturation: {}", settings.saturation);
                    }
                    Err(e) => println!("  Failed to get settings: {}", e),
                }

                // Cleanup
                match camera::camera_deinitialize() {
                    Ok(()) => println!("  Camera deinitialized"),
                    Err(e) => println!("  Deinit failed: {}", e),
                }

                println!("Camera test done\n");
            }

            "q" => {
                for instance in &threads {
                    instance.stop_flag.store(true, Ordering::Relaxed);
                }
                for mut instance in threads {
                    if let Some(handle) = instance.handle.take() {
                        let _ = handle.join();
                    }
                }
                break;
            }

            "" => {}
            _ => println!("Unknown command. Use 's', 't', 'm', 'b', 'a', 'g', 'w', 'c', or 'q'"),
        }
    }

    println!("Goodbye!");
    0
}

// ============================================================================
// Platform-specific entry points
// ============================================================================

// FFI debug helper
#[cfg(feature = "platform-nuttx")]
extern "C" {
    fn rust_debug_print(msg: *const u8);
}

/// NuttX entry point (called from C wrapper)
#[cfg(feature = "platform-nuttx")]
#[no_mangle]
pub extern "C" fn rust_rustcam_main(_argc: i32, _argv: *const *const u8) -> i32 {
    unsafe {
        rust_debug_print(b"rust_rustcam_main entered\0".as_ptr());
    }

    // Run camera test
    let cam_result = camera_test_nuttx();

    // Then run WiFi test
    let wifi_result = wifi_test_nuttx();

    if cam_result != 0 { cam_result } else { wifi_result }
}

/// Simple WiFi test for NuttX (no interactive input needed)
#[cfg(feature = "platform-nuttx")]
fn wifi_test_nuttx() -> i32 {
    unsafe { rust_debug_print(b"Starting WiFi test...\0".as_ptr()); }

    // Initialize WiFi
    unsafe { rust_debug_print(b"Initializing WiFi...\0".as_ptr()); }
    match wifi::wifi_initialize() {
        Ok(()) => unsafe { rust_debug_print(b"  WiFi initialized OK\0".as_ptr()); },
        Err(e) => {
            unsafe { rust_debug_print(b"  WiFi init FAILED\0".as_ptr()); }
            return 1;
        }
    }

    // Start scan
    unsafe { rust_debug_print(b"Starting WiFi scan...\0".as_ptr()); }
    match wifi::wifi_start_scan() {
        Ok(()) => unsafe { rust_debug_print(b"  Scan started\0".as_ptr()); },
        Err(e) => {
            unsafe { rust_debug_print(b"  Scan FAILED\0".as_ptr()); }
            return 1;
        }
    }

    // Wait for scan to complete
    unsafe { rust_debug_print(b"Waiting for scan...\0".as_ptr()); }
    for i in 0..20 {
        thread::sleep(Duration::from_millis(300));
        match wifi::wifi_scan_is_complete() {
            Ok(true) => {
                unsafe { rust_debug_print(b"  Scan complete!\0".as_ptr()); }
                break;
            }
            Ok(false) => {
                if i == 19 {
                    unsafe { rust_debug_print(b"  Scan timeout\0".as_ptr()); }
                }
            }
            Err(_) => {
                unsafe { rust_debug_print(b"  Scan error\0".as_ptr()); }
                break;
            }
        }
    }

    // Get scan results
    unsafe { rust_debug_print(b"Getting scan results...\0".as_ptr()); }
    match wifi::wifi_get_scan_results() {
        Ok((results, count)) => {
            unsafe {
                extern "C" {
                    fn printf(format: *const u8, ...) -> i32;
                }
                printf(b"Found %d networks:\n\0".as_ptr(), count as i32);
                for i in 0..count {
                    let r = &results[i];
                    if r.ssid_len > 0 {
                        // Print SSID manually
                        printf(b"  %d. \0".as_ptr(), (i + 1) as i32);
                        for j in 0..r.ssid_len {
                            printf(b"%c\0".as_ptr(), r.ssid[j] as i32);
                        }
                        printf(b" ch%d %ddBm\n\0".as_ptr(), r.channel as i32, r.rssi as i32);
                    }
                }
            }
        }
        Err(_) => {
            unsafe { rust_debug_print(b"  Get results FAILED\0".as_ptr()); }
        }
    }

    // Connect to eduheim
    unsafe { rust_debug_print(b"\nConnecting to eduheim...\0".as_ptr()); }
    let config = wifi::StationConfig::new("eduheim", "10220727");
    match wifi::wifi_connect(&config) {
        Ok(()) => unsafe { rust_debug_print(b"  Connection initiated\0".as_ptr()); },
        Err(_) => {
            unsafe { rust_debug_print(b"  Connection FAILED\0".as_ptr()); }
            return 1;
        }
    }

    // Wait for connection
    unsafe { rust_debug_print(b"Waiting for connection...\0".as_ptr()); }
    for i in 0..30 {
        thread::sleep(Duration::from_millis(500));
        match wifi::wifi_get_connection_status() {
            Ok(wifi::ConnectionStatus::Connected) => {
                unsafe { rust_debug_print(b"  CONNECTED!\0".as_ptr()); }

                // Get IP
                if let Ok(ip) = wifi::wifi_get_ip_info() {
                    unsafe {
                        extern "C" {
                            fn printf(format: *const u8, ...) -> i32;
                        }
                        printf(b"  IP: %d.%d.%d.%d\n\0".as_ptr(),
                            ip.ip[0] as i32, ip.ip[1] as i32, ip.ip[2] as i32, ip.ip[3] as i32);
                    }
                }
                break;
            }
            Ok(wifi::ConnectionStatus::Connecting) => {
                if i % 4 == 0 {
                    unsafe { rust_debug_print(b"  Still connecting...\0".as_ptr()); }
                }
            }
            Ok(wifi::ConnectionStatus::Disconnected) => {
                if i >= 10 {
                    unsafe { rust_debug_print(b"  Not connected after 5s\0".as_ptr()); }
                    break;
                }
            }
            Ok(wifi::ConnectionStatus::Failed) => {
                unsafe { rust_debug_print(b"  Connection failed\0".as_ptr()); }
                break;
            }
            Err(_) => {
                unsafe { rust_debug_print(b"  Status error\0".as_ptr()); }
                break;
            }
        }
    }

    unsafe { rust_debug_print(b"\nWiFi test done\0".as_ptr()); }
    0
}

/// Camera test for NuttX
#[cfg(feature = "platform-nuttx")]
fn camera_test_nuttx() -> i32 {
    unsafe { rust_debug_print(b"Starting Camera test...\0".as_ptr()); }

    // Initialize camera with VGA JPEG
    unsafe { rust_debug_print(b"Initializing camera (VGA JPEG)...\0".as_ptr()); }
    let config = camera::CameraConfig::new(
        camera::PixelFormat::Jpeg,
        camera::Resolution::Vga,
    );

    match camera::camera_initialize(config) {
        Ok(()) => unsafe { rust_debug_print(b"  Camera initialized OK\0".as_ptr()); },
        Err(_) => {
            unsafe { rust_debug_print(b"  Camera init FAILED\0".as_ptr()); }
            return 1;
        }
    }

    // Capture a few frames
    unsafe { rust_debug_print(b"Capturing 3 frames...\0".as_ptr()); }
    for i in 1..=3 {
        match camera::camera_capture_frame() {
            Ok(frame) => {
                unsafe {
                    extern "C" {
                        fn printf(format: *const u8, ...) -> i32;
                    }
                    printf(
                        b"  Frame %d: %dx%d, %d bytes\n\0".as_ptr(),
                        i,
                        frame.width as i32,
                        frame.height as i32,
                        frame.len() as i32,
                    );
                }
            }
            Err(_) => {
                unsafe {
                    extern "C" {
                        fn printf(format: *const u8, ...) -> i32;
                    }
                    printf(b"  Frame %d capture FAILED\n\0".as_ptr(), i);
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    // Cleanup
    unsafe { rust_debug_print(b"Deinitializing camera...\0".as_ptr()); }
    match camera::camera_deinitialize() {
        Ok(()) => unsafe { rust_debug_print(b"  Camera deinitialized OK\0".as_ptr()); },
        Err(_) => unsafe { rust_debug_print(b"  Deinit FAILED\0".as_ptr()); },
    }

    unsafe { rust_debug_print(b"\nCamera test done\0".as_ptr()); }
    0
}
