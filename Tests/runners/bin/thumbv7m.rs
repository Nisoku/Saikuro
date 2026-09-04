#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;

use cortex_m_rt::entry;
use cortex_m_semihosting::debug;
use linked_list_allocator::Heap;
use panic_semihosting as _;
use spin::Mutex;

mod embedded_runner;

static HEAP: Mutex<Heap> = Mutex::new(Heap::empty());
static mut TICKS: u64 = 0;

struct GlobalHeap;

unsafe impl GlobalAlloc for GlobalHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        HEAP.lock()
            .allocate_first_fit(layout)
            .map_or(core::ptr::null_mut(), |ptr| ptr.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(ptr) = NonNull::new(ptr) {
            HEAP.lock().deallocate(ptr, layout);
        }
    }
}

#[global_allocator]
static ALLOCATOR: GlobalHeap = GlobalHeap;

#[no_mangle]
unsafe extern "C" fn _embassy_time_now() -> u64 {
    TICKS
}

#[no_mangle]
unsafe extern "C" fn _embassy_time_schedule_wake(_at: u64, _waker: *const ()) {}

#[entry]
fn main() -> ! {
    extern "C" {
        static __sheap: u8;
        static _stack_start: u8;
    }
    let heap_start = unsafe { &__sheap as *const u8 as *mut u8 };
    let heap_size = unsafe { &_stack_start as *const u8 as usize }.wrapping_sub(0x4000)
        - heap_start as usize;
    unsafe { HEAP.lock().init(heap_start, heap_size); }

    let failed = embedded_runner::run_qemu_tests("saikuro QEMU tests (thumbv7m)");

    if failed == 0 {
        debug::exit(debug::EXIT_SUCCESS);
    } else {
        debug::exit(debug::EXIT_FAILURE);
    }
    loop {}
}
