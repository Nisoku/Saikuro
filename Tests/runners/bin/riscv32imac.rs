#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;

use linked_list_allocator::Heap;
use riscv_rt::entry;
use spin::Mutex;

mod embedded_runner;

static HEAP: Mutex<Heap> = Mutex::new(Heap::empty());
static mut HEAP_MEM: [u8; 128 * 1024] = [0u8; 128 * 1024];
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

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[entry]
fn main() -> ! {
    unsafe {
        HEAP.lock().init(HEAP_MEM.as_mut_ptr(), HEAP_MEM.len());
    }

    let failed = embedded_runner::run_qemu_tests("saikuro QEMU tests (riscv32imac)");

    embedded_runner::qemu_exit(failed == 0);
}
