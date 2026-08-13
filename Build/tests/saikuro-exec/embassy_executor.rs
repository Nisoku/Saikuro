#![cfg(feature = "embassy-test")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use embassy_executor::raw;
use embassy_executor::Spawner;

use saikuro_exec::{mpsc, oneshot, sleep, timeout, watch, ChannelCapacity};

const TEST_TIMEOUT: Duration = Duration::from_secs(30);

type Done = Arc<AtomicBool>;

/// Wake hook for the raw executor.
///
/// The executor is driven by a busy-poll loop in `run_until`, so the pender
/// does not need to wake anything; woken tasks are enqueued by `wake_task`
/// regardless. On host this is the equivalent of an interrupt pender.
#[no_mangle]
fn __pender(_context: *mut ()) {}

#[embassy_executor::task]
async fn producer(tx: mpsc::Sender<u64>, done_tx: oneshot::Sender<()>) {
    for i in 0..5 {
        tx.send(i).await.expect("consumer dropped the channel");
    }
    let _ = done_tx.send(());
}

#[embassy_executor::task]
async fn consumer(mut rx: mpsc::Receiver<u64>, done_rx: oneshot::Receiver<()>, done: Done) {
    let mut got = Vec::new();
    while let Some(value) = rx.recv().await {
        got.push(value);
    }
    assert_eq!(got, vec![0, 1, 2, 3, 4]);
    assert!(done_rx.await.is_ok(), "producer did not signal completion");
    done.store(true, Ordering::SeqCst);
}

#[embassy_executor::task]
async fn timer_task(done: Done) {
    let start = Instant::now();
    sleep(Duration::from_millis(50)).await;
    assert!(
        start.elapsed() >= Duration::from_millis(40),
        "timer woke too early: {:?}",
        start.elapsed()
    );

    let expired = timeout(
        Duration::from_millis(10),
        sleep(Duration::from_millis(1000)),
    )
    .await;
    assert!(
        expired.is_err(),
        "timeout should have fired before the sleep"
    );

    done.store(true, Ordering::SeqCst);
}

#[embassy_executor::task]
async fn watch_writer(tx: watch::Sender<u64>) {
    tx.send(42).expect("reader was dropped");
}

#[embassy_executor::task]
async fn watch_reader(mut rx: watch::Receiver<u64>, done: Done) {
    rx.changed().await.expect("watch closed before the update");
    assert_eq!(rx.borrow(), 42);
    done.store(true, Ordering::SeqCst);
}

/// Drive a raw executor on the current thread until `done` is set, then return.
/// Task panics propagate out of `poll` and fail the test.
fn run_until(done: &Done, init: impl FnOnce(&Spawner)) {
    let executor: &'static raw::Executor =
        Box::leak(Box::new(raw::Executor::new(core::ptr::null_mut())));
    let spawner = executor.spawner();
    init(&spawner);

    let deadline = Instant::now() + TEST_TIMEOUT;
    while Instant::now() < deadline {
        unsafe { executor.poll() };
        if done.load(Ordering::SeqCst) {
            return;
        }
        thread::sleep(Duration::from_millis(1));
    }
    panic!("executor integration test timed out");
}

#[test]
fn mpsc_and_oneshot_between_spawned_tasks() {
    let done: Done = Arc::new(AtomicBool::new(false));
    let done_clone = done.clone();
    let (tx, rx) = mpsc::channel::<u64>(ChannelCapacity::new(4).expect("valid capacity"));
    let (done_tx, done_rx) = oneshot::channel::<()>();

    run_until(&done, move |spawner| {
        spawner.must_spawn(producer(tx, done_tx));
        spawner.must_spawn(consumer(rx, done_rx, done_clone));
    });
}

#[test]
fn timers_and_timeout_on_app_executor() {
    let done: Done = Arc::new(AtomicBool::new(false));
    let done_clone = done.clone();

    run_until(&done, move |spawner| {
        spawner.must_spawn(timer_task(done_clone));
    });
}

#[test]
fn watch_channel_between_spawned_tasks() {
    let done: Done = Arc::new(AtomicBool::new(false));
    let done_clone = done.clone();
    let (tx, rx) = watch::channel::<u64>(0);

    run_until(&done, move |spawner| {
        spawner.must_spawn(watch_writer(tx));
        spawner.must_spawn(watch_reader(rx, done_clone));
    });
}
