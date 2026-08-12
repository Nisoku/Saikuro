//! Embassy networking facade (`saikuro_exec::net`).
//!
//! This is the `no_std` counterpart of the host `net` module
//!
//! # Ownership model
//!
//! - the application provides the device driver (the [`driver`] module) and
//!   the [`StackResources`] memory for sockets;
//! - [`Stack::new`] returns the [`Stack`] handle plus a [`Runner`]; the runner
//!   must be driven to completion on a task, otherwise the stack never
//!   processes packets or wakes sockets;
//! - [`tcp::TcpSocket`] and [`udp::UdpSocket`] are created from the `Stack`
//!   handle with caller-provided send and receive buffers.
//!
//! Unlike the host backend there is no global stack, so address and port
//! binding are explicit and the app controls every resource lifetime.

pub mod net {
    pub use embassy_net::*;
}
