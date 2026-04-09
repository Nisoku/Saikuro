use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::ptr;
use std::sync::{Mutex, OnceLock};

use saikuro::{Client, Provider, SaikuroChannel, Value};
use tokio::runtime::Handle;

// TODO: Modularize a bit haha

static LAST_ERROR: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn last_error_slot() -> &'static Mutex<Option<String>> {
    LAST_ERROR.get_or_init(|| Mutex::new(None))
}

fn set_last_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error_slot().lock() {
        *slot = Some(msg.into());
    }
}

fn clear_last_error() {
    if let Ok(mut slot) = last_error_slot().lock() {
        *slot = None;
    }
}

fn cstr_to_string(ptr: *const c_char, arg_name: &str) -> Result<String, String> {
    if ptr.is_null() {
        return Err(format!("{arg_name} must not be null"));
    }
    let s = unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map_err(|_| format!("{arg_name} must be valid UTF-8"))?;
    Ok(s.to_owned())
}

fn into_c_string_ptr(s: &str) -> *mut c_char {
    let sanitized = s.replace('\0', " ");
    match CString::new(sanitized) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

fn parse_json_array_arg(raw: &str, arg_name: &str) -> Result<Vec<Value>, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("{arg_name} must be valid JSON: {e}"))?;
    match parsed {
        serde_json::Value::Array(items) => Ok(items),
        _ => Err(format!("{arg_name} must be a JSON array")),
    }
}

fn parse_batch_calls(raw: &str) -> Result<Vec<(String, Vec<Value>)>, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("calls_json must be valid JSON: {e}"))?;
    let entries = match parsed {
        serde_json::Value::Array(items) => items,
        _ => return Err("calls_json must be a JSON array".to_owned()),
    };

    let mut calls = Vec::with_capacity(entries.len());
    for entry in entries {
        match entry {
            serde_json::Value::Object(obj) => {
                let target = obj
                    .get("target")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "batch call object requires string 'target'".to_owned())?
                    .to_owned();
                let args = match obj.get("args") {
                    Some(serde_json::Value::Array(items)) => items.clone(),
                    _ => return Err("batch call object requires array 'args'".to_owned()),
                };
                calls.push((target, args));
            }
            serde_json::Value::Array(tuple) if tuple.len() == 2 => {
                let target = tuple[0]
                    .as_str()
                    .ok_or_else(|| "batch tuple[0] must be target string".to_owned())?
                    .to_owned();
                let args = match &tuple[1] {
                    serde_json::Value::Array(items) => items.clone(),
                    _ => return Err("batch tuple[1] must be args array".to_owned()),
                };
                calls.push((target, args));
            }
            _ => {
                return Err(
                    "batch calls must be objects {target,args} or [target,args] tuples".to_owned(),
                )
            }
        }
    }

    Ok(calls)
}

fn parse_json_object_arg(
    raw: &str,
    arg_name: &str,
) -> Result<serde_json::Map<String, Value>, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("{arg_name} must be valid JSON: {e}"))?;
    match parsed {
        serde_json::Value::Object(map) => Ok(map),
        _ => Err(format!("{arg_name} must be a JSON object")),
    }
}

struct ClientHandle {
    rt: tokio::runtime::Runtime,
    client: Option<Client>,
}

impl ClientHandle {
    fn new(address: &str) -> Result<Self, String> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("failed to create runtime: {e}"))?;

        let client = rt
            .block_on(Client::connect(address))
            .map_err(|e| format!("failed to connect client: {e}"))?;

        Ok(Self {
            rt,
            client: Some(client),
        })
    }

    fn close(&mut self) -> Result<(), String> {
        if let Some(client) = self.client.take() {
            self.rt
                .block_on(client.close())
                .map_err(|e| format!("failed to close client: {e}"))?;
        }
        Ok(())
    }
}

type ProviderHandler = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char;

struct ProviderHandle {
    rt: tokio::runtime::Runtime,
    provider: Option<Provider>,
}

struct StreamHandle {
    rt: Handle,
    stream: saikuro::SaikuroStream,
}

