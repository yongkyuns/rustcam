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
    println!("Commands: s=spawn, t=terminate, m=memory, b=ble scan, a=advertise, g=gatt server, q=quit\n");

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
            _ => println!("Unknown command. Use 's', 't', 'm', 'b', 'a', 'g', or 'q'"),
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
    // Debug: print before any std usage
    unsafe {
        rust_debug_print(b"rust_rustcam_main entered\0".as_ptr());
    }

    // Try raw libc write to stdout (fd 1)
    unsafe {
        rust_debug_print(b"Before raw write\0".as_ptr());
        extern "C" {
            fn write(fd: i32, buf: *const u8, count: usize) -> isize;
        }
        let msg = b"Raw write to stdout\n";
        let ret = write(1, msg.as_ptr(), msg.len());
        if ret >= 0 {
            rust_debug_print(b"raw write succeeded\0".as_ptr());
        } else {
            rust_debug_print(b"raw write failed\0".as_ptr());
        }
    }

    // Test pthread key operations directly
    unsafe {
        rust_debug_print(b"Testing pthread_key_create\0".as_ptr());
        extern "C" {
            fn pthread_key_create(key: *mut u32, dtor: extern "C" fn(*mut u8)) -> i32;
            fn pthread_getspecific(key: u32) -> *mut u8;
            fn pthread_setspecific(key: u32, val: *mut u8) -> i32;
        }

        extern "C" fn dummy_dtor(_: *mut u8) {}

        let mut key: u32 = 0;
        let ret = pthread_key_create(&mut key as *mut u32, dummy_dtor);
        if ret == 0 {
            rust_debug_print(b"pthread_key_create succeeded\0".as_ptr());
        } else {
            rust_debug_print(b"pthread_key_create FAILED\0".as_ptr());
        }

        let val = 0x12345678u32 as *mut u8;
        let ret = pthread_setspecific(key, val);
        if ret == 0 {
            rust_debug_print(b"pthread_setspecific succeeded\0".as_ptr());
        } else {
            rust_debug_print(b"pthread_setspecific FAILED\0".as_ptr());
        }

        let got = pthread_getspecific(key);
        if got == val {
            rust_debug_print(b"pthread_getspecific matched\0".as_ptr());
        } else {
            rust_debug_print(b"pthread_getspecific MISMATCH\0".as_ptr());
        }
    }

    // Test getting thread ID
    unsafe {
        rust_debug_print(b"Testing pthread_self\0".as_ptr());
        extern "C" {
            fn pthread_self() -> u32;
        }
        let tid = pthread_self();
        rust_debug_print(b"pthread_self succeeded\0".as_ptr());
    }

    // Try a minimal box allocation
    unsafe {
        rust_debug_print(b"Testing Box allocation\0".as_ptr());
    }
    let boxed: Box<u32> = Box::new(42);
    unsafe {
        rust_debug_print(b"Box allocation succeeded\0".as_ptr());
    }
    drop(boxed);
    unsafe {
        rust_debug_print(b"Box drop succeeded\0".as_ptr());
    }

    // Try Vec
    unsafe {
        rust_debug_print(b"Testing Vec\0".as_ptr());
    }
    let mut v: Vec<u32> = Vec::new();
    v.push(1);
    v.push(2);
    unsafe {
        rust_debug_print(b"Vec succeeded\0".as_ptr());
    }

    // Test BLE advertising directly
    unsafe {
        rust_debug_print(b"Testing BLE advertising\0".as_ptr());
    }

    // Initialize BLE
    match ble::ble_initialize() {
        Ok(()) => unsafe { rust_debug_print(b"BLE init OK\0".as_ptr()) },
        Err(ble::BleError::AlreadyInitialized) => unsafe { rust_debug_print(b"BLE already init\0".as_ptr()) },
        Err(_) => {
            unsafe { rust_debug_print(b"BLE init FAILED\0".as_ptr()) };
            return 1;
        }
    }

    // Wait for NimBLE host to sync with controller
    unsafe {
        rust_debug_print(b"Waiting 5s for host sync...\0".as_ptr());
        extern "C" {
            fn sleep(seconds: u32) -> u32;
        }
        sleep(5);  // Give more time for HCI socket to establish connection
    }

    // Start advertising
    match ble::ble_start_advertising("RustCam") {
        Ok(()) => unsafe { rust_debug_print(b"BLE advertising started!\0".as_ptr()) },
        Err(_) => {
            unsafe { rust_debug_print(b"BLE advertising FAILED\0".as_ptr()) };
            let _ = ble::ble_deinitialize();
            return 1;
        }
    }

    // Wait for advertising to run
    unsafe {
        rust_debug_print(b"Advertising for 15 seconds...\0".as_ptr());
        extern "C" {
            fn sleep(seconds: u32) -> u32;
        }
        sleep(15);
    }

    // Stop advertising
    let _ = ble::ble_stop_advertising();
    unsafe { rust_debug_print(b"Advertising stopped\0".as_ptr()) };

    // Deinitialize
    let _ = ble::ble_deinitialize();
    unsafe { rust_debug_print(b"BLE deinit OK\0".as_ptr()) };

    0
}
