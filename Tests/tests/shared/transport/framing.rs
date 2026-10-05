use crate::shared_test;
use crate::TestSuite;
use core::cmp;
use saikuro_transport::shared::framing::{
    decode_length_prefix, encode_length_prefix, read_frame, write_frame, AsyncByteRead,
    AsyncByteWrite,
};
use saikuro_transport::{TransportError, MAX_FRAME_SIZE};

// The length-prefixed wire codec, tested against in-memory byte sinks

struct ReplayReader {
    buf: crate::Vec<u8>,
    pos: usize,
    max_chunk: usize,
}

impl ReplayReader {
    fn new(buf: crate::Vec<u8>) -> Self {
        Self {
            buf,
            pos: 0,
            max_chunk: usize::MAX,
        }
    }
}

impl AsyncByteRead for ReplayReader {
    async fn read(&mut self, buf: &mut [u8]) -> core::result::Result<usize, TransportError> {
        if self.pos >= self.buf.len() {
            return Ok(0);
        }
        let n = cmp::min(
            buf.len(),
            cmp::min(self.buf.len() - self.pos, self.max_chunk),
        );
        buf[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

struct CaptureWriter {
    out: crate::Vec<u8>,
    max_chunk: usize,
    flush_called: u32,
}

impl CaptureWriter {
    fn new(max_chunk: usize) -> Self {
        Self {
            out: crate::Vec::new(),
            max_chunk,
            flush_called: 0,
        }
    }
}

impl AsyncByteWrite for CaptureWriter {
    async fn write(&mut self, buf: &[u8]) -> core::result::Result<usize, TransportError> {
        let n = cmp::min(buf.len(), self.max_chunk);
        self.out.extend_from_slice(&buf[..n]);
        Ok(n)
    }

    async fn flush(&mut self) -> core::result::Result<(), TransportError> {
        self.flush_called += 1;
        Ok(())
    }
}

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "transport::length_prefix_roundtrip",
        length_prefix_roundtrip
    );
    shared_test!(
        suite,
        "transport::write_then_read_frame_roundtrip",
        write_then_read_frame_roundtrip,
    );
    shared_test!(
        suite,
        "transport::empty_frame_roundtrips",
        empty_frame_roundtrips,
    );
    shared_test!(
        suite,
        "transport::read_frame_tolerates_trickle_reads",
        read_frame_tolerates_trickle_reads,
    );
    shared_test!(
        suite,
        "transport::write_frame_tolerates_trickle_writes",
        write_frame_tolerates_trickle_writes,
    );
    shared_test!(
        suite,
        "transport::read_frame_clean_eof_returns_none",
        read_frame_clean_eof_returns_none,
    );
    shared_test!(
        suite,
        "transport::read_frame_eof_mid_header_is_lost",
        read_frame_eof_mid_header_is_lost,
    );
    shared_test!(
        suite,
        "transport::read_frame_eof_mid_body_is_lost",
        read_frame_eof_mid_body_is_lost,
    );
    shared_test!(
        suite,
        "transport::read_frame_rejects_oversized_frame",
        read_frame_rejects_oversized_frame,
    );
    shared_test!(
        suite,
        "transport::read_frame_honors_caller_max_len",
        read_frame_honors_caller_max_len,
    );
}

fn length_prefix_roundtrip() -> Result<(), &'static str> {
    for len in [0usize, 1, 0xFF, 0xFFFF, 0x0100_0000, u32::MAX as usize] {
        let header = encode_length_prefix(len);
        crate::check_test!(
            decode_length_prefix(&header) == len,
            "length prefix must roundtrip"
        );
    }
    Ok(())
}

fn write_then_read_frame_roundtrip() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut writer = CaptureWriter::new(usize::MAX);
        let payload = b"hello saikuro";
        write_frame(&mut writer, payload)
            .await
            .map_err(|_| "write must succeed")?;

        let mut reader = ReplayReader::new(writer.out);
        let frame = read_frame(&mut reader, MAX_FRAME_SIZE)
            .await
            .map_err(|_| "read must succeed")?
            .ok_or("a frame must be present")?;
        crate::check_test!(frame[..] == payload[..], "payload must roundtrip verbatim");
        Ok(())
    })
}

