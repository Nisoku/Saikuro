#![cfg(feature = "embedded")]

extern crate alloc;

use saikuro_exec::start_runner;
use saikuro_runtime::embedded::run;

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
    run(board::stack(), board::endpoint()).await;
}
