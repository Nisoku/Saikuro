#![cfg(feature = "embedded")]

extern crate alloc;

use saikuro_exec::start_runner;
use saikuro_runtime::embedded::serve_with;
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
async fn main(spawner: embassy_executor::Spawner) {
    start_runner(spawner);
    let runtime = saikuro_runtime::SaikuroRuntime::builder().build().await;
    let listener = TcpTransportListener::new(board::stack(), board::endpoint());
    serve_with(&runtime, alloc::vec![listener]).await;
}
