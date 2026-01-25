//! Simple WiFi HAL test for Linux

fn main() {
    println!("Linux WiFi HAL Test");
    println!("===================\n");

    // Initialize
    println!("Initializing WiFi...");
    match hal::wifi::wifi_initialize() {
        Ok(()) => println!("  OK: WiFi initialized"),
        Err(e) => {
            println!("  FAIL: {:?}", e);
            println!("\nNote: Scanning requires CAP_NET_ADMIN capability.");
            println!("Try: sudo setcap cap_net_admin+ep <binary>");
            return;
        }
    }

    // Get MAC
    print!("Getting MAC address... ");
    match hal::wifi::wifi_get_mac_address() {
        Ok(mac) => println!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        ),
        Err(e) => println!("FAIL: {:?}", e),
    }

    // Start scan
    println!("\nStarting WiFi scan...");
    match hal::wifi::wifi_start_scan() {
        Ok(()) => println!("  Scan triggered"),
        Err(e) => {
            println!("  Scan trigger failed: {:?}", e);
            println!("  Trying to get cached results...");
        }
    }

    // Wait for results
    println!("Waiting for scan results...");
    for i in 0..10 {
        std::thread::sleep(std::time::Duration::from_millis(300));
        match hal::wifi::wifi_scan_is_complete() {
            Ok(true) => {
                println!("  Scan complete after {}ms", (i + 1) * 300);
                break;
            }
            Ok(false) => {
                if i == 9 {
                    println!("  Timeout waiting for scan");
                }
            }
            Err(e) => {
                println!("  Error: {:?}", e);
                break;
            }
        }
    }

    // Get results
    match hal::wifi::wifi_get_scan_results() {
        Ok((results, count)) => {
            println!("\nFound {} networks:", count);
            for i in 0..count {
                let r = &results[i];
                let ssid = r.ssid_str().unwrap_or("<hidden>");
                let bssid = r.bssid_str();
                let bssid_str = std::str::from_utf8(&bssid).unwrap_or("??:??:??:??:??:??");
                println!(
                    "  {:2}. {:32} {} ch{:2} {:3}dBm {:?}",
                    i + 1,
                    ssid,
                    bssid_str,
                    r.channel,
                    r.rssi,
                    r.auth_mode
                );
            }
        }
        Err(e) => println!("\nFailed to get results: {:?}", e),
    }

    println!("\nDone!");
}
