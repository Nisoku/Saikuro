//! Native-only concurrency and time tests for saikuro-exec.

use saikuro_exec::sync::{Barrier, Mutex, RwLock};
use std::sync::Arc;
use std::time::Duration;

pub fn register(suite: &mut saikuro_tests::TestSuite) {
    suite.register("exec::spawn_and_join", spawn_and_join);
    suite.register("exec::spawn_multiple_tasks", spawn_multiple_tasks);
    suite.register(
        "exec::spawn_task_with_side_effect",
        spawn_task_with_side_effect,
    );
    suite.register("exec::spawn_nested_tasks", spawn_nested_tasks);
    suite.register("exec::sleep_basic", sleep_basic);
    suite.register("exec::sleep_zero_duration", sleep_zero_duration);
    suite.register(
        "exec::sleep_does_not_block_other_tasks",
        sleep_does_not_block_other_tasks,
    );
    suite.register(
        "exec::timeout_completes_before_deadline",
        timeout_completes_before_deadline,
    );
    suite.register("exec::timeout_exceeds_deadline", timeout_exceeds_deadline);
    suite.register("exec::yield_now_does_not_block", yield_now_does_not_block);
    suite.register(
        "exec::yield_now_allows_other_tasks_to_progress",
        yield_now_allows_other_tasks_to_progress,
    );
    suite.register("exec::block_on_returns_value", block_on_returns_value);
    suite.register(
        "exec::barrier_synchronizes_two_tasks",
        barrier_synchronizes_two_tasks,
    );
    suite.register("exec::barrier_multiple_tasks", barrier_multiple_tasks);
    suite.register("exec::mutex_lock_unlock", mutex_lock_unlock);
    suite.register("exec::mutex_exclusive_access", mutex_exclusive_access);
    suite.register(
        "exec::rwlock_read_allows_concurrent_reads",
        rwlock_read_allows_concurrent_reads,
    );
    suite.register(
        "exec::rwlock_write_excludes_read",
        rwlock_write_excludes_read,
    );
}

fn spawn_and_join() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let handle = saikuro_exec::spawn(async { 42 });
        assert_eq!(handle.await.map_err(|_| "join")?, 42);
        Ok(())
    })
}

fn spawn_multiple_tasks() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let mut handles = Vec::new();
        for i in 0..10 {
            handles.push(saikuro_exec::spawn(async move { i * i }));
        }
        let mut results: Vec<i32> = Vec::new();
        for h in handles {
            results.push(h.await.map_err(|_| "join")?);
        }
        assert_eq!(results, (0..10).map(|i| i * i).collect::<Vec<_>>());
        Ok(())
    })
}

fn spawn_task_with_side_effect() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let f = flag.clone();
        let handle = saikuro_exec::spawn(async move {
            f.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        handle.await.map_err(|_| "join")?;
        assert!(flag.load(std::sync::atomic::Ordering::SeqCst));
        Ok(())
    })
}

fn spawn_nested_tasks() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let outer = saikuro_exec::spawn(async {
            let inner = saikuro_exec::spawn(async { "nested" });
            inner.await.unwrap()
        });
        assert_eq!(outer.await.map_err(|_| "join")?, "nested");
        Ok(())
    })
}

fn sleep_basic() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let start = std::time::Instant::now();
        saikuro_exec::sleep(Duration::from_millis(20)).await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(15), "slept {elapsed:?}");
        Ok(())
    })
}

fn sleep_zero_duration() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        saikuro_exec::sleep(Duration::ZERO).await;
        Ok(())
    })
}

fn sleep_does_not_block_other_tasks() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
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
        Ok(())
    })
}

fn timeout_completes_before_deadline() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let result = saikuro_exec::timeout(Duration::from_secs(10), async { "done" }).await;
        assert_eq!(result.map_err(|_| "timeout")?, "done");
        Ok(())
    })
}

fn timeout_exceeds_deadline() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let result = saikuro_exec::timeout(Duration::from_millis(10), async {
            saikuro_exec::sleep(Duration::from_secs(60)).await;
            "too slow"
        })
        .await;
        assert!(result.is_err(), "expected timeout error");
        Ok(())
    })
}

fn yield_now_does_not_block() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        saikuro_exec::yield_now().await;
        Ok(())
    })
}

fn yield_now_allows_other_tasks_to_progress() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let f = flag.clone();
        let handle = saikuro_exec::spawn(async move {
            f.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        // Spin-yield until the flag is set.
        while !flag.load(std::sync::atomic::Ordering::SeqCst) {
            saikuro_exec::yield_now().await;
        }
        handle.await.map_err(|_| "join")?;
        Ok(())
    })
}

fn block_on_returns_value() -> Result<(), &'static str> {
    let result = saikuro_tests::block_on(async { 7 + 11 });
    assert_eq!(result, 18);
    Ok(())
}

fn barrier_synchronizes_two_tasks() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let barrier = Arc::new(Barrier::new(2));
        let b1 = barrier.clone();
        let h1 = saikuro_exec::spawn(async move { b1.wait().await });
        let b2 = barrier.clone();
        let h2 = saikuro_exec::spawn(async move { b2.wait().await });
        h1.await.map_err(|_| "join")?;
        h2.await.map_err(|_| "join")?;
        Ok(())
    })
}

fn barrier_multiple_tasks() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let n = 5;
        let barrier = Arc::new(Barrier::new(n));
        let mut handles = Vec::new();
        for _ in 0..n {
            let b = barrier.clone();
            handles.push(saikuro_exec::spawn(async move { b.wait().await }));
        }
        for h in handles {
            h.await.map_err(|_| "join")?;
        }
        Ok(())
    })
}

fn mutex_lock_unlock() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let mtx = Mutex::new(0u32);
        let mut guard = mtx.lock().await;
        *guard += 1;
        drop(guard);
        let guard = mtx.lock().await;
        assert_eq!(*guard, 1);
        Ok(())
    })
}

fn mutex_exclusive_access() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
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
            h.await.map_err(|_| "join")?;
        }
        assert_eq!(*mtx.lock().await, 10);
        Ok(())
    })
}

fn rwlock_read_allows_concurrent_reads() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let lock = Arc::new(RwLock::new(42u32));
        let r1 = lock.clone();
        let h1 = saikuro_exec::spawn(async move { *r1.read().await });
        let r2 = lock.clone();
        let h2 = saikuro_exec::spawn(async move { *r2.read().await });
        assert_eq!(h1.await.map_err(|_| "join")?, 42);
        assert_eq!(h2.await.map_err(|_| "join")?, 42);
        Ok(())
    })
}

fn rwlock_write_excludes_read() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let lock = Arc::new(RwLock::new(0u32));
        let w_lock = lock.clone();
        let writer = saikuro_exec::spawn(async move {
            let mut guard = w_lock.write().await;
            *guard = 100;
        });
        writer.await.map_err(|_| "join")?;
        assert_eq!(*lock.read().await, 100);
        Ok(())
    })
}
