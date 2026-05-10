use std::cell::RefCell;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::ptr;
use std::thread_local;
use std::time::Duration;

use saikuro::{Client, Provider, SaikuroChannel, Value};
use saikuro_exec::Runtime;
use std::sync::Arc;

// TODO: Modularize a bit haha

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn set_last_error(msg: impl Into<String>) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(msg.into());
    });
}

fn clear_last_error() {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = None;
    });
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

//  C API helpers factor out the null-check / cast / error pattern

macro_rules! ok_or_ptr {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => {
                set_last_error(e);
                return ptr::null_mut();
            }
        }
    };
}

macro_rules! ok_or_int {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => {
                set_last_error(e);
                return 1;
            }
        }
    };
}

/// Parse a JSON array from a C string pointer.
fn c_json_array(ptr: *const c_char) -> Result<Vec<Value>, String> {
    let s = cstr_to_string(ptr, "args_json")?;
    parse_json_array_arg(&s, "args_json")
}

/// Validate and dereference a client handle.
fn client_handle(h: *mut c_void) -> Result<&'static mut ClientHandle, String> {
    if h.is_null() {
        return Err("handle must not be null".to_owned());
    }
    let h = unsafe { &mut *(h as *mut ClientHandle) };
    if h.client.is_none() {
        return Err("client is already closed".to_owned());
    }
    Ok(h)
}

/// Serialise a `saikuro::Result<Value>` into a heap-allocated C string pointer,
/// or set `last_error` and return null on failure.
fn ptr_saikuro(result: Result<Value, saikuro::Error>, op: &str) -> *mut c_char {
    match result {
        Ok(v) => match serde_json::to_string(&v) {
            Ok(json) => into_c_string_ptr(&json),
            Err(e) => {
                set_last_error(format!("failed to serialize result: {e}"));
                ptr::null_mut()
            }
        },
        Err(e) => {
            set_last_error(format!("{op} failed: {e}"));
            ptr::null_mut()
        }
    }
}

/// Map a `saikuro::Result<()>` to a C `c_int` return, setting `last_error` on failure.
fn int_saikuro(result: Result<(), saikuro::Error>, op: &str) -> c_int {
    match result {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("{op} failed: {e}"));
            1
        }
    }
}

struct ClientHandle {
    rt: Arc<Runtime>,
    client: Option<Client>,
}