struct ChannelHandle {
    rt: Handle,
    channel: SaikuroChannel,
}

impl ProviderHandle {
    fn new(namespace: &str) -> Result<Self, String> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("failed to create runtime: {e}"))?;

        Ok(Self {
            rt,
            provider: Some(Provider::new(namespace)),
        })
    }
}

#[no_mangle]
pub extern "C" fn saikuro_string_dup(input: *const c_char) -> *mut c_char {
    match cstr_to_string(input, "input") {
        Ok(s) => into_c_string_ptr(&s),
        Err(e) => {
            set_last_error(e);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
/// # Safety
///
/// `ptr` must either be null or a pointer previously returned by
/// `saikuro_string_dup`, `saikuro_last_error_message`, or another Saikuro C API
/// function that transfers ownership of a heap string to the caller.
pub unsafe extern "C" fn saikuro_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(ptr);
    }
}

#[no_mangle]
pub extern "C" fn saikuro_last_error_message() -> *mut c_char {
    let msg = last_error_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
        .unwrap_or_else(|| "".to_owned());
    into_c_string_ptr(&msg)
}

#[no_mangle]
pub extern "C" fn saikuro_client_connect(address: *const c_char) -> *mut c_void {
    clear_last_error();

    let address = match cstr_to_string(address, "address") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    match ClientHandle::new(&address) {
        Ok(handle) => Box::into_raw(Box::new(handle)) as *mut c_void,
        Err(e) => {
            set_last_error(e);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn saikuro_client_close(handle: *mut c_void) -> c_int {
    clear_last_error();

    if handle.is_null() {
        set_last_error("handle must not be null");
        return 1;
    }

    let handle = unsafe { &mut *(handle as *mut ClientHandle) };
    match handle.close() {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e);
            1
        }
    }
}

#[no_mangle]
pub extern "C" fn saikuro_client_free(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }

    let mut boxed = unsafe { Box::from_raw(handle as *mut ClientHandle) };
    let _ = boxed.close();
}

#[no_mangle]
pub extern "C" fn saikuro_client_call_json(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    if handle.is_null() {
        set_last_error("handle must not be null");
        return ptr::null_mut();
    }

    let target = match cstr_to_string(target, "target") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    let args_json = match cstr_to_string(args_json, "args_json") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    let args_value: serde_json::Value = match serde_json::from_str(&args_json) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("args_json must be valid JSON: {e}"));
            return ptr::null_mut();
        }
    };

    let args: Vec<Value> = match args_value {
        serde_json::Value::Array(items) => items,
        _ => {
            set_last_error("args_json must be a JSON array");
            return ptr::null_mut();
        }
    };

    let handle = unsafe { &mut *(handle as *mut ClientHandle) };
    let client = match handle.client.as_ref() {
        Some(c) => c,
        None => {
            set_last_error("client is already closed");
            return ptr::null_mut();
        }
    };

    let result = handle.rt.block_on(client.call(target, args));
    match result {
        Ok(v) => match serde_json::to_string(&v) {
            Ok(json) => into_c_string_ptr(&json),
            Err(e) => {
                set_last_error(format!("failed to serialize result: {e}"));
                ptr::null_mut()
            }
        },
        Err(e) => {
            set_last_error(format!("call failed: {e}"));
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn saikuro_client_cast_json(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
) -> c_int {
    clear_last_error();

    if handle.is_null() {
        set_last_error("handle must not be null");
        return 1;
    }

    let target = match cstr_to_string(target, "target") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return 1;
        }
    };

    let args_json = match cstr_to_string(args_json, "args_json") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return 1;
        }
    };

    let args: Vec<Value> = match parse_json_array_arg(&args_json, "args_json") {
        Ok(items) => items,
        Err(e) => {
            set_last_error(e);
            return 1;
        }
    };

    let handle = unsafe { &mut *(handle as *mut ClientHandle) };
    let client = match handle.client.as_ref() {
        Some(c) => c,
        None => {
            set_last_error("client is already closed");
            return 1;
        }
    };

    match handle.rt.block_on(client.cast(target, args)) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("cast failed: {e}"));
            1
        }
    }
}

