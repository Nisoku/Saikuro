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

static mut HEAP_MEM: [u8; 128 * 1024] = [0u8; 128 * 1024];

#[entry]
fn main() -> ! {
    // SAFETY: single-threaded at startup, no heap allocations before init.
    let heap = unsafe { core::ptr::addr_of_mut!(HEAP_MEM) }.cast();
    embedded_runner::init_heap(heap, 128 * 1024);

    let failed = embedded_runner::run_qemu_tests("saikuro QEMU tests (riscv32imc)");

    embedded_runner::exit_code(failed == 0);
}