impl ClientHandle {
    fn new(address: &str) -> Result<Self, String> {
        let rt = Arc::new(
            saikuro_exec::new_runtime()
                .enable_all()
                .build()
                .map_err(|e| format!("failed to create runtime: {e}"))?,
        );

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

/// C callback for provider functions.
///
/// # Safety
/// The returned pointer must be an owned C string allocated via `saikuro_string_dup`
/// (or `CString::into_raw`-compatible allocation semantics).
/// Ownership is transferred to Rust, which reclaims it with `CString::from_raw`.
/// Returning strings from `malloc`/`strdup` is undefined behavior because allocator
/// ownership does not match `CString::from_raw` expectations.
type ProviderHandler = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char;

struct ProviderHandle {
    rt: saikuro_exec::Runtime,
    provider: Option<Provider>,
}

struct StreamHandle {
    rt: Arc<Runtime>,
    stream: saikuro::SaikuroStream,
}

struct ChannelHandle {
    rt: Arc<Runtime>,
    channel: SaikuroChannel,
}

impl ProviderHandle {
    fn new(namespace: &str) -> Result<Self, String> {
        let rt = saikuro_exec::new_runtime()
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

/// Frees a heap-allocated string returned by the Saikuro C API.
///
/// # Safety
///
/// `ptr` must be either null or a pointer previously returned by
/// [`saikuro_string_dup`], [`saikuro_last_error_message`], or another Saikuro C API function
/// that transfers ownership of a heap string to the caller. Passing any other pointer,
/// or a pointer not obtained from Saikuro, results in undefined behavior.
#[no_mangle]
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
    let msg = LAST_ERROR
        .with(|cell| cell.borrow().clone())
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
    match unsafe { &mut *(handle as *mut ClientHandle) }.close() {
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
    let h = ok_or_ptr!(client_handle(handle));
    let target = ok_or_ptr!(cstr_to_string(target, "target"));
    let args = ok_or_ptr!(c_json_array(args_json));
    ptr_saikuro(
        h.rt.block_on(h.client.as_ref().unwrap().call(target, args)),
        "call",
    )
}

#[no_mangle]
pub extern "C" fn saikuro_client_call_json_timeout(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
    timeout_ms: c_int,
) -> *mut c_char {
    clear_last_error();
    let h = ok_or_ptr!(client_handle(handle));
    if timeout_ms < 0 {
        set_last_error("timeout_ms must be non-negative");
        return ptr::null_mut();
    }
    let target = ok_or_ptr!(cstr_to_string(target, "target"));
    let args = ok_or_ptr!(c_json_array(args_json));
    let timeout = Duration::from_millis(timeout_ms as u64);
    ptr_saikuro(
        h.rt.block_on(
            h.client
                .as_ref()
                .unwrap()
                .call_with_timeout(target, args, Some(timeout)),
        ),
        "call",
    )
}

#[no_mangle]
pub extern "C" fn saikuro_client_cast_json(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
) -> c_int {
    clear_last_error();
    let h = ok_or_int!(client_handle(handle));
    let target = ok_or_int!(cstr_to_string(target, "target"));
    let args = ok_or_int!(c_json_array(args_json));
    int_saikuro(
        h.rt.block_on(h.client.as_ref().unwrap().cast(target, args)),
        "cast",
    )
}

#[no_mangle]
pub extern "C" fn saikuro_client_batch_json(
    handle: *mut c_void,
    calls_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = ok_or_ptr!(client_handle(handle));
    let raw = ok_or_ptr!(cstr_to_string(calls_json, "calls_json"));
    let calls = ok_or_ptr!(parse_batch_calls(&raw));
    match h.rt.block_on(h.client.as_ref().unwrap().batch(calls)) {
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
    let h = ok_or_ptr!(client_handle(handle));
    let target = ok_or_ptr!(cstr_to_string(target, "target"));
    let args = ok_or_ptr!(c_json_array(args_json));
    let rt = h.rt.clone();
    let stream = match h
        .rt
        .block_on(h.client.as_ref().unwrap().stream(target, args))
    {
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

    unsafe {
        if !out_done.is_null() {
            *out_done = 1;
        }
        if !out_item_json.is_null() {
            *out_item_json = ptr::null_mut();
        }
    }

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
                unsafe {
                    *out_done = 1;
                    *out_item_json = ptr::null_mut();
                }
                set_last_error(format!("failed to serialize stream item: {e}"));
                1
            }
        },
        Some(Err(e)) => {
            unsafe {
                *out_done = 1;
                *out_item_json = ptr::null_mut();
            }
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
    let h = ok_or_ptr!(client_handle(handle));
    let target = ok_or_ptr!(cstr_to_string(target, "target"));
    let args = ok_or_ptr!(c_json_array(args_json));
    let rt = h.rt.clone();
    let channel = match h
        .rt
        .block_on(h.client.as_ref().unwrap().channel(target, args))
    {
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
pub extern "C" fn saikuro_channel_close(channel: *mut c_void) -> c_int {
    clear_last_error();

    if channel.is_null() {
        set_last_error("channel must not be null");
        return 1;
    }

    let channel = unsafe { &mut *(channel as *mut ChannelHandle) };
    match channel.rt.block_on(channel.channel.close()) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("channel close failed: {e}"));
            1
        }
    }
}

#[no_mangle]
pub extern "C" fn saikuro_channel_abort(channel: *mut c_void) -> c_int {
    clear_last_error();

    if channel.is_null() {
        set_last_error("channel must not be null");
        return 1;
    }

    let channel = unsafe { &mut *(channel as *mut ChannelHandle) };
    match channel.rt.block_on(channel.channel.abort()) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("channel abort failed: {e}"));
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

    unsafe {
        if !out_done.is_null() {
            *out_done = 1;
        }
        if !out_item_json.is_null() {
            *out_item_json = ptr::null_mut();
        }
    }

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
                unsafe {
                    *out_done = 1;
                    *out_item_json = ptr::null_mut();
                }
                set_last_error(format!("failed to serialize channel item: {e}"));
                1
            }
        },
        Some(Err(e)) => {
            unsafe {
                *out_done = 1;
                *out_item_json = ptr::null_mut();
            }
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
    let h = ok_or_ptr!(client_handle(handle));
    let target = ok_or_ptr!(cstr_to_string(target, "target"));
    let args = ok_or_ptr!(c_json_array(args_json));
    ptr_saikuro(
        h.rt.block_on(h.client.as_ref().unwrap().resource(target, args)),
        "resource",
    )
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
    let h = ok_or_int!(client_handle(handle));
    let level = ok_or_int!(cstr_to_string(level, "level"));
    let name = ok_or_int!(cstr_to_string(name, "name"));
    let msg = ok_or_int!(cstr_to_string(msg, "msg"));
    let fields = if fields_json.is_null() {
        None
    } else {
        let raw = ok_or_int!(cstr_to_string(fields_json, "fields_json"));
        match parse_json_object_arg(&raw, "fields_json") {
            Ok(map) => Some(Value::Object(map)),
            Err(e) => {
                set_last_error(e);
                return 1;
            }
        }
    };
    int_saikuro(
        h.rt.block_on(h.client.as_ref().unwrap().log(level, name, msg, fields)),
        "log",
    )
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

/// Safety: The `user_data` pointer is captured and later used inside
/// asynchronous callbacks registered with the provider. Callers must ensure
/// that the `user_data` pointer remains valid for the entire lifetime of the
/// registered provider (i.e., until `saikuro_provider_free` is called). If
/// `user_data` is freed or becomes dangling while the provider remains
/// registered, subsequent callback invocations will dereference invalid
/// memory and cause undefined behavior.
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