#[no_mangle]
pub extern "C" fn saikuro_client_batch_json(
    handle: *mut c_void,
    calls_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    if handle.is_null() {
        set_last_error("handle must not be null");
        return ptr::null_mut();
    }

    let calls_json = match cstr_to_string(calls_json, "calls_json") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    let calls = match parse_batch_calls(&calls_json) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    let handle = unsafe { &mut *(handle as *mut ClientHandle) };
    let client = match handle.client.as_ref() {
        Some(c) => c,
        None => {
            set_last_error("client is already closed");
            return ptr::null_mut();
        }
    };

    let result = handle.rt.block_on(client.batch(calls));
    match result {
        Ok(v) => match serde_json::to_string(&v) {
            Ok(json) => into_c_string_ptr(&json),
            Err(e) => {
                set_last_error(format!("failed to serialize result: {e}"));
                ptr::null_mut()
            }
        },
        Err(e) => {
            set_last_error(format!("batch failed: {e}"));
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn saikuro_client_stream_json(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
) -> *mut c_void {
    clear_last_error();

    if handle.is_null() {
        set_last_error("handle must not be null");
        return ptr::null_mut();
    }

    let target = match cstr_to_string(target, "target") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    let args_json = match cstr_to_string(args_json, "args_json") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    let args = match parse_json_array_arg(&args_json, "args_json") {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    let handle = unsafe { &mut *(handle as *mut ClientHandle) };
    let rt = handle.rt.handle().clone();
    let client = match handle.client.as_ref() {
        Some(c) => c,
        None => {
            set_last_error("client is already closed");
            return ptr::null_mut();
        }
    };

    let stream = match handle.rt.block_on(client.stream(target, args)) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("stream open failed: {e}"));
            return ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(StreamHandle { rt, stream })) as *mut c_void
}

#[no_mangle]
/// # Safety
///
/// `stream` must be a valid handle returned by `saikuro_client_stream_json`.
/// `out_item_json` and `out_done` must be non-null writable pointers valid for
/// writes for the duration of this call.
pub unsafe extern "C" fn saikuro_stream_next_json(
    stream: *mut c_void,
    out_item_json: *mut *mut c_char,
    out_done: *mut c_int,
) -> c_int {
    clear_last_error();

    if stream.is_null() {
        set_last_error("stream must not be null");
        return 1;
    }
    if out_item_json.is_null() || out_done.is_null() {
        set_last_error("out_item_json and out_done must not be null");
        return 1;
    }

    let stream = unsafe { &mut *(stream as *mut StreamHandle) };
    let next = stream.rt.block_on(stream.stream.next());

    match next {
        Some(Ok(value)) => match serde_json::to_string(&value) {
            Ok(json) => {
                unsafe {
                    *out_done = 0;
                    *out_item_json = into_c_string_ptr(&json);
                }
                0
            }
            Err(e) => {
                set_last_error(format!("failed to serialize stream item: {e}"));
                1
            }
        },
        Some(Err(e)) => {
            set_last_error(format!("stream receive failed: {e}"));
            1
        }
        None => {
            unsafe {
                *out_done = 1;
                *out_item_json = ptr::null_mut();
            }
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn saikuro_stream_free(stream: *mut c_void) {
    if stream.is_null() {
        return;
    }
    let _ = unsafe { Box::from_raw(stream as *mut StreamHandle) };
}

#[no_mangle]
pub extern "C" fn saikuro_client_channel_json(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
) -> *mut c_void {
    clear_last_error();

    if handle.is_null() {
        set_last_error("handle must not be null");
        return ptr::null_mut();
    }

    let target = match cstr_to_string(target, "target") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    let args_json = match cstr_to_string(args_json, "args_json") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    let args = match parse_json_array_arg(&args_json, "args_json") {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    let handle = unsafe { &mut *(handle as *mut ClientHandle) };
    let rt = handle.rt.handle().clone();
    let client = match handle.client.as_ref() {
        Some(c) => c,
        None => {
            set_last_error("client is already closed");
            return ptr::null_mut();
        }
    };

    let channel = match handle.rt.block_on(client.channel(target, args)) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("channel open failed: {e}"));
            return ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(ChannelHandle { rt, channel })) as *mut c_void
}

#[no_mangle]
pub extern "C" fn saikuro_channel_send_json(
    channel: *mut c_void,
    item_json: *const c_char,
) -> c_int {
    clear_last_error();

    if channel.is_null() {
        set_last_error("channel must not be null");
        return 1;
    }

    let item_json = match cstr_to_string(item_json, "item_json") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return 1;
        }
    };

    let item: Value = match serde_json::from_str(&item_json) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("item_json must be valid JSON: {e}"));
            return 1;
        }
    };

    let channel = unsafe { &mut *(channel as *mut ChannelHandle) };
    match channel.rt.block_on(channel.channel.send(item)) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("channel send failed: {e}"));
            1
        }
    }
}

