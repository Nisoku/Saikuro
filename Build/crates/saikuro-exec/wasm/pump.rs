//! Event-driven executor pump for the wasm engine.
//!
//! embassy's `raw::Executor` parks when idle; its `__pender` export fires on
//! each empty to non-empty run-queue transition. We can't `poll()` from there
//! (reentrancy), so it enqueues a microtask that polls once. Concurrent wakes
//! coalesce on `PUMP_SCHEDULED` so spawned work keeps progressing.

use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsError, JsValue};

use super::jspi::static_executor;
use super::time;

/// True while a pump microtask is already queued.
static PUMP_SCHEDULED: AtomicBool = AtomicBool::new(false);

/// Address of the leaked tick closure (`*const Closure<...>`), written once.
static PUMP_TICK: AtomicUsize = AtomicUsize::new(0);

/// Number of inboxed executor polls on the current wasm stack.
static POLL_DEPTH: AtomicU32 = AtomicU32::new(0);

/// Marker for one active executor poll.
pub(crate) struct PollGuard;

impl Drop for PollGuard {
    fn drop(&mut self) {
        POLL_DEPTH.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Claim the driver role for the duration of one executor `poll`.
pub(crate) fn entering_poll() -> PollGuard {
    POLL_DEPTH.fetch_add(1, Ordering::SeqCst);
    PollGuard
}

/// True when an executor poll is already active on this stack.
pub(crate) fn is_driving() -> bool {
    POLL_DEPTH.load(Ordering::SeqCst) != 0
}

/// Single executor tick on the shared runtime and clock.
fn run_tick() {
    PUMP_SCHEDULED.store(false, Ordering::SeqCst);
    let _guard = entering_poll();
    time::advance_clock();
    unsafe { static_executor().poll() };
}

/// Pender linking hook, resolved by `embassy_executor::raw::Executor`.
///
/// Called from the waker path whenever the run queue gains work. Never poll
/// here: the executor forbids reentrant `poll()`. Enqueue a microtask instead.
#[unsafe(export_name = "__pender")]
fn __pender(_context: *mut ()) {
    schedule();
}

/// Request one executor poll on a future microtask tick; coalescing
pub(crate) fn schedule() {
    if PUMP_SCHEDULED.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = js_sys::Promise::resolve(&JsValue::UNDEFINED).then_map(tick());
}

/// The tick microtask, leaked once for the module lifetime.
///
/// `Closure` is not `Sync`, so a direct `static` is unavailable; the leaked
/// pointer is stored instead.
fn tick() -> &'static Closure<dyn FnMut(JsValue) -> Result<(), JsError>> {
    let addr = PUMP_TICK.load(Ordering::SeqCst);
    if addr == 0 {
        let closure = Closure::new(|_: JsValue| -> Result<(), JsError> {
            run_tick();
            Ok(())
        });
        let leaked: &'static Closure<dyn FnMut(JsValue) -> Result<(), JsError>> =
            Box::leak(Box::new(closure));
        PUMP_TICK.store(leaked as *const _ as usize, Ordering::SeqCst);
        leaked
    } else {
        // SAFETY: `Box::leak` in the branch above runs once (guarded by the
        // `addr == 0` check) and the allocation is never freed, so the
        // pointer stays valid for `'static`. Written before it is ever read.
        unsafe { &*(addr as *const Closure<dyn FnMut(JsValue) -> Result<(), JsError>>) }
    }
}