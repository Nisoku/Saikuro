//! Concurrency and time tests for saikuro-exec.
//!
//! Tests spawn, JoinHandle, sleep, timeout, yield_now, block_on, and
//! synchronisation primitives (Barrier, Mutex, RwLock) through the
//! saikuro-exec facade.

use saikuro_exec::sync::{Barrier, Mutex, RwLock};
use std::sync::Arc;
use std::time::Duration;

// SPAWN / JOIN

#[test]
fn spawn_and_join() {
    saikuro_exec::block_on(async {
        let handle = saikuro_exec::spawn(async { 42 });
        assert_eq!(handle.await.unwrap(), 42);
    })
}

#[test]
fn spawn_multiple_tasks() {
    saikuro_exec::block_on(async {
        let mut handles = Vec::new();
        for i in 0..10 {
            handles.push(saikuro_exec::spawn(async move { i * i }));
        }
        let mut results: Vec<i32> = Vec::new();
        for h in handles {
            results.push(h.await.unwrap());
        }
        assert_eq!(results, (0..10).map(|i| i * i).collect::<Vec<_>>());
    })
}

#[test]
fn spawn_task_with_side_effect() {
    saikuro_exec::block_on(async {
        let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let f = flag.clone();
        let handle = saikuro_exec::spawn(async move {
            f.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        handle.await.unwrap();
        assert!(flag.load(std::sync::atomic::Ordering::SeqCst));
    })
}

#[test]
fn spawn_nested_tasks() {
    saikuro_exec::block_on(async {
        let outer = saikuro_exec::spawn(async {
            let inner = saikuro_exec::spawn(async { "nested" });
            inner.await.unwrap()
        });
        assert_eq!(outer.await.unwrap(), "nested");
    })
}

// SLEEP

#[test]
fn sleep_basic() {
    saikuro_exec::block_on(async {
        let start = std::time::Instant::now();
        saikuro_exec::sleep(Duration::from_millis(20)).await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(15), "slept {elapsed:?}");
    })
}

#[test]
fn sleep_zero_duration() {
    saikuro_exec::block_on(async {
        saikuro_exec::sleep(Duration::ZERO).await;
    })
}

#[test]
fn sleep_does_not_block_other_tasks() {
    saikuro_exec::block_on(async {
        let start = std::time::Instant::now();
        let h1 = saikuro_exec::spawn(async {
            saikuro_exec::sleep(Duration::from_millis(50)).await;
            1
        });
        let h2 = saikuro_exec::spawn(async {
            saikuro_exec::sleep(Duration::from_millis(50)).await;
            2
        });
        let (r1, r2) = (h1.await.unwrap(), h2.await.unwrap());
        let elapsed = start.elapsed();
        assert!(elapsed < Duration::from_millis(100), "took {elapsed:?}");
        assert_eq!(r1, 1);
        assert_eq!(r2, 2);
    })
}

// TIMEOUT

#[test]
fn timeout_completes_before_deadline() {
    saikuro_exec::block_on(async {
        let result = saikuro_exec::timeout(Duration::from_secs(10), async { "done" }).await;
        assert_eq!(result.unwrap(), "done");
    })
}

#[test]
fn timeout_exceeds_deadline() {
    saikuro_exec::block_on(async {
        let result = saikuro_exec::timeout(Duration::from_millis(10), async {
            saikuro_exec::sleep(Duration::from_secs(60)).await;
            "too slow"
        })
        .await;
        assert!(result.is_err(), "expected timeout error");
    })
}

// YIELD_NOW

#[test]
fn yield_now_does_not_block() {
    saikuro_exec::block_on(async {
        saikuro_exec::yield_now().await;
    })
}

#[test]
fn yield_now_allows_other_tasks_to_progress() {
    saikuro_exec::block_on(async {
        let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let f = flag.clone();
        let handle = saikuro_exec::spawn(async move {
            f.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        // Spin-yield until the flag is set.
        while !flag.load(std::sync::atomic::Ordering::SeqCst) {
            saikuro_exec::yield_now().await;
        }
        handle.await.unwrap();
    })
}

// BLOCK_ON

#[test]
fn block_on_returns_value() {
    let result = saikuro_exec::block_on(async { 7 + 11 });
    assert_eq!(result, 18);
}

#[test]
fn block_on_nested_block_on_panics() {
    // tokio does not allow nested block_on by default.
    let result = std::panic::catch_unwind(|| {
        saikuro_exec::block_on(async { saikuro_exec::block_on(async {}) })
    });
    assert!(result.is_err(), "nested block_on should panic");
}

// BARRIER

#[test]
fn barrier_synchronizes_two_tasks() {
    saikuro_exec::block_on(async {
        let barrier = Arc::new(Barrier::new(2));
        let b1 = barrier.clone();
        let h1 = saikuro_exec::spawn(async move { b1.wait().await });
        let b2 = barrier.clone();
        let h2 = saikuro_exec::spawn(async move { b2.wait().await });
        h1.await.unwrap();
        h2.await.unwrap();
    })
}

#[test]
fn barrier_multiple_tasks() {
    saikuro_exec::block_on(async {
        let n = 5;
        let barrier = Arc::new(Barrier::new(n));
        let mut handles = Vec::new();
        for _ in 0..n {
            let b = barrier.clone();
            handles.push(saikuro_exec::spawn(async move { b.wait().await }));
        }
        for h in handles {
            h.await.unwrap();
        }
    })
}

// MUTEX

#[test]
fn mutex_lock_unlock() {
    saikuro_exec::block_on(async {
        let mtx = Mutex::new(0u32);
        let mut guard = mtx.lock().await;
        *guard += 1;
        drop(guard);
        let guard = mtx.lock().await;
        assert_eq!(*guard, 1);
    })
}

#[test]
fn mutex_exclusive_access() {
    saikuro_exec::block_on(async {
        let mtx = Arc::new(Mutex::new(0u32));
        let mut handles = Vec::new();
        for _ in 0..10 {
            let m = mtx.clone();
            handles.push(saikuro_exec::spawn(async move {
                let mut guard = m.lock().await;
                *guard += 1;
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        assert_eq!(*mtx.lock().await, 10);
    })
}

// RWLock

#[test]
fn rwlock_read_allows_concurrent_reads() {
    saikuro_exec::block_on(async {
        let lock = Arc::new(RwLock::new(42u32));
        let r1 = lock.clone();
        let h1 = saikuro_exec::spawn(async move { *r1.read().await });
        let r2 = lock.clone();
        let h2 = saikuro_exec::spawn(async move { *r2.read().await });
        assert_eq!(h1.await.unwrap(), 42);
        assert_eq!(h2.await.unwrap(), 42);
    })
}

#[test]
fn rwlock_write_excludes_read() {
    saikuro_exec::block_on(async {
        let lock = Arc::new(RwLock::new(0u32));
        let w_lock = lock.clone();
        let writer = saikuro_exec::spawn(async move {
            let mut guard = w_lock.write().await;
            *guard = 100;
        });
        writer.await.unwrap();
        assert_eq!(*lock.read().await, 100);
    })
}
