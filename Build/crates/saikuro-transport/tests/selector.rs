#![cfg(not(target_arch = "wasm32"))]

use saikuro_transport::{TransportKind, TransportSelector};

#[test]
fn quic_selection_requires_native_and_quic() {
    let (kind, address) = TransportSelector::select(Some("quic://[::1]:443"), None);
    #[cfg(all(feature = "native", feature = "quic"))]
    {
        assert_eq!(kind, TransportKind::Quic);
        assert_eq!(address.as_deref(), Some("[::1]:443"));
    }
    #[cfg(not(all(feature = "native", feature = "quic")))]
    {
        assert_eq!(kind, TransportKind::Tcp);
        assert_eq!(address.as_deref(), Some("quic://[::1]:443"));
    }
}
