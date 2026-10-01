// Asyncify (`wasm-opt --asyncify`) unwind/rewind fallback for `block_on`.

use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicBool, Ordering};

use wasm_bindgen::prelude::*;

/// Asyncify data struct the pass reads and writes: `{ stack_pos, stack_end }`.
/// Layout defined by Binaryen's `DataOffset` (Asyncify.cpp).
#[repr(C)]
struct AsyncifyData {
    stack_pos: i32,
    stack_end: i32,
}

/// Size of the unwind frame stack.
const STACK_BYTES: usize = 64 * 1024;

/// Unwind frames are written with 4-byte alignment, so the buffer must be too.
#[repr(align(4))]
struct AlignedStack(#[allow(dead_code)] [u8; STACK_BYTES]);

static mut DATA: AsyncifyData = AsyncifyData {
    stack_pos: 0,
    stack_end: 0,
};
static mut STACK: AlignedStack = AlignedStack([0; STACK_BYTES]);
static INIT: AtomicBool = AtomicBool::new(false);

/// Point the asyncify frame stack at the Rust-owned buffer. Idempotent.
fn ensure_init() {
    if !INIT.swap(true, Ordering::SeqCst) {
        let base = addr_of_mut!(STACK) as *mut u8;
        // SAFETY: single-threaded wasm and the one-shot INIT guard; the write
        // happens-before any read through `data_ptr`/the pass, which both run
        // after `ensure_init` on the calling thread.
        unsafe {
            addr_of_mut!(DATA).write(AsyncifyData {
                stack_pos: base as i32,
                stack_end: base as i32 + STACK_BYTES as i32,
            });
        }
    }
}

/// Address of the asyncify data struct.
#[wasm_bindgen]
pub fn data_ptr() -> u32 {
    ensure_init();
    addr_of_mut!(DATA) as *mut u8 as u32
}

/// The suspend point.
#[wasm_bindgen(module = "saikuro")]
extern "C" {
    fn asyncify_suspend();
}

/// Block until `cond` returns true, unwinding to the host on every false check
/// so the JS event loop can run.
pub(crate) fn wait_until(mut cond: impl FnMut() -> bool) {
    ensure_init();
    loop {
        if cond() {
            return;
        }
        asyncify_suspend();
    }
}
