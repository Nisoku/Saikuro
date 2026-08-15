#![cfg(feature = "no_std")]
#![no_std]

extern crate alloc;

use alloc::sync::Arc;

use saikuro_exec::watch;
use saikuro_runtime::transport_adapter::{HostPipeListener, LocalRuntimeListener};
use saikuro_runtime::SaikuroRuntime;
use saikuro_transport::wasi::host::WasiPipe;
use saikuro_transport::wasi::tcp::WasiTcpListener;

/// WASI command entry point. Returns a process exit code.
#[no_mangle]
pub extern "C" fn _start() -> i32 {
    let runtime = Arc::new(SaikuroRuntime::builder().build());
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);

    let tcp = match WasiTcpListener::new("0.0.0.0:7700") {
        Ok(listener) => LocalRuntimeListener::new(listener),
        Err(_) => return 1,
    };
    let pipe = HostPipeListener::<WasiPipe>::new("saikuro");

    saikuro_exec::block_on(async move {
        let mut rx1 = shutdown_rx.clone();
        let mut rx2 = shutdown_rx.clone();

        let tcp_task = {
            let rt = runtime.clone();
            saikuro_exec::spawn(async move {
                rt.serve(vec![tcp], rx1).await;
            })
        };
        let pipe_task = {
            let rt = runtime.clone();
            saikuro_exec::spawn(async move {
                rt.serve(vec![pipe], rx2).await;
            })
        };

        let _ = tcp_task.await;
        let _ = pipe_task.await;
    });

    0
}
