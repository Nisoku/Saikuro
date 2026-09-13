//! Response helpers for the provider.

use alloc::string::String;
use alloc::string::ToString;

use bytes::Bytes;
use saikuro_core::envelope::ResponseEnvelope;
use saikuro_core::invocation::InvocationId;
use saikuro_event::{ErrorCode, ErrorDetail, LogLevel, LogRecord, LogSink, Result, SaikuroError};
use saikuro_transport::AdapterTransport;

pub(super) async fn send_response(
    transport: &mut dyn AdapterTransport,
    response: &ResponseEnvelope,
    log: &dyn LogSink,
) {
    match response.to_msgpack() {
        Ok(bytes) => {
            if let Err(e) = transport.send(Bytes::from(bytes)).await {
                let mut record = LogRecord::now(
                    LogLevel::Error,
                    "saikuro.rust.provider",
                    "failed to send response",
                );
                record.set_context("error", alloc::format!("{e}"));
                log.emit(&record).await;
            }
        }
        Err(e) => {
            let mut record = LogRecord::now(
                LogLevel::Error,
                "saikuro.rust.provider",
                "failed to encode response",
            );
            record.set_context("error", alloc::format!("{e}"));
            log.emit(&record).await;
        }
    }
}

pub(super) async fn send_error(
    transport: &mut dyn AdapterTransport,
    id: InvocationId,
    code: ErrorCode,
    message: impl Into<String>,
) {
    let detail = ErrorDetail::new(code, message);
    let response = ResponseEnvelope::err(id, detail);
    let _ = send_response_raw(transport, &response).await;
}

pub(super) async fn send_response_raw(
    transport: &mut dyn AdapterTransport,
    response: &ResponseEnvelope,
) -> Result<()> {
    let bytes = response
        .to_msgpack()
        .map_err(|e| SaikuroError::Serialization(e.to_string()))?;
    transport
        .send(Bytes::from(bytes))
        .await
        .map_err(|e| SaikuroError::SendFailed(e.to_string()))
}
