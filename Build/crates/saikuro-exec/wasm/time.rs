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

use crate::shared::TimeoutError;

struct SleepEntry {
    deadline: Instant,
    waker: Waker,
}

/// Pending sleeps plus the deadline
struct SleepTable {
    sleeps: Vec<SleepEntry>,
    armed: Option<Instant>,
}

static SLEEPS: CriticalSectionMutex<RefCell<SleepTable>> =
    CriticalSectionMutex::new(RefCell::new(SleepTable {
        sleeps: Vec::new(),
        armed: None,
    }));

/// Claim the leader timer for `deadline`.
fn claim_leader(table: &mut SleepTable, deadline: Instant, now: Instant) -> Option<Duration> {
    if let Some(armed) = table.armed {
        if armed <= deadline {
            return None;
        }
    }
    table.armed = Some(deadline);
    Some(deadline.saturating_duration_since(now))
}

/// Monotonic clock, readable synchronously from wasm on both browser and
/// Node.
pub(crate) fn now() -> Instant {
    Instant::now()
}

/// Number of sleeps currently waiting on a clock advance.
#[cfg(not(feature = "asyncify"))]
pub(crate) fn pending_sleeps() -> usize {
    SLEEPS.lock(|r| r.borrow().sleeps.len())
}

/// Wake every sleep whose deadline has passed, then make sure a JS timer is
/// aimed at the earliest deadline still pending.
pub(crate) fn advance_clock() {
    let now = now();
    let mut expired = Vec::new();
    let mut arm_delay = None;
    SLEEPS.lock(|r| {
        let mut table = r.borrow_mut();
        table.sleeps.retain(|entry| {
            if entry.deadline <= now {
                expired.push(entry.waker.clone());
                false
            } else {
                true
            }
        });
        if let Some(next) = table.sleeps.iter().map(|e| e.deadline).min() {
            arm_delay = claim_leader(&mut table, next, now);
        }
    });
    if let Some(delay) = arm_delay {
        schedule_js_advance(delay);
    }
    for waker in expired {
        waker.wake();
    }
}

/// Leader timer callback.
fn on_leader_timer() {
    SLEEPS.lock(|r| r.borrow_mut().armed = None);
    advance_clock();
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
        // Leader-timer registration: keep exactly one entry per (task, deadline)
        // and take over the shared JS timer only when this sleep is the earliest
        // pending deadline.
        let mut arm_delay = None;
        SLEEPS.lock(|r| {
            let mut table = r.borrow_mut();
            if table
                .sleeps
                .iter()
                .any(|e| e.deadline == deadline && e.waker.will_wake(cx.waker()))
            {
                return;
            }
            table
                .sleeps
                .retain(|e| e.deadline != deadline || !e.waker.will_wake(cx.waker()));
            table.sleeps.push(SleepEntry {
                deadline,
                waker: cx.waker().clone(),
            });
            let head = table.sleeps.iter().map(|e| e.deadline).min();
            if head == Some(deadline) {
                arm_delay = claim_leader(&mut table, deadline, now);
            }
        });
        if let Some(delay) = arm_delay {
            schedule_js_advance(delay);
        }
        Poll::Pending
    })
    .await
}

/// Race `fut` against [`sleep`], returning `Err(TimeoutError)` if the deadline
/// passes first. The abandoned `fut` is dropped.
pub async fn timeout<F, T>(dur: Duration, fut: F) -> Result<T, TimeoutError>
where
    F: Future<Output = T>,
{
    match embassy_futures::select::select(fut, sleep(dur)).await {
        embassy_futures::select::Either::First(res) => Ok(res),
        embassy_futures::select::Either::Second(_) => Err(TimeoutError),
    }
}

/// Arm a one-shot JS timer that advances the clock in `after`.
fn schedule_js_advance(after: Duration) {
    let global = js_sys::global();
    let settimeout: js_sys::Function =
        js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
            .expect("globalThis.setTimeout must exist in browser and Node")
            .unchecked_into();
    let callback: JsValue = Closure::once_into_js(on_leader_timer);
    // Cap at u32 ms; browser setTimeout saturates at 2^31-1 anyway.
    let delay: JsValue = (after.as_millis().min(u32::MAX as u128) as u32).into();
    settimeout
        .call2(&global, &callback, &delay)
        .expect("setTimeout invocation failed");
}
