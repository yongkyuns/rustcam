//! NuttX heap introspection implementation
//!
//! Uses NuttX's mallinfo() for heap statistics.

use super::HeapStats;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct MallInfo {
    arena: i32,
    ordblks: i32,
    mxordblk: i32,
    uordblks: i32,
    fordblks: i32,
}

extern "C" {
    fn mallinfo() -> MallInfo;
}

/// Get current heap usage in bytes
pub fn get_heap_used() -> i32 {
    // NuttX's fordblks tracks used space
    unsafe { mallinfo().fordblks }
}

/// Get detailed heap statistics
pub fn get_heap_stats() -> Option<HeapStats> {
    let info = unsafe { mallinfo() };
    Some(HeapStats {
        arena: info.arena,
        ordblks: info.ordblks,
        mxordblk: info.mxordblk,
        uordblks: info.uordblks,
        fordblks: info.fordblks,
    })
}
