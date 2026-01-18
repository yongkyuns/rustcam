//! Rustcam entry point
//!
//! For Linux: standard main() function
//! For NuttX: entry point is rustcam_main() in lib.rs (built as staticlib)

fn main() {
    std::process::exit(rustcam::run());
}
