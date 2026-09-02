#![no_std]
#![no_main]

extern crate alloc;

use linked_list_allocator::Heap;
use riscv_rt::entry;
use saikuro_qemu_tests::{host_println, TestRunner};

static mut HEAP_MEM: [u8; 128 * 1024] = [0u8; 128 * 1024];
static mut HEAP: Heap = Heap::empty();

struct GlobalHeap;

unsafe impl core::alloc::GlobalAlloc for GlobalHeap {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        HEAP.allocate_first_fit(layout)
            .map_or(core::ptr::null_mut(), |ptr| ptr.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        if let Some(ptr) = core::ptr::NonNull::new(ptr) {
            HEAP.deallocate(ptr, layout);
        }
    }
}

#[global_allocator]
static ALLOCATOR: GlobalHeap = GlobalHeap;

static mut TICKS: u64 = 0;

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
        HEAP.init(HEAP_MEM.as_mut_ptr(), HEAP_MEM.len());
    }
    host_println("saikuro QEMU tests (riscv32imac)");

    let _ = saikuro_random::init_from(&saikuro_qemu_tests::FixedEntropy);

    let (shared_passed, shared_failed) = saikuro_qemu_tests::run_shared_sync_tests();

    let mut runner = TestRunner::new();

    saikuro_qemu_tests::test_random_try_auto_seed_fails(&mut runner);
    saikuro_qemu_tests::test_random_init_from(&mut runner);
    saikuro_qemu_tests::test_random_uuid_after_seed(&mut runner);

    runner.merge(shared_passed, shared_failed);
    let sync_ok = runner.summary();

    saikuro_qemu_tests::qemu_exit(sync_ok);
}
