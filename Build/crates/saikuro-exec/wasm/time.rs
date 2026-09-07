// Async sleep and timeout for the wasm engine.

use core::cell::RefCell;
use core::future::{poll_fn, Future};
use core::task::{Poll, Waker};
use core::time::Duration;

use alloc::vec::Vec;
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_time::Instant;

struct SleepEntry {
    deadline: Instant,
    waker: Waker,
}

static SLEEPS: CriticalSectionMutex<RefCell<Vec<SleepEntry>>> =
    CriticalSectionMutex::new(RefCell::new(Vec::new()));

/// Monotonic clock, readable synchronously from wasm on both browser and
/// Node.
pub(crate) fn now() -> Instant {
    Instant::now()
}

/// Wake every sleep whose deadline has passed, then re-arm a JS timer for the
/// next pending deadline. Called from every executor pump site: `pump()`, the
/// `block_on` spin loop, and the JS timer itself.
pub(crate) fn advance_clock() {
    let now = now();
    let mut expired = Vec::new();
    let mut arm_next = None;
    SLEEPS.lock(|r| {
        let mut sleeps = r.borrow_mut();
        sleeps.retain(|entry| {
            if entry.deadline <= now {
                expired.push(entry.waker.clone());
                false
            } else {
                true
            }
        });
        if !expired.is_empty() {
            arm_next = sleeps.iter().map(|e| e.deadline).min();
        }
    });
    if let Some(next) = arm_next {
        schedule_js_advance(next.saturating_duration_since(now));
    }
    for waker in expired {
        waker.wake();
    }
}

/// Sleep for `dur`, suspending until a pump advances the clock past the
/// deadline. Returns immediately for zero-length durations.
pub async fn sleep(dur: Duration) {
    let deadline = now() + dur;
    poll_fn(move |cx| {
        let now = now();
        if now >= deadline {
            return Poll::Ready(());
        }
        // Leader-timer registration: keep exactly one entry per (task,
        // deadline) and arm a fresh JS timer whenever this sleep becomes the
        // earliest pending deadline.
        let mut should_arm = false;
        SLEEPS.lock(|r| {
            let mut sleeps = r.borrow_mut();
            if sleeps
                .iter()
                .any(|e| e.deadline == deadline && e.waker.will_wake(cx.waker()))
            {
                return;
            }
            sleeps.retain(|e| e.deadline != deadline || !e.waker.will_wake(cx.waker()));
            should_arm = match sleeps.iter().map(|e| e.deadline).min() {
                None => true,
                Some(head) => deadline < head,
            };
            sleeps.push(SleepEntry {
                deadline,
                waker: cx.waker().clone(),
            });
        });
        if should_arm {
            schedule_js_advance(deadline.saturating_duration_since(now));
        }
        Poll::Pending
    })
    .await
}

/// Race `fut` against [`sleep`], returning `Err(())` if the deadline passes
/// first. The abandoned `fut` is dropped.
pub async fn timeout<F, T>(dur: Duration, fut: F) -> Result<T, ()>
where
    F: Future<Output = T>,
{
    match embassy_futures::select::select(fut, sleep(dur)).await {
        embassy_futures::select::Either::First(res) => Ok(res),
        embassy_futures::select::Either::Second(_) => Err(()),
    }
}

/// Arm a one-shot JS timer that advances the clock in `after`.
fn schedule_js_advance(after: Duration) {
    let global = js_sys::global();
    let settimeout: js_sys::Function =
        js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
            .expect("globalThis.setTimeout must exist in browser and Node")
            .unchecked_into();
    let callback: JsValue = Closure::once_into_js(advance_clock);
    // Cap at u32 ms; browser setTimeout saturates at 2^31-1 anyway.
    let delay: JsValue = (after.as_millis().min(u32::MAX as u128) as u32).into();
    settimeout
        .call2(&global, &callback, &delay)
        .expect("setTimeout invocation failed");
}
