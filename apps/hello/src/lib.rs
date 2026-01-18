//! Hello World - Minimal Rust std example

use hal::get_heap_used;

pub fn run() -> i32 {
    println!("Hello from Rust!");
    println!("Current heap usage: {} bytes", get_heap_used());
    0
}

#[cfg(feature = "platform-nuttx")]
#[no_mangle]
pub extern "C" fn hello_main(_argc: i32, _argv: *const *const u8) -> i32 {
    run()
}
