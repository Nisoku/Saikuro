//! Length-prefixed framing tests for stream transports.
//!
//! Covers the no_std [`LengthPrefixedCodec`] directly and the native
//! [`FramedStream`] adapter over an in-memory duplex stream and over real
//! TCP.  The native adapter is not available on wasm32 (no
//! `native-transport` feature there), so this file is native-only.

#![cfg(not(target_arch = "wasm32"))]

use bytes::{BufMut, Bytes, BytesMut};
use futures::{SinkExt, StreamExt};
use saikuro_exec::block_on;
use saikuro_exec::io::AsyncWriteExt;
use saikuro_transport::error::TransportError;
use saikuro_transport::framing::{FramedStream, LengthPrefixedCodec};

fn encode_frames(items: &[Bytes]) -> BytesMut {
    let mut codec = LengthPrefixedCodec::new();
    let mut out = BytesMut::new();
    for item in items {
        codec.encode(item.clone(), &mut out).expect("encode");
    }
    out
}

#[test]
fn codec_roundtrip_preserves_frames() {
    let items = vec![
        Bytes::from_static(b"hello"),
        Bytes::new(),
        Bytes::from(vec![0xAB; 100_000]),
        Bytes::from_static(b"goodbye"),
    ];
    let mut wire = encode_frames(&items);

    let mut codec = LengthPrefixedCodec::new();
    for expected in &items {
        let got = codec.decode(&mut wire).expect("decode").expect("frame");
        assert_eq!(&got, expected);
    }
    // Every byte should have been consumed.
    assert!(wire.is_empty());
    // Decoding an empty buffer yields nothing, not an error.
    assert!(codec.decode(&mut wire).expect("decode").is_none());
}

#[test]
fn codec_handles_partial_input() {
    let wire = encode_frames(&[Bytes::from_static(b"ping")]);
    let mut codec = LengthPrefixedCodec::new();

    // Feed the wire bytes one at a time; only the final byte completes a frame.
    let mut buf = BytesMut::new();
    let mut remaining = wire;
    let got = loop {
        if !remaining.is_empty() {
            let byte = remaining.split_to(1);
            buf.extend_from_slice(&byte);
        }
        match codec.decode(&mut buf) {
            Ok(Some(frame)) => break frame,
            Ok(None) if remaining.is_empty() => {
                panic!("frame never completed");
            }
            Ok(None) => continue,
            Err(e) => panic!("unexpected decode error: {e}"),
        }
    };
    assert_eq!(got, Bytes::from_static(b"ping"));
}

#[test]
fn codec_rejects_oversized_frame_then_recovers() {
    // Forge a length header just over MAX_FRAME_SIZE and retain the declared
    // trailing payload on the wire.  The codec must swallow exactly that many
    // bytes so they are not misread as a fresh header, then resynchronize at
    // the valid frame that follows on the same buffer.
    let forged = saikuro_transport::MAX_FRAME_SIZE as u32 + 3;
    let valid = encode_frames(&[Bytes::from_static(b"ok")]);
    let mut wire = BytesMut::new();
    wire.put_u32(forged);
    wire.resize(4 + forged as usize, 0);
    wire.extend_from_slice(&valid);

    let mut codec = LengthPrefixedCodec::new();
    match codec.decode(&mut wire) {
        Err(TransportError::MessageTooLarge { .. }) => {}
        other => panic!("expected MessageTooLarge, got {other:?}"),
    }

    let got = codec.decode(&mut wire).expect("decode").expect("frame");
    assert_eq!(got, Bytes::from_static(b"ok"));
    assert!(wire.is_empty(), "all wire bytes consumed");
}

#[test]
fn codec_encode_rejects_oversized_frame() {
    let too_big = Bytes::from(vec![0u8; saikuro_transport::MAX_FRAME_SIZE + 1]);
    let mut codec = LengthPrefixedCodec::new();
    let mut out = BytesMut::new();
    match codec.encode(too_big, &mut out) {
        Err(TransportError::MessageTooLarge { .. }) => {}
        other => panic!("expected MessageTooLarge, got {other:?}"),
    }
}

#[test]
fn framed_stream_roundtrips_multiple_frames() {
    block_on(async {
        let (client, server) = saikuro_exec::io::duplex(1024 * 1024);
        let framed_client = FramedStream::new(client);
        let (mut tx, _rx) = framed_client.split();
        let mut framed_server = FramedStream::new(server);

        let frames = vec![
            Bytes::from_static(b"a"),
            Bytes::from_static(b"bb"),
            Bytes::from(vec![0x42; 100_000]),
        ];
        for frame in &frames {
            tx.send(frame.clone()).await.expect("send");
        }
        tx.close().await.expect("close");

        for expected in &frames {
            let got = framed_server.next().await.expect("stream").expect("frame");
            assert_eq!(&got, expected);
        }
        // Clean EOF after the sender closed.
        assert!(framed_server.next().await.is_none());
    })
}

#[test]
fn framed_stream_truncated_frame_errors() {
    block_on(async {
        let (client, server) = saikuro_exec::io::duplex(4096);
        // Write a length header promising 100 bytes, then only 3 bytes, and
        // drop the write half: the reader must report a framing error, not
        // silently return a short frame or hang.
        let (_rx, mut tx) = saikuro_exec::io::split(client);
        let mut framed_server = FramedStream::new(server);

        let mut partial = BytesMut::new();
        partial.put_u32(100);
        partial.put_slice(b"abc");
        tx.write_all(&partial).await.expect("write");
        // Shut down the write half so the reader sees EOF after the partial
        // frame (dropping the half alone does not signal EOF on a duplex).
        tx.shutdown().await.expect("shutdown");
        drop(tx);

        match framed_server.next().await {
            Some(Err(TransportError::FramingError(_))) => {}
            other => panic!("expected FramingError, got {other:?}"),
        }
        // The stream is terminal after a framing error.
        assert!(framed_server.next().await.is_none());
    })
}