#[no_mangle]
/// # Safety
///
/// `channel` must be a valid handle returned by `saikuro_client_channel_json`.
/// `out_item_json` and `out_done` must be non-null writable pointers valid for
/// writes for the duration of this call.
pub unsafe extern "C" fn saikuro_channel_next_json(
    channel: *mut c_void,
    out_item_json: *mut *mut c_char,
    out_done: *mut c_int,
) -> c_int {
    clear_last_error();

    if channel.is_null() {
        set_last_error("channel must not be null");
        return 1;
    }
    if out_item_json.is_null() || out_done.is_null() {
        set_last_error("out_item_json and out_done must not be null");
        return 1;
    }

    let channel = unsafe { &mut *(channel as *mut ChannelHandle) };
    let next = channel.rt.block_on(channel.channel.next());

    match next {
        Some(Ok(value)) => match serde_json::to_string(&value) {
            Ok(json) => {
                unsafe {
                    *out_done = 0;
                    *out_item_json = into_c_string_ptr(&json);
                }
                0
            }
            Err(e) => {
                set_last_error(format!("failed to serialize channel item: {e}"));
                1
            }
        },
        Some(Err(e)) => {
            set_last_error(format!("channel receive failed: {e}"));
            1
        }
        None => {
            unsafe {
                *out_done = 1;
                *out_item_json = ptr::null_mut();
            }
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn saikuro_channel_free(channel: *mut c_void) {
    if channel.is_null() {
        return;
    }
    let _ = unsafe { Box::from_raw(channel as *mut ChannelHandle) };
}

#[no_mangle]
pub extern "C" fn saikuro_client_resource_json(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    if handle.is_null() {
        set_last_error("handle must not be null");
        return ptr::null_mut();
    }

    let target = match cstr_to_string(target, "target") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    let args_json = match cstr_to_string(args_json, "args_json") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    let args = match parse_json_array_arg(&args_json, "args_json") {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    let handle = unsafe { &mut *(handle as *mut ClientHandle) };
    let client = match handle.client.as_ref() {
        Some(c) => c,
        None => {
            set_last_error("client is already closed");
            return ptr::null_mut();
        }
    };

    let result = handle.rt.block_on(client.resource(target, args));
    match result {
        Ok(v) => match serde_json::to_string(&v) {
            Ok(json) => into_c_string_ptr(&json),
            Err(e) => {
                set_last_error(format!("failed to serialize resource result: {e}"));
                ptr::null_mut()
            }
        },
        Err(e) => {
            set_last_error(format!("resource call failed: {e}"));
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn saikuro_client_log(
    handle: *mut c_void,
    level: *const c_char,
    name: *const c_char,
    msg: *const c_char,
    fields_json: *const c_char,
) -> c_int {
    clear_last_error();

    if handle.is_null() {
        set_last_error("handle must not be null");
        return 1;
    }

    let level = match cstr_to_string(level, "level") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return 1;
        }
    };
    let name = match cstr_to_string(name, "name") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return 1;
        }
    };
    let msg = match cstr_to_string(msg, "msg") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return 1;
        }
    };

    let fields = if fields_json.is_null() {
        None
    } else {
        let raw = match cstr_to_string(fields_json, "fields_json") {
            Ok(s) => s,
            Err(e) => {
                set_last_error(e);
                return 1;
            }
        };
        match parse_json_object_arg(&raw, "fields_json") {
            Ok(map) => Some(Value::Object(map)),
            Err(e) => {
                set_last_error(e);
                return 1;
            }
        }
    };

    let handle = unsafe { &mut *(handle as *mut ClientHandle) };
    let client = match handle.client.as_ref() {
        Some(c) => c,
        None => {
            set_last_error("client is already closed");
            return 1;
        }
    };

    match handle.rt.block_on(client.log(level, name, msg, fields)) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("log send failed: {e}"));
            1
        }
    }
}

