#![no_std]
#![no_main]

extern crate alloc;

use cortex_m_rt::entry;
use cortex_m_semihosting::debug;
use linked_list_allocator::Heap;
use panic_semihosting as _;

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

#[entry]
fn main() -> ! {
    {
        extern "C" {
            static __sheap: u8;
            static _stack_start: u8;
        }
        let heap_start = unsafe { &__sheap as *const u8 as *mut u8 };
        let heap_start_addr = heap_start as usize;
        let ram_top = unsafe { &_stack_start as *const u8 as usize };
        let stack_size: usize = 0x2000;
        let heap_end = ram_top - stack_size;
        let heap_size = heap_end - heap_start_addr;
        unsafe { HEAP.init(heap_start, heap_size) };
        saikuro_qemu_tests::host_println(&alloc::format!(
            "heap: {heap_start_addr:#010x}..{heap_end:#010x} ({heap_size} bytes)"
        ));
    }
    saikuro_qemu_tests::host_println("saikuro QEMU tests (thumbv6m)");

    let _ = saikuro_random::init_from(&saikuro_qemu_tests::FixedEntropy);

    let (shared_passed, shared_failed) = saikuro_qemu_tests::run_shared_sync_tests();

    let mut runner = saikuro_qemu_tests::TestRunner::new();

    saikuro_qemu_tests::test_random_try_auto_seed_fails(&mut runner);
    saikuro_qemu_tests::test_random_init_from(&mut runner);
    saikuro_qemu_tests::test_random_uuid_after_seed(&mut runner);

    runner.merge(shared_passed, shared_failed);
    let sync_ok = runner.summary();

    if sync_ok {
        debug::exit(debug::EXIT_SUCCESS);
    } else {
        debug::exit(debug::EXIT_FAILURE);
    }
    loop {}
}
