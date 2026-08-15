#![cfg(feature = "embedded")]

extern crate alloc;

use saikuro_exec::watch;
use saikuro_net::net::Stack;
use saikuro_runtime::SaikuroRuntime;
use saikuro_transport::embedded::tcp::TcpTransportListener;

/// Host-provided board support.
mod board {
    use saikuro_net::net::Stack;

    pub fn stack() -> &'static Stack<'static> {
        compile_error!(
            "provide `crate::board::stack() -> &'static Stack<'static>` in the firmware"
        );
    }

    pub fn endpoint() -> saikuro_net::net::IpEndpoint {
        compile_error!("provide `crate::board::endpoint() -> IpEndpoint` in the firmware");
    }
}

#[embassy_executor::main]
async fn main() {
    let stack = board::stack();
    let runtime = SaikuroRuntime::builder().build();

    let (_shutdown_tx, shutdown_rx) = watch::channel(false);

    runtime
        .serve(
            vec![TcpTransportListener::new(stack, board::endpoint())],
            shutdown_rx,
        )
        .await;
}