fn empty_frame_roundtrips() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut writer = CaptureWriter::new(usize::MAX);
        write_frame(&mut writer, &[])
            .await
            .map_err(|_| "empty write must succeed")?;

        let mut reader = ReplayReader::new(writer.out);
        let frame = read_frame(&mut reader, MAX_FRAME_SIZE)
            .await
            .map_err(|_| "read must succeed")?
            .ok_or("an empty frame must still be a frame")?;
        crate::check_test!(frame.is_empty(), "frame must be empty");
        Ok(())
    })
}

fn read_frame_tolerates_trickle_reads() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut writer = CaptureWriter::new(usize::MAX);
        write_frame(&mut writer, b"trickled")
            .await
            .map_err(|_| "write must succeed")?;
        let mut reader = ReplayReader::new(writer.out);
        reader.max_chunk = 1;
        let frame = read_frame(&mut reader, MAX_FRAME_SIZE)
            .await
            .map_err(|_| "read must succeed with one-byte reads")?
            .ok_or("a frame must be present")?;
        crate::check_test!(
            frame[..] == b"trickled"[..],
            "payload must survive trickle reads"
        );
        Ok(())
    })
}

fn write_frame_tolerates_trickle_writes() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut writer = CaptureWriter::new(2);
        let payload = b"0123456789abcdef";
        write_frame(&mut writer, payload)
            .await
            .map_err(|_| "write must loop over partial writes")?;
        crate::check_test!(
            writer.out.len() == 4 + payload.len(),
            "trickled writer must absorb the full frame"
        );
        let mut reader = ReplayReader::new(writer.out);
        let frame = read_frame(&mut reader, MAX_FRAME_SIZE)
            .await
            .map_err(|_| "read must succeed")?;
        crate::check_test!(
            frame.as_deref() == Some(&payload[..]),
            "trickled-write frame must decode"
        );
        crate::check_test!(writer.flush_called == 1, "flush must be called once");
        Ok(())
    })
}

fn read_frame_clean_eof_returns_none() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut reader = ReplayReader::new(crate::Vec::new());
        let frame = read_frame(&mut reader, MAX_FRAME_SIZE)
            .await
            .map_err(|_| "clean eof must not error")?;
        crate::check_test!(
            frame.is_none(),
            "clean eof at a frame boundary must yield None"
        );
        Ok(())
    })
}

fn read_frame_eof_mid_header_is_lost() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut reader = ReplayReader::new(crate::vec![0u8, 0, 0]);
        let result = read_frame(&mut reader, MAX_FRAME_SIZE).await;
        crate::check_test!(
            matches!(result, Err(TransportError::ConnectionLost(_))),
            "eof inside the header must report ConnectionLost"
        );
        Ok(())
    })
}

fn read_frame_eof_mid_body_is_lost() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut buf = encode_length_prefix(8).to_vec();
        buf.extend_from_slice(&[0u8; 3]);
        let mut reader = ReplayReader::new(buf);
        let result = read_frame(&mut reader, MAX_FRAME_SIZE).await;
        crate::check_test!(
            matches!(result, Err(TransportError::ConnectionLost(_))),
            "eof inside the body must report ConnectionLost"
        );
        Ok(())
    })
}

fn read_frame_rejects_oversized_frame() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut reader = ReplayReader::new(encode_length_prefix(MAX_FRAME_SIZE + 1).to_vec());
        let result = read_frame(&mut reader, MAX_FRAME_SIZE).await;
        crate::check_test!(
            matches!(result, Err(TransportError::MessageTooLarge { .. })),
            "a frame past the persistent 16 MiB limit must be rejected before the body is read"
        );
        Ok(())
    })
}

fn read_frame_honors_caller_max_len() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut buf = encode_length_prefix(64).to_vec();
        buf.extend_from_slice(&[0u8; 64]);
        let mut reader = ReplayReader::new(buf);
        let result = read_frame(&mut reader, 32).await;
        crate::check_test!(
            matches!(
                result,
                Err(TransportError::MessageTooLarge { limit: 32, .. })
            ),
            "a frame over the caller limit must be rejected with that limit"
        );
        Ok(())
    })
}
