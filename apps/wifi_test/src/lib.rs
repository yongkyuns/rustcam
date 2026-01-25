//! WiFi Test Application
//!
//! Tests WiFi scanning and connection on NuttX ESP32S3.

use std::thread;
use std::time::Duration;

use hal::wifi;

/// Test WiFi scanning
fn test_scan() -> bool {
    println!("=== WiFi Scan Test ===");

    // Start scan
    println!("Starting WiFi scan...");
    if let Err(e) = wifi::wifi_start_scan() {
        println!("Failed to start scan: {:?}", e);
        return false;
    }

    // Wait for scan to complete (poll every 500ms, max 10 seconds)
    println!("Waiting for scan results...");
    for i in 0..20 {
        thread::sleep(Duration::from_millis(500));

        match wifi::wifi_scan_is_complete() {
            Ok(true) => {
                println!("Scan complete after {}ms", (i + 1) * 500);
                break;
            }
            Ok(false) => {
                // Still scanning
                continue;
            }
            Err(e) => {
                println!("Scan status error: {:?}", e);
                return false;
            }
        }
    }

    // Get scan results
    match wifi::wifi_get_scan_results() {
        Ok((results, count)) => {
            println!("Found {} networks:", count);

            for i in 0..count {
                let result = &results[i];
                let ssid = result.ssid_str().unwrap_or("<invalid>");
                let bssid = result.bssid_str();
                let bssid_str = std::str::from_utf8(&bssid).unwrap_or("??:??:??:??:??:??");
                println!(
                    "  {}. {} ({}) ch{} {}dBm {:?}",
                    i + 1,
                    ssid,
                    bssid_str,
                    result.channel,
                    result.rssi,
                    result.auth_mode
                );
            }
            true
        }
        Err(e) => {
            println!("Failed to get scan results: {:?}", e);
            false
        }
    }
}

/// Test WiFi connection (requires SSID and password to be set)
fn test_connect(ssid: &str, password: &str) -> bool {
    println!("=== WiFi Connect Test ===");
    println!("Connecting to '{}'...", ssid);

    let config = wifi::StationConfig::new(ssid, password);

    if let Err(e) = wifi::wifi_connect(&config) {
        println!("Connection failed: {:?}", e);
        return false;
    }

    println!("Connection initiated, waiting for association...");

    // Wait for connection (poll every 500ms, max 30 seconds)
    for i in 0..60 {
        thread::sleep(Duration::from_millis(500));

        match wifi::wifi_get_connection_status() {
            Ok(wifi::ConnectionStatus::Connected) => {
                println!("Connected after {}ms!", (i + 1) * 500);

                // Get ESSID to confirm
                if let Ok((essid, len)) = wifi::wifi_get_essid() {
                    if let Ok(name) = std::str::from_utf8(&essid[..len]) {
                        println!("Associated with: {}", name);
                    }
                }

                // Get IP info
                if let Ok(ip) = wifi::wifi_get_ip_info() {
                    println!("IP Address: {}", ip);
                }

                return true;
            }
            Ok(wifi::ConnectionStatus::Failed) => {
                println!("Connection failed!");
                return false;
            }
            Ok(wifi::ConnectionStatus::Connecting) | Ok(wifi::ConnectionStatus::Disconnected) => {
                // Still connecting
                continue;
            }
            Err(e) => {
                println!("Status error: {:?}", e);
            }
        }
    }

    println!("Connection timeout!");
    false
}

/// Run the WiFi test
fn run_wifi_test() -> i32 {
    println!("WiFi Test Application");
    println!("=====================");

    // Initialize WiFi
    println!("Initializing WiFi...");
    match wifi::wifi_initialize() {
        Ok(()) => println!("WiFi initialized successfully"),
        Err(e) => {
            println!("WiFi init failed: {:?}", e);
            return 1;
        }
    }

    // Get MAC address
    match wifi::wifi_get_mac_address() {
        Ok(mac) => {
            println!(
                "MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
            );
        }
        Err(e) => {
            println!("Failed to get MAC: {:?}", e);
        }
    }

    // Set station mode
    println!("Setting station mode...");
    if let Err(e) = wifi::wifi_set_mode(wifi::WifiMode::Station) {
        println!("Failed to set mode: {:?}", e);
    }

    // Test scanning
    let scan_ok = test_scan();
    if !scan_ok {
        println!("Scan test FAILED");
    }

    // Test connection (use your own SSID/password)
    // Uncomment and modify to test:
    // let connect_ok = test_connect("YourSSID", "YourPassword");
    // if !connect_ok {
    //     println!("Connect test FAILED");
    // }
    let _ = test_connect; // Suppress unused warning

    println!();
    println!("WiFi test complete!");

    0
}

/// Main entry point for NuttX
#[no_mangle]
pub extern "C" fn wifi_test_main(_argc: i32, _argv: *const *const u8) -> i32 {
    run_wifi_test()
}
