//! Host-run loopback test for the Embassy net facade (`saikuro_exec::net`).
//!
//! Two `embassy-net` stacks are bridged back to back through in-memory
//! `embassy-net-driver-channel` devices. A TCP connection is opened between
//! them and data flows in both directions. This exercises the re-exported
//! surface (config, addresses, sockets) on the host, without any hardware.
//!
//! Run with: `cargo test -p saikuro-exec --no-default-features --features embassy-test,net`

#![cfg(all(feature = "embassy-test", feature = "net"))]

use core::time::Duration as CoreDuration;

use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

use embassy_net_driver_channel::driver::{HardwareAddress, LinkState};
use embassy_net_driver_channel::{RxRunner, State, TxRunner};

use saikuro_exec::net::{
    tcp, Config, IpAddress, IpEndpoint, Ipv4Address, Ipv4Cidr, StackResources, StaticConfigV4,
};
use saikuro_exec::timeout;

const MTU: usize = 1500;
const CHAN_RX: usize = 4;
const CHAN_TX: usize = 4;
const SOCKET_BUFFER: usize = 4096;
const TEST_TIMEOUT: CoreDuration = CoreDuration::from_secs(30);
const PORT: u16 = 4242;

const ADDR_A: Ipv4Address = Ipv4Address::new(10, 0, 0, 1);
const ADDR_B: Ipv4Address = Ipv4Address::new(10, 0, 0, 2);

/// Copy every outbound packet of the source stack into the inbound path of the
/// destination stack. Runs forever; dropped when the test body completes.
async fn bridge<const M: usize>(src_tx: &mut TxRunner<'_, M>, dst_rx: &mut RxRunner<'_, M>) {
    loop {
        let len = {
            let pkt = src_tx.tx_buf().await;
            let len = pkt.len();
            let dst = dst_rx.rx_buf().await;
            dst[..len].copy_from_slice(&pkt[..len]);
            len
        };
        dst_rx.rx_done(len);
        src_tx.tx_done();
    }
}

#[test]
fn tcp_loopback_between_two_stacks() {
    futures_executor::block_on(async {
        let outcome = timeout(TEST_TIMEOUT, async {
            let mut state_a = State::<MTU, CHAN_RX, CHAN_TX>::new();
            let mut state_b = State::<MTU, CHAN_RX, CHAN_TX>::new();

            let (mut chan_runner_a, device_a) =
                embassy_net_driver_channel::new(&mut state_a, HardwareAddress::Ip);
            let (mut chan_runner_b, device_b) =
                embassy_net_driver_channel::new(&mut state_b, HardwareAddress::Ip);
            chan_runner_a.set_link_state(LinkState::Up);
            chan_runner_b.set_link_state(LinkState::Up);

            let mut resources_a = StackResources::<2>::new();
            let mut resources_b = StackResources::<2>::new();

            let config_a = Config::ipv4_static(StaticConfigV4 {
                address: Ipv4Cidr::new(ADDR_A, 24),
                gateway: None,
                dns_servers: Default::default(),
            });
            let config_b = Config::ipv4_static(StaticConfigV4 {
                address: Ipv4Cidr::new(ADDR_B, 24),
                gateway: None,
                dns_servers: Default::default(),
            });

            let (stack_a, mut stack_runner_a) =
                saikuro_exec::net::new(device_a, config_a, &mut resources_a, 1234);
            let (stack_b, mut stack_runner_b) =
                saikuro_exec::net::new(device_b, config_b, &mut resources_b, 4321);

            let (_state_runner_a, mut rx_runner_a, mut tx_runner_a) = chan_runner_a.split();
            let (_state_runner_b, mut rx_runner_b, mut tx_runner_b) = chan_runner_b.split();

            let mut sock_a_rx = [0u8; SOCKET_BUFFER];
            let mut sock_a_tx = [0u8; SOCKET_BUFFER];
            let mut sock_b_rx = [0u8; SOCKET_BUFFER];
            let mut sock_b_tx = [0u8; SOCKET_BUFFER];

            let body = async {
                stack_a.wait_config_up().await;
                stack_b.wait_config_up().await;

                let mut sock_a = tcp::TcpSocket::new(stack_a, &mut sock_a_rx, &mut sock_a_tx);
                let mut sock_b = tcp::TcpSocket::new(stack_b, &mut sock_b_rx, &mut sock_b_tx);

                // accept() waits for the first connection, so it must be
                // driven concurrently with the peer's connect().
                let server = IpEndpoint::new(IpAddress::Ipv4(ADDR_A), PORT);
                let (accept_res, connect_res) =
                    futures::join!(sock_a.accept(PORT), sock_b.connect(server));
                accept_res.expect("bind + listen");
                connect_res.expect("connect");

                let mut ping = [0u8; 4];
                sock_b.write(b"ping").await.expect("write ping");
                sock_b.flush().await.expect("flush ping");
                let n = sock_a.read(&mut ping).await.expect("read ping");
                assert_eq!(n, 4);
                assert_eq!(&ping, b"ping");

                sock_a.write(&ping).await.expect("write echo");
                sock_a.flush().await.expect("flush echo");
                let mut echo = [0u8; 4];
                let n = sock_b.read(&mut echo).await.expect("read echo");
                assert_eq!(n, 4);
                assert_eq!(&echo, b"ping");
            };

            // Poll the two stack runners, the two bridges, and the test body
            // together. The runners and bridges never complete; the future
            // resolves once the body finishes.
            let mut body = Box::pin(body);
            let mut runner_a = Box::pin(stack_runner_a.run());
            let mut runner_b = Box::pin(stack_runner_b.run());
            let mut bridge_ab = Box::pin(bridge(&mut tx_runner_a, &mut rx_runner_b));
            let mut bridge_ba = Box::pin(bridge(&mut tx_runner_b, &mut rx_runner_a));

            std::future::poll_fn(move |cx| {
                let mut finished = false;
                if let Poll::Ready(_) = Pin::new(&mut body).poll(cx) {
                    finished = true;
                }
                let _ = Pin::new(&mut runner_a).poll(cx);
                let _ = Pin::new(&mut runner_b).poll(cx);
                let _ = Pin::new(&mut bridge_ab).poll(cx);
                let _ = Pin::new(&mut bridge_ba).poll(cx);
                if finished {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            })
            .await
        })
        .await;

        outcome.expect("net loopback test timed out")
    });
}
