use crate::handle::RuntimeHandle;
use crate::SaikuroRuntime;
use saikuro_exec::watch;

/// Build and serve the runtime with the provided listeners until the runtime
/// signals shutdown.
pub async fn serve_with<L: crate::transport_adapter::RuntimeListener + 'static>(
    runtime: &SaikuroRuntime,
    listeners: alloc::vec::Vec<L>,
) {
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    runtime.serve(listeners, shutdown_rx).await;
}

/// Accept a connected transport and spawn a connection handler on the runtime.
pub fn accept(
    handle: &RuntimeHandle,
    transport: impl crate::transport_adapter::RuntimeTransport + 'static,
    peer_id: impl Into<alloc::string::String>,
    peer_caps: saikuro_core::capability::CapabilitySet,
) {
    handle.accept_transport(transport, peer_id, peer_caps);
}
