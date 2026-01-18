//! Hardware Abstraction Layer
//!
//! Platform-specific implementations are selected at compile time via Cargo features.
//! This follows the same pattern as godevice's HAL architecture.

pub mod heap;

pub use heap::{get_heap_stats, get_heap_used};
