//! Process-global live/peak heap accounting.
//!
//! Single source of truth for memory observability across engines. The global
//! allocator feeds [`add`] / [`sub`] on every allocation; tests and diagnostics
//! pull a snapshot through [`live`] / [`peak`]. The counters use a critical
//! section so the accounting is safe to run from inside the allocator (an ISR
//! allocating while the main context counts cannot lose or corrupt updates).

use core::cell::Cell;

use embassy_sync::blocking_mutex::CriticalSectionMutex;

static LIVE: CriticalSectionMutex<Cell<usize>> = CriticalSectionMutex::new(Cell::new(0));
static PEAK: CriticalSectionMutex<Cell<usize>> = CriticalSectionMutex::new(Cell::new(0));

/// Account a freshly reserved live allocation of `bytes` bytes.
pub fn add(bytes: usize) {
    let new_live = LIVE.lock(|cell| {
        let next = cell.get() + bytes;
        cell.set(next);
        next
    });
    let _ = PEAK.lock(|cell| {
        if new_live > cell.get() {
            cell.set(new_live);
        }
    });
}

/// Account a freed allocation of `bytes` bytes.
pub fn sub(bytes: usize) {
    let _ = LIVE.lock(|cell| cell.set(cell.get().saturating_sub(bytes)));
}

/// Live heap bytes at this instant.
pub fn live() -> usize {
    LIVE.lock(|cell| cell.get())
}

/// Highest live-heap watermark observed so far.
pub fn peak() -> usize {
    PEAK.lock(|cell| cell.get())
}
