//! Linux heap introspection implementation
//!
//! Uses glibc's mallinfo() for heap statistics.

use super::HeapStats;

/// Get current heap usage in bytes
pub fn get_heap_used() -> i32 {
    let info = unsafe { libc::mallinfo() };
    info.uordblks
}

/// Get detailed heap statistics
pub fn get_heap_stats() -> Option<HeapStats> {
    let info = unsafe { libc::mallinfo() };
    Some(HeapStats {
        arena: info.arena,
        ordblks: info.ordblks,
        mxordblk: 0, // Not available in Linux mallinfo
        uordblks: info.uordblks,
        fordblks: info.fordblks,
    })
}
