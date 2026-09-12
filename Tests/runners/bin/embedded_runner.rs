//! Embedded QEMU test runner shared by all five `bin` binaries.

use alloc::format;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::{Cell, RefCell};
use core::fmt::Write as _;
use core::ptr::NonNull;
use core::task::Waker;

use critical_section::Mutex as CsMutex;
use embassy_time_driver::{time_driver_impl, Driver};
use linked_list_allocator::Heap;
use saikuro_event::log::LogLevel;
use saikuro_event::LogRecord;
use saikuro_event::SaikuroError;
use saikuro_random::EntropySource;

use saikuro_tests::{register_all, run, TestSuite};

/// Heap backing every allocation. Protected by a critical section so an
/// interrupt that allocates can never spin against a main-context holder.
pub(crate) static HEAP: CsMutex<RefCell<Heap>> = CsMutex::new(RefCell::new(Heap::empty()));
static HEAP_SIZE: CsMutex<Cell<usize>> = CsMutex::new(Cell::new(0));

/// Heap region size recorded at init.
pub(crate) fn heap_size_allocated() -> usize {
    critical_section::with(|cs| HEAP_SIZE.borrow(cs).get())
}

/// Initialize the heap to the region `[start, start + size)`, which each
/// runner binary carves just below `_stack_start` minus its own per-target
/// stack reserve.
pub(crate) fn init_heap(start: *mut u8, size: usize) {
    critical_section::with(|cs| {
        HEAP_SIZE.borrow(cs).set(size);
        // SAFETY: the region is reserved by the linker and never accessed
        // elsewhere; the critical section keeps initialization atomic with
        // any ISR allocation.
        unsafe {
            let mut heap = HEAP.borrow(cs).borrow_mut();
            heap.init(start, size);
        }
    });
}

pub(crate) struct GlobalHeap;

unsafe impl GlobalAlloc for GlobalHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = critical_section::with(|cs| {
            let mut heap = HEAP.borrow(cs).borrow_mut();
            heap.allocate_first_fit(layout)
                .map_or(core::ptr::null_mut(), |ptr| ptr.as_ptr())
        });
        if ptr.is_null() {
            log_oom(layout.size(), heap_size_allocated());
        } else {
            saikuro_exec::heap_stats::add(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(ptr) = NonNull::new(ptr) {
            critical_section::with(|cs| {
                // SAFETY: `ptr` was returned by `alloc` with the same layout
                // and is not aliased while the allocator region is locked.
                unsafe {
                    let mut heap = HEAP.borrow(cs).borrow_mut();
                    heap.deallocate(ptr, layout);
                }
            });
            saikuro_exec::heap_stats::sub(layout.size());
        }
    }
}

#[global_allocator]
pub(crate) static ALLOCATOR: GlobalHeap = GlobalHeap;

/// Live heap bytes at this instant.
pub(crate) fn live_allocated_bytes() -> usize {
    saikuro_exec::heap_stats::live()
}

/// Highest live-heap watermark observed by the allocator.
pub(crate) fn peak_allocated_bytes() -> usize {
    saikuro_exec::heap_stats::peak()
}

/// Log from inside the allocator on OOM.
fn log_oom(wanted: usize, heap_size: usize) -> ! {
    let live = saikuro_exec::heap_stats::live();
    let peak = saikuro_exec::heap_stats::peak();
    let free = critical_section::with(|cs| HEAP.borrow(cs).borrow().free());
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
