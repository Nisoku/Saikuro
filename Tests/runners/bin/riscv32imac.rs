#![no_std]
#![no_main]

extern crate alloc;

use riscv_rt::entry;

// The `riscv` crate provides the critical-section impl via `critical_section::set_impl!`.
// Nothing in this app imports from it, so force linkage so the `_critical_section_1_0_*`
// symbols reach the final image (required by portable-atomic's no-atomics CAS fallback).
#[used]
static KEEP_RISCV_CRITICAL_SECTION_IMPL: fn() = riscv::interrupt::disable;

mod embedded_runner;

#[entry]
fn main() -> ! {
    // The heap is the free span between the end of .bss and the top of the
    // stack region.
    extern "C" {
        static __ebss: u8;
        static _stack_start: u8;
    }
    let heap_start = unsafe { &__ebss as *const u8 as *mut u8 };
    let heap_size = unsafe { &_stack_start as *const u8 as usize }
        .wrapping_sub(embedded_runner::STACK_GUARD)
        - heap_start as usize;
    embedded_runner::init_heap(heap_start, heap_size);

    let failed = embedded_runner::run_qemu_tests("saikuro QEMU tests (riscv32imac)");

    embedded_runner::exit_code(failed == 0);
}
