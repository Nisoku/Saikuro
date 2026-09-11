//! Embedded QEMU test runner shared by all five `bin` binaries.

use alloc::format;
use core::alloc::{GlobalAlloc, Layout};
use core::fmt::Write as _;
use core::ptr::NonNull;
use core::task::Waker;

use embassy_time_driver::{time_driver_impl, Driver};
use linked_list_allocator::Heap;
use saikuro_event::log::LogLevel;
use saikuro_event::LogRecord;
use saikuro_event::SaikuroError;
use saikuro_random::EntropySource;
use spin::Mutex;

use saikuro_tests::{register_all, run, TestSuite};

/// Heap backing every allocation, with a live/peak watermark.
static LIVE: Mutex<usize> = Mutex::new(0);
static PEAK: Mutex<usize> = Mutex::new(0);
static HEAP_SIZE: Mutex<usize> = Mutex::new(0);

pub(crate) static HEAP: Mutex<Heap> = Mutex::new(Heap::empty());

/// Reserved bytes between the top of the heap and `_stack_start`.
pub(crate) const STACK_GUARD: usize = 0x5400;

/// Heap region size recorded at init.
pub(crate) fn heap_size_allocated() -> usize {
    *HEAP_SIZE.lock()
}

/// Initialize the heap to the region `[start, start + size)`, which the
/// runner binaries carve just below `_stack_start - STACK_GUARD`.
pub(crate) fn init_heap(start: *mut u8, size: usize) {
    *HEAP_SIZE.lock() = size;
    unsafe {
        HEAP.lock().init(start, size);
    }
}

pub(crate) struct GlobalHeap;

unsafe impl GlobalAlloc for GlobalHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr;
        {
            let mut heap = HEAP.lock();
            ptr = heap
                .allocate_first_fit(layout)
                .map_or(core::ptr::null_mut(), |ptr| ptr.as_ptr());
            if ptr.is_null() {
                // Release the spin guard before `log_oom`: it re-locks `HEAP`
                // (for `free()`) and would deadlock on the spin lock otherwise.
                drop(heap);
                log_oom(layout.size(), *HEAP_SIZE.lock());
            }
        }
        if !ptr.is_null() {
            let mut live = LIVE.lock();
            *live += layout.size();
            let mut peak = PEAK.lock();
            if *live > *peak {
                *peak = *live;
            }
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(ptr) = NonNull::new(ptr) {
            HEAP.lock().deallocate(ptr, layout);
            let mut live = LIVE.lock();
            *live = live.saturating_sub(layout.size());
        }
    }
}

#[global_allocator]
pub(crate) static ALLOCATOR: GlobalHeap = GlobalHeap;

/// Live heap bytes at this instant.
pub(crate) fn live_allocated_bytes() -> usize {
    *LIVE.lock()
}

/// Highest live-heap watermark observed by the allocator.
pub(crate) fn peak_allocated_bytes() -> usize {
    *PEAK.lock()
}

/// Log from inside the allocator on OOM.
fn log_oom(wanted: usize, heap_size: usize) -> ! {
    let live = *LIVE.lock();
    let peak = *PEAK.lock();
    let free = HEAP.lock().free();
    let mut console = Console::new();
    let _ = writeln!(
        console,
        "OOM: wanted {wanted}B, live {live}B, peak {peak}B, heap {heap_size}B, free {free}B"
    );
    exit_code(false)
}

/// Terminate the QEMU guest with a semihosting exit so the host `cargo run`
/// observes a matching status instead of a hanging machine.
pub fn exit_code(success: bool) -> ! {
    #[cfg(target_arch = "riscv32")]
    {
        console::qemu_exit(success);
    }
    #[cfg(target_arch = "arm")]
    {
        let code = if success {
            cortex_m_semihosting::debug::EXIT_SUCCESS
        } else {
            cortex_m_semihosting::debug::EXIT_FAILURE
        };
        cortex_m_semihosting::debug::exit(code);
        loop {}
    }
    #[allow(unreachable_code)]
    loop {}
}

/// Bare-metal panic handler.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let mut console = Console::new();
    let _ = writeln!(console, "{info}");
    exit_code(false)
}

#[path = "../../tests/embedded/mod.rs"]
pub mod embedded;

#[path = "console.rs"]
mod console;
pub use console::Console;

/// Busy-poll embassy-time driver for QEMU.
struct QemuTimeDriver;

impl Driver for QemuTimeDriver {
    fn now(&self) -> u64 {
        console::time_now_us()
    }

    fn schedule_wake(&self, _at: u64, waker: &Waker) {
        waker.wake_by_ref();
    }
}

time_driver_impl!(static QEMU_TIME_DRIVER: QemuTimeDriver = QemuTimeDriver);

/// Deterministic entropy source for the QEMU fleet.
///
/// No guest machine (mps2-an385, mps2-an505, virt) exposes a TRNG.
struct QemuEntropy;

impl EntropySource for QemuEntropy {
    fn try_fill(&self, dest: &mut [u8]) -> Result<(), SaikuroError> {
        for (i, byte) in dest.iter_mut().enumerate() {
            *byte = i.wrapping_mul(31).wrapping_add(17) as u8;
        }
        Ok(())
    }
}

/// Emit `msg` as an info-level log record on the semihosting console.
fn log_line(msg: &str) {
    let record = LogRecord::now(LogLevel::Info, "qemu", msg);
    let mut console = Console::new();
    let _ = writeln!(console, "{record}");
}

/// Run the full embedded suite and return the number of failures.
pub fn run_qemu_tests(banner: &str) -> u32 {
    if let Err(e) = saikuro_random::init_from(&QemuEntropy) {
        log_line(&format!("FATAL: DRBG seeding failed: {e}"));
        return 1;
    }

    let mut suite = TestSuite::new();
    register_all(&mut suite);
    embedded::register(&mut suite);

    log_line(banner);
    log_line(&format!("heap region: {} bytes", heap_size_allocated()));
    let mut prev_peak = peak_allocated_bytes();
    let failed = run(&mut suite, |line| {
        log_line(line);
        if line.starts_with("  PASS ") || line.starts_with("  FAIL ") {
            let live = live_allocated_bytes();
            let tasks = saikuro_exec::active_tasks();
            log_line(&format!("      live {live} bytes, tasks {tasks}"));
            let peak = peak_allocated_bytes();
            if peak > prev_peak {
                log_line(&format!(
                    "      PEAK bumped to {peak} bytes in this test (+{} over prior)",
                    peak - prev_peak
                ));
                prev_peak = peak;
            }
            if line.starts_with("  FAIL ") {
                log_line("stopping: guest failure");
                exit_code(false);
            }
        }
    });
    log_line(&format!(
        "heap: live {} bytes, peak {} bytes",
        live_allocated_bytes(),
        peak_allocated_bytes()
    ));
    failed
}
