//! Native (host) runner: runs the full portable suite plus host-only tests.

use std::alloc::{GlobalAlloc, Layout, System};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "../tests/native/mod.rs"]
mod native;

use saikuro_tests::{register_all, run, TestSuite};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn main() -> ExitCode {
    let mut suite = TestSuite::new();
    register_all(&mut suite);
    native::register(&mut suite);

    let failed = run(&mut suite, |line| println!("{line}"));

    println!(
        "heap watermark: peak {} bytes, final live {} bytes",
        PEAK.load(Ordering::Relaxed),
        LIVE.load(Ordering::Relaxed)
    );
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
