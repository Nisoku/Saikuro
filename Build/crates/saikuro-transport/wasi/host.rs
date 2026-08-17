use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use async_trait::async_trait;
use bytes::Bytes;

use crate::shared::error::{Result, TransportError};
use crate::shared::host::{HostPipeFactory, HostPipeRecv, HostPipeSend, Role};
use crate::shared::traits::{
    LocalTransport, LocalTransportConnector, LocalTransportListener, LocalTransportReceiver,
    LocalTransportSender,
};
use crate::wasi::tcp::{WasiTcpConnector, WasiTcpListener, WasiTcpReceiver, WasiTcpSender};

/// Base of the deterministic loopback rendezvous port range.
const PIPE_PORT_BASE: u16 = 0xC000;
/// Number of ports in the rendezvous range (ephemeral space).
const PIPE_PORT_SPAN: u16 = 0x1000;

/// The WASI `HostPipeFactory` backend.
pub struct WasiPipe;

/// Sending half of a [`WasiPipe`] connection.
pub struct WasiHostSend(pub WasiTcpSender);

/// Receiving half of a [`WasiPipe`] connection.
pub struct WasiHostRecv(pub WasiTcpReceiver);

#[async_trait(?Send)]
impl HostPipeSend for WasiHostSend {
    async fn send(&mut self, frame: &[u8]) -> Result<()> {
        self.0.send(Bytes::copy_from_slice(frame)).await
    }
}

#[async_trait(?Send)]
impl HostPipeRecv for WasiHostRecv {
    async fn recv(&mut self) -> Result<Option<Vec<u8>>> {
        match self.0.recv().await? {
            Some(bytes) => Ok(Some(bytes.to_vec())),
            None => Ok(None),
        }
    }
}

#[async_trait(?Send)]
impl HostPipeFactory for WasiPipe {
    type Send = WasiHostSend;
    type Recv = WasiHostRecv;

    async fn open(channel: &str, role: Role) -> Result<(Self::Send, Self::Recv)> {
        let addr = loopback_addr(channel_port(channel));
        match role {
            Role::Connect => {
                let transport = WasiTcpConnector::new(addr).connect().await?;
                let (mut tx, mut rx) = transport.split();
                tx.send(Bytes::from_static(b"connect")).await?;
                match rx.recv().await? {
                    Some(bytes) if bytes.as_ref() == b"accept" => {}
                    _ => {
                        return Err(TransportError::ConnectionLost(
                            "wasi-host handshake: expected accept".into(),
                        ))
                    }
                }
                Ok((WasiHostSend(tx), WasiHostRecv(rx)))
            }
            Role::Accept => {
                let mut listener = WasiTcpListener::new(addr)?;
                let transport = listener.accept().await?.ok_or_else(|| {
                    TransportError::ConnectionLost("wasi-host listener closed".into())
                })?;
                let (mut tx, mut rx) = transport.split();
                match rx.recv().await? {
                    Some(bytes) if bytes.as_ref() == b"connect" => {}
                    _ => {
                        return Err(TransportError::ConnectionLost(
                            "wasi-host handshake: expected connect".into(),
                        ))
                    }
                }
                tx.send(Bytes::from_static(b"accept")).await?;
                Ok((WasiHostSend(tx), WasiHostRecv(rx)))
            }
        }
    }
}

/// Map a channel name to a deterministic loopback port.
fn channel_port(channel: &str) -> u16 {
    let mut hash: u32 = 0x811c_9dc5;
    for b in channel.as_bytes() {
        hash ^= u32::from(*b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    PIPE_PORT_BASE.wrapping_add((hash & (PIPE_PORT_SPAN as u32 - 1)) as u16)
}

/// Build a `127.0.0.1:port` rendezvous address.
fn loopback_addr(port: u16) -> String {
    alloc::format!("127.0.0.1:{port}")
}