#[no_mangle]
pub extern "C" fn saikuro_provider_new(namespace: *const c_char) -> *mut c_void {
    clear_last_error();

    let namespace = match cstr_to_string(namespace, "namespace") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };

    match ProviderHandle::new(&namespace) {
        Ok(handle) => Box::into_raw(Box::new(handle)) as *mut c_void,
        Err(e) => {
            set_last_error(e);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn saikuro_provider_register(
    handle: *mut c_void,
    name: *const c_char,
    callback: Option<ProviderHandler>,
    user_data: *mut c_void,
) -> c_int {
    clear_last_error();

    if handle.is_null() {
        set_last_error("handle must not be null");
        return 1;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => {
            set_last_error("callback must not be null");
            return 1;
        }
    };

    let name = match cstr_to_string(name, "name") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return 1;
        }
    };

    let handle = unsafe { &mut *(handle as *mut ProviderHandle) };
    let provider = match handle.provider.as_mut() {
        Some(p) => p,
        None => {
            set_last_error("provider has already started serving");
            return 1;
        }
    };

    let user_data_addr = user_data as usize;

    provider.register(name, move |args: Vec<Value>| {
        let func = callback;
        let user_data = user_data_addr;
        async move {
            let args_json = serde_json::to_string(&args)
                .map_err(|e| saikuro::Error::InvalidState(format!("args encode failed: {e}")))?;
            let args_c = CString::new(args_json)
                .map_err(|_| saikuro::Error::InvalidState("args contain NUL byte".to_owned()))?;

            let result_ptr = unsafe { (func)(user_data as *mut c_void, args_c.as_ptr()) };
            if result_ptr.is_null() {
                return Err(saikuro::Error::InvalidState(
                    "C handler returned null".to_owned(),
                ));
            }

            let result_owned = unsafe { CString::from_raw(result_ptr) };
            let result_str = result_owned
                .to_str()
                .map_err(|_| {
                    saikuro::Error::InvalidState("C handler returned non-UTF8".to_owned())
                })?
                .to_owned();

            let value: Value = serde_json::from_str(&result_str).map_err(|e| {
                saikuro::Error::InvalidState(format!("C handler returned invalid JSON: {e}"))
            })?;

            Ok(value)
        }
    });

    0
}

#[no_mangle]
pub extern "C" fn saikuro_provider_serve(handle: *mut c_void, address: *const c_char) -> c_int {
    clear_last_error();

    if handle.is_null() {
        set_last_error("handle must not be null");
        return 1;
    }

    let address = match cstr_to_string(address, "address") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return 1;
        }
    };

    let handle = unsafe { &mut *(handle as *mut ProviderHandle) };
    let provider = match handle.provider.take() {
        Some(p) => p,
        None => {
            set_last_error("provider has already started serving");
            return 1;
        }
    };

    match handle.rt.block_on(provider.serve(address)) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("provider serve failed: {e}"));
            1
        }
    }
}

#[no_mangle]
pub extern "C" fn saikuro_provider_free(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }

    let _ = unsafe { Box::from_raw(handle as *mut ProviderHandle) };
}
