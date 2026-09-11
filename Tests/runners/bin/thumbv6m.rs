#![no_std]
#![no_main]

extern crate alloc;

use cortex_m::interrupt;
#[used]
static KEEP_CORTEX_M_CRITICAL_SECTION_IMPL: fn() = interrupt::disable;

use cortex_m_rt::entry;

mod embedded_runner;

#[entry]
fn main() -> ! {
    extern "C" {
        static __sheap: u8;
        static _stack_start: u8;
    }
    let heap_start = unsafe { &__sheap as *const u8 as *mut u8 };
    let heap_size = unsafe { &_stack_start as *const u8 as usize }
        .wrapping_sub(embedded_runner::STACK_GUARD)
        - heap_start as usize;
    embedded_runner::init_heap(heap_start, heap_size);

    let failed = embedded_runner::run_qemu_tests("saikuro QEMU tests (thumbv6m)");

    embedded_runner::exit_code(failed == 0);
}