#[test]
fn framed_stream_rejects_header_only_eof() {
    block_on(async {
        let (client, server) = saikuro_exec::io::duplex(4096);
        let (_rx, mut tx) = saikuro_exec::io::split(client);
        let mut framed_server = FramedStream::new(server);

        let mut header = BytesMut::new();
        header.put_u32(100);
        tx.write_all(&header).await.expect("write");
        tx.shutdown().await.expect("shutdown");
        drop(tx);

        match framed_server.next().await {
            Some(Err(TransportError::FramingError(_))) => {}
            other => panic!("expected FramingError, got {other:?}"),
        }
        assert!(framed_server.next().await.is_none());
    })
}

#[test]
fn framed_stream_stays_terminal_after_oversized_frame_error() {
    block_on(async {
        let (client, server) = saikuro_exec::io::duplex(4096);
        let (_rx, mut tx) = saikuro_exec::io::split(client);
        let mut framed_server = FramedStream::new(server);

        // Forge an oversized length header.  The byte stream is unaligned
        // after it, so the reader must error once and stay terminal rather
        // than resuming and misreading payload bytes as a header.
        let mut wire = BytesMut::new();
        wire.put_u32(u32::MAX);
        tx.write_all(&wire).await.expect("write");
        tx.shutdown().await.expect("shutdown");
        drop(tx);

        match framed_server.next().await {
            Some(Err(TransportError::MessageTooLarge { .. })) => {}
            other => panic!("expected MessageTooLarge, got {other:?}"),
        }
        assert!(framed_server.next().await.is_none());
    })
}

#[test]
fn framed_stream_supports_bidirectional_use() {
    block_on(async {
        let (client, server) = saikuro_exec::io::duplex(4096);
        let (mut client_tx, mut client_rx) = FramedStream::new(client).split();
        let (mut server_tx, mut server_rx) = FramedStream::new(server).split();

        client_tx.send(Bytes::from_static(b"ping")).await.unwrap();
        let got = server_rx.next().await.unwrap().unwrap();
        assert_eq!(got, Bytes::from_static(b"ping"));

        server_tx.send(Bytes::from_static(b"pong")).await.unwrap();
        let got = client_rx.next().await.unwrap().unwrap();
        assert_eq!(got, Bytes::from_static(b"pong"));
    })
}

#[test]
fn framed_stream_tcp_roundtrip_concurrent() {
    block_on(async {
        let listener = saikuro_exec::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("local addr");

        let server_task = saikuro_exec::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut framed = FramedStream::new(stream);
            let mut frames = Vec::new();
            while let Some(frame) = framed.next().await {
                frames.push(frame.expect("frame"));
            }
            frames
        });

        let client_task = saikuro_exec::spawn(async move {
            let stream = saikuro_exec::net::TcpStream::connect(addr)
                .await
                .expect("connect");
            let (mut tx, _rx) = FramedStream::new(stream).split();
            for i in 0..5 {
                let payload = Bytes::from(vec![i as u8; 300_000]);
                tx.send(payload.clone()).await.expect("send");
            }
            tx.close().await.expect("close");
        });

        let (client_res, server_res) = (client_task.await, server_task.await);
        client_res.expect("client task");
        let frames = server_res.expect("server task");
        assert_eq!(frames.len(), 5, "expected 5 frames, got {}", frames.len());
        for (i, frame) in frames.iter().enumerate() {
            assert_eq!(frame.len(), 300_000, "frame {i} wrong length");
            assert!(
                frame.iter().all(|&b| b == i as u8),
                "frame {i} content wrong"
            );
        }
    })
}

#[test]
fn framed_stream_tcp_raw_writer() {
    // Server side uses FramedStream; client writes pre-encoded wire bytes
    // directly, isolating the read path.
    block_on(async {
        let listener = saikuro_exec::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("local addr");

        let server_task = saikuro_exec::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut framed = FramedStream::new(stream);
            let mut frames = Vec::new();
            while let Some(frame) = framed.next().await {
                frames.push(frame.expect("frame"));
            }
            frames
        });

        let client_task = saikuro_exec::spawn(async move {
            let stream = saikuro_exec::net::TcpStream::connect(addr)
                .await
                .expect("connect");
            let (_r, mut w) = saikuro_exec::io::split(stream);
            let mut codec = LengthPrefixedCodec::new();
            let mut wire = BytesMut::new();
            for i in 0..5 {
                let payload = Bytes::from(vec![i as u8; 300_000]);
                codec.encode(payload, &mut wire).expect("encode");
            }
            w.write_all(&wire).await.expect("write_all");
            w.shutdown().await.expect("shutdown");
            drop(w);
        });

        let (client_res, server_res) = (client_task.await, server_task.await);
        client_res.expect("client task");
        let frames = server_res.expect("server task");
        assert_eq!(frames.len(), 5, "expected 5 frames, got {}", frames.len());
        for (i, frame) in frames.iter().enumerate() {
            assert_eq!(frame.len(), 300_000, "frame {i} wrong length");
            assert!(
                frame.iter().all(|&b| b == i as u8),
                "frame {i} content wrong"
            );
        }
    })
}
