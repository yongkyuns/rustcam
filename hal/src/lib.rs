//! Hardware Abstraction Layer
//!
//! Platform-specific implementations are selected at compile time via Cargo features.
//! Apps select which HAL modules they need; platform is set by the build command.
//!
//! # Example
//! ```toml
//! # App's Cargo.toml
//! [dependencies]
//! hal = { path = "../../hal", default-features = false, features = ["heap"] }
//! ```

// HAL modules - conditionally compiled based on features
#[cfg(feature = "heap")]
pub mod heap;

#[cfg(feature = "heap")]
pub use heap::{get_heap_stats, get_heap_used};
