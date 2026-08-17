use crate::SaikuroRuntime;
use saikuro_exec::watch;
use saikuro_net::net::Stack;
use saikuro_transport::embedded::tcp::TcpTransportListener;

/// Run the runtime against a host-provided network stack. The firmware is
/// responsible for supplying the stack and endpoint (see `board` integration).
pub async fn run(stack: &'static Stack<'static>, endpoint: saikuro_net::net::IpEndpoint) {
    let runtime = SaikuroRuntime::builder().build().await;
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    runtime
        .serve(
            vec![TcpTransportListener::new(stack, endpoint)],
            shutdown_rx,
        )
        .await;
}
