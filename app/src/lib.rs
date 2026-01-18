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

// Hardware Abstraction Layer
mod hal;
use hal::{get_heap_stats, get_heap_used};

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
    println!("=== Interactive Thread Demo ===");
    println!("Commands: s=spawn, t=terminate, m=memory, q=quit\n");

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
            _ => println!("Unknown command. Use 's', 't', 'm', or 'q'"),
        }
    }

    println!("Goodbye!");
    0
}

// ============================================================================
// Platform-specific entry points
// ============================================================================

/// NuttX entry point
#[cfg(feature = "platform-nuttx")]
#[no_mangle]
pub extern "C" fn rustcam_main(_argc: i32, _argv: *const *const u8) -> i32 {
    run()
}
