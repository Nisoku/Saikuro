pub mod framed;
pub mod io_transport;

#[cfg(feature = "tcp")]
pub mod tcp;

pub use io_transport::{EmbeddedIoReceiver, EmbeddedIoSender, EmbeddedIoTransport};

#[cfg(feature = "tcp")]
pub use tcp::TcpTransport;
