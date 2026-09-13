//! Dispatch inbound invocations to registered handlers.

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap as HashMap;
#[cfg(feature = "std")]
use std::collections::HashMap;

use alloc::{
    borrow::ToOwned,
    string::{String, ToString},
    vec::Vec,
};

use saikuro_core::envelope::{Envelope, InvocationType, ResponseEnvelope};
use saikuro_event::{
    core_to_json, json_to_core, ErrorCode, LogLevel, LogRecord, LogSink, SaikuroError,
};
use saikuro_transport::AdapterTransport;

use super::handler::HandlerEntry;
use super::send::{send_error, send_response};
use crate::Value;

pub(super) async fn dispatch_call(
    envelope: Envelope,
    handlers: &HashMap<String, HandlerEntry>,
    transport: &mut dyn AdapterTransport,
    log: &dyn LogSink,
) {
    let id = envelope.id;
    let target = envelope.target.clone();

    let fn_name = local_name(&target);
    let entry = match handlers.get(fn_name) {
        Some(e) => e,
        None => {
            send_error(
                transport,
                id,
                ErrorCode::FunctionNotFound,
                format!("no handler registered for '{target}'"),
            )
            .await;
            return;
        }
    };

    let args: Vec<Value> = envelope.args.into_iter().map(core_to_json).collect();
    let handler = entry.handler.clone();

    match handler(args).await {
        Ok(result) => {
            let response = ResponseEnvelope::ok(id, json_to_core(result));
            send_response(transport, &response, log).await;
        }
        Err(SaikuroError::Remote { code, message, .. }) => {
            let error_code = parse_error_code(&code);
            send_error(transport, id, error_code, message).await;
        }
        Err(e) => {
            send_error(transport, id, ErrorCode::ProviderError, e.to_string()).await;
        }
    }
}

pub(super) async fn dispatch_cast(
    envelope: Envelope,
    handlers: &HashMap<String, HandlerEntry>,
    log: &dyn LogSink,
) {
    let fn_name = local_name(&envelope.target);
    let entry = match handlers.get(fn_name) {
        Some(e) => e,
        None => return,
    };

    let args: Vec<Value> = envelope.args.into_iter().map(core_to_json).collect();
    let handler = entry.handler.clone();

    if let Err(e) = handler(args).await {
        let mut record = LogRecord::now(
            LogLevel::Warn,
            "saikuro.rust.provider",
            "cast handler returned error",
        );
        record.set_context("target", envelope.target.clone());
        record.set_context("error", format!("{e}"));
        log.emit(&record).await;
    }
}

pub(super) async fn dispatch_batch(
    envelope: Envelope,
    handlers: &HashMap<String, HandlerEntry>,
    transport: &mut dyn AdapterTransport,
    log: &dyn LogSink,
) {
    use saikuro_event::Value as CoreValue;

    let id = envelope.id;
    let items = match envelope.batch_items {
        Some(items) => items,
        None => {
            send_error(
                transport,
                id,
                ErrorCode::MalformedEnvelope,
                "batch envelope missing batch_items field",
            )
            .await;
            return;
        }
    };

    let mut results: Vec<CoreValue> = Vec::with_capacity(items.len());

    for item in items {
        match item.invocation_type {
            InvocationType::Call => {
                let fn_name = local_name(&item.target).to_owned();
                match handlers.get(&fn_name) {
                    Some(entry) => {
                        let args: Vec<Value> = item.args.into_iter().map(core_to_json).collect();
                        let handler = entry.handler.clone();
                        match handler(args).await {
                            Ok(v) => results.push(json_to_core(v)),
                            Err(e) => {
                                let mut record = LogRecord::now(
                                    LogLevel::Warn,
                                    "saikuro.rust.provider",
                                    "batch item handler error",
                                );
                                record.set_context("target", item.target.clone());
                                record.set_context("error", format!("{e}"));
                                log.emit(&record).await;
                                results.push(CoreValue::Null);
                            }
                        }
                    }
                    None => {
                        results.push(CoreValue::Null);
                    }
                }
            }
            InvocationType::Cast => {
                dispatch_cast(item, handlers, log).await;
                results.push(CoreValue::Null);
            }
            _other => {
                results.push(CoreValue::Null);
            }
        }
    }

    let response = ResponseEnvelope::ok(id, CoreValue::Array(results));
    send_response(transport, &response, log).await;
}

/// Strip the namespace prefix from a fully-qualified function target.
fn local_name(target: &str) -> &str {
    match target.rsplit_once('.') {
        Some((_, name)) => name,
        None => target,
    }
}

/// Convert a provider-supplied error code string to an [`ErrorCode`].
fn parse_error_code(s: &str) -> ErrorCode {
    serde_json::from_value(serde_json::Value::String(s.to_owned())).unwrap_or(ErrorCode::Internal)
}
