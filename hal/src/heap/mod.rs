//! Heap introspection HAL
//!
//! Provides platform-specific heap memory statistics.
//! Implementation is selected at compile time based on platform feature.

// Platform-specific implementations
#[cfg(feature = "platform-linux")]
mod linux;
#[cfg(feature = "platform-linux")]
pub use linux::*;

#[cfg(feature = "platform-nuttx")]
mod nuttx;
#[cfg(feature = "platform-nuttx")]
pub use nuttx::*;

#[cfg(not(any(feature = "platform-linux", feature = "platform-nuttx")))]
mod none;
#[cfg(not(any(feature = "platform-linux", feature = "platform-nuttx")))]
pub use none::*;

/// Heap statistics structure
#[derive(Debug, Clone, Copy)]
pub struct HeapStats {
    /// Total heap arena size in bytes
    pub arena: i32,
    /// Number of free chunks
    pub ordblks: i32,
    /// Size of largest free chunk
    pub mxordblk: i32,
    /// Total allocated space in bytes
    pub uordblks: i32,
    /// Total free space in bytes
    pub fordblks: i32,
}
