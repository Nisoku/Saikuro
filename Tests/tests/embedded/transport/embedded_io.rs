//! `EmbeddedIoTransport` tests

use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use bytes::Bytes;
use core::cell::RefCell;

use embedded_io_async::{ErrorType, Read, Write};
use saikuro_tests::block_on;
use saikuro_transport::{
    EmbeddedIoTransport, LocalTransportReceiver, LocalTransportSender, TransportError,
};

struct FakeReader {
    wire: Vec<u8>,
    position: usize,
    chunk_size: usize,
}

impl FakeReader {
    fn new(wire: Vec<u8>, chunk_size: usize) -> Self {
        Self {
            wire,
            position: 0,
            chunk_size,
        }
    }
}

impl ErrorType for FakeReader {
    type Error = embedded_io_async::ErrorKind;
}

impl Read for FakeReader {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let available = self.wire.len().saturating_sub(self.position);
        let count = available.min(buf.len()).min(self.chunk_size);
        buf[..count].copy_from_slice(&self.wire[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }
}

struct FakeWriter {
    wire: Rc<RefCell<Vec<u8>>>,
    chunk_size: usize,
    write_zero: bool,
}

impl FakeWriter {
    fn new(chunk_size: usize) -> (Self, Rc<RefCell<Vec<u8>>>) {
        let wire = Rc::new(RefCell::new(Vec::new()));
        (
            Self {
                wire: Rc::clone(&wire),
                chunk_size,
                write_zero: false,
            },
            wire,
        )
    }

    fn write_zero() -> Self {
        Self {
            wire: Rc::new(RefCell::new(Vec::new())),
            chunk_size: 1,
            write_zero: true,
        }
    }
}

impl ErrorType for FakeWriter {
    type Error = embedded_io_async::ErrorKind;
}

impl Write for FakeWriter {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if self.write_zero {
            return Ok(0);
        }
        let count = buf.len().min(self.chunk_size);
        self.wire.borrow_mut().extend_from_slice(&buf[..count]);
        Ok(count)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn unused_writer() -> FakeWriter {
    FakeWriter::new(usize::MAX).0
}

pub fn register(suite: &mut saikuro_tests::TestSuite) {
    suite.register(
        "transport::embedded_io_roundtrip_preserves_order_with_partial_io",
        roundtrip_preserves_order_with_partial_io,
    );
    suite.register(
        "transport::embedded_io_clean_eof_returns_none",
        clean_eof_returns_none,
    );
    suite.register(
        "transport::embedded_io_oversized_header_is_rejected_before_payload_read",
        oversized_header_is_rejected_before_payload_read,
    );
    suite.register(
        "transport::embedded_io_constructor_rejects_limit_above_crate_maximum",
        constructor_rejects_limit_above_crate_maximum,
    );
    suite.register(
        "transport::embedded_io_truncated_header_is_an_error",
        truncated_header_is_an_error,
    );
    suite.register(
        "transport::embedded_io_truncated_payload_is_an_error",
        truncated_payload_is_an_error,
    );
    suite.register(
        "transport::embedded_io_write_zero_is_an_error",
        write_zero_is_an_error,
    );
}

fn roundtrip_preserves_order_with_partial_io() -> Result<(), &'static str> {
    block_on(async {
        let (writer, wire) = FakeWriter::new(2);
        let transport = EmbeddedIoTransport::new(FakeReader::new(Vec::new(), 1), writer, 64)
            .expect("valid frame limit");
        let (mut sender, _) = transport.split();

        sender
            .send(Bytes::from_static(b"first"))
            .await
            .expect("send first frame");
        sender
            .send(Bytes::from_static(b"second"))
            .await
            .expect("send second frame");

        let transport = EmbeddedIoTransport::new(
            FakeReader::new(wire.borrow().clone(), 1),
            unused_writer(),
            64,
        )
        .expect("valid frame limit");
        let (_, mut receiver) = transport.split();
        assert_eq!(
            receiver.recv().await.expect("receive first frame"),
            Some(Bytes::from_static(b"first"))
        );
        assert_eq!(
            receiver.recv().await.expect("receive second frame"),
            Some(Bytes::from_static(b"second"))
        );
        assert_eq!(receiver.recv().await.expect("clean EOF"), None);
    });
    Ok(())
}

fn clean_eof_returns_none() -> Result<(), &'static str> {
    block_on(async {
        let transport =
            EmbeddedIoTransport::new(FakeReader::new(Vec::new(), 1), unused_writer(), 16)
                .expect("valid frame limit");
        let (_, mut receiver) = transport.split();
        assert_eq!(receiver.recv().await.expect("clean EOF"), None);
    });
    Ok(())
}

fn oversized_header_is_rejected_before_payload_read() -> Result<(), &'static str> {
    block_on(async {
        let transport = EmbeddedIoTransport::new(
            FakeReader::new(17_u32.to_be_bytes().to_vec(), 4),
            unused_writer(),
            16,
        )
        .expect("valid frame limit");
        let (_, mut receiver) = transport.split();
        assert!(matches!(
            receiver.recv().await,
            Err(TransportError::MessageTooLarge {
                size: 17,
                limit: 16
            })
        ));
    });
    Ok(())
}

fn constructor_rejects_limit_above_crate_maximum() -> Result<(), &'static str> {
    let result = EmbeddedIoTransport::new(
        FakeReader::new(Vec::new(), 1),
        unused_writer(),
        saikuro_transport::MAX_FRAME_SIZE + 1,
    );
    assert!(matches!(
        result,
        Err(TransportError::MessageTooLarge { .. })
    ));
    Ok(())
}

fn truncated_header_is_an_error() -> Result<(), &'static str> {
    block_on(async {
        let transport =
            EmbeddedIoTransport::new(FakeReader::new(vec![0, 0, 0], 1), unused_writer(), 16)
                .expect("valid frame limit");
        let (_, mut receiver) = transport.split();
        match receiver.recv().await {
            Err(TransportError::ConnectionLost(message)) => assert!(message.contains("header")),
            other => panic!("expected truncated header error, got {other:?}"),
        }
    });
    Ok(())
}

fn truncated_payload_is_an_error() -> Result<(), &'static str> {
    block_on(async {
        let mut wire = 4_u32.to_be_bytes().to_vec();
        wire.extend_from_slice(b"abc");
        let transport = EmbeddedIoTransport::new(FakeReader::new(wire, 2), unused_writer(), 16)
            .expect("valid frame limit");
        let (_, mut receiver) = transport.split();
        match receiver.recv().await {
            Err(TransportError::ConnectionLost(_)) => {}
            other => panic!("expected truncated payload error, got {other:?}"),
        }
    });
    Ok(())
}

fn write_zero_is_an_error() -> Result<(), &'static str> {
    block_on(async {
        let transport =
            EmbeddedIoTransport::new(FakeReader::new(Vec::new(), 1), FakeWriter::write_zero(), 16)
                .expect("valid frame limit");
        let (mut sender, _) = transport.split();
        assert!(matches!(
            sender.send(Bytes::from_static(b"data")).await,
            Err(TransportError::FramingError(message))
                if message == "write made no progress"
        ));
    });
    Ok(())
}
