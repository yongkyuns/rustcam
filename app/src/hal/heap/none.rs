//! Stub heap introspection implementation
//!
//! Used when no platform-specific implementation is available.

use super::HeapStats;

/// Get current heap usage in bytes (stub: returns 0)
pub fn get_heap_used() -> i32 {
    0
}

/// Get detailed heap statistics (stub: returns None)
pub fn get_heap_stats() -> Option<HeapStats> {
    None
}
