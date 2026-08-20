#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(all(
    not(feature = "std"),
    not(feature = "native"),
    any(target_os = "none", all(target_os = "wasi", target_env = "p1"),),
))]
mod embedded_rt {
    use core::alloc::{GlobalAlloc, Layout};

    struct StubAllocator;

    unsafe impl GlobalAlloc for StubAllocator {
        unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
            core::ptr::null_mut()
        }
        unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
    }

    #[global_allocator]
    static ALLOCATOR: StubAllocator = StubAllocator;

    #[panic_handler]
    fn panic(_info: &core::panic::PanicInfo) -> ! {
        loop {}
    }
}

#[cfg(not(feature = "std"))]
use alloc::borrow::ToOwned;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
use alloc::ffi::CString;
#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::ffi::{c_char, c_int, c_void, CStr};
use core::future::Future;
use core::ptr;

use saikuro::{
    ArgDescriptor, FunctionSchema, PrimitiveType, Provider, RegisterOptions, TypeDescriptor, Value,
};
#[cfg(feature = "std")]
use saikuro::{Client, SaikuroChannel, SaikuroStream};

// C API helpers for client handle validation and result serialization.

const ERR_HANDLE_NULL: &str = "handle must not be null";

// Last-error slot.
#[cfg(feature = "std")]
static LAST_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[cfg(not(feature = "std"))]
static LAST_ERROR: spin::Mutex<Option<String>> = spin::Mutex::new(None);

fn set_last_error(msg: impl Into<String>) {
    #[cfg(feature = "std")]
    {
        *LAST_ERROR.lock().expect("last-error lock poisoned") = Some(msg.into());
    }
    #[cfg(not(feature = "std"))]
    {
        *LAST_ERROR.lock() = Some(msg.into());
    }
}

fn clear_last_error() {
    #[cfg(feature = "std")]
    {
        *LAST_ERROR.lock().expect("last-error lock poisoned") = None;
    }
    #[cfg(not(feature = "std"))]
    {
        *LAST_ERROR.lock() = None;
    }
}

fn last_error_string() -> String {
    #[cfg(feature = "std")]
    {
        LAST_ERROR
            .lock()
            .expect("last-error lock poisoned")
            .clone()
            .unwrap_or_default()
    }
    #[cfg(not(feature = "std"))]
    {
        LAST_ERROR.lock().clone().unwrap_or_default()
    }
}

#[cfg(feature = "native")]
mod exec {
    use core::future::Future;
    use std::sync::OnceLock;
    use tokio::runtime::Runtime as TokioRuntime;

    static RT: OnceLock<TokioRuntime> = OnceLock::new();

    pub(super) fn spawn<F>(fut: F)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let rt = RT
            .get_or_init(|| TokioRuntime::new().expect("saikuro-c: failed to start tokio runtime"));
        rt.handle().spawn(fut);
    }

    pub(super) fn block_on<F: Future>(fut: F) -> F::Output {
        let rt = RT
            .get_or_init(|| TokioRuntime::new().expect("saikuro-c: failed to start tokio runtime"));
        rt.block_on(fut)
    }
}

#[cfg(feature = "native")]
fn spawn_future<F>(fut: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    exec::spawn(fut);
}

#[cfg(feature = "native")]
fn block_on_future<F: Future>(fut: F) -> F::Output {
    exec::block_on(fut)
}

#[cfg(not(feature = "native"))]
fn spawn_future<F>(fut: F)
where
    F: Future<Output = ()> + 'static,
{
    saikuro_exec::spawn(fut);
}

// C callback signatures.

/// Called with the created handle (or null on error) once a connect / stream /
/// channel open completes.
pub type SaikuroConnectCb = extern "C" fn(*mut c_void, *mut c_void);

/// Called with the serialised result (or null on error) once an RPC completes.
pub type SaikuroResultCb = extern "C" fn(*mut c_char, *mut c_void);

/// Called with a status code (0 = ok, 1 = error) once a fire-and-forget op
/// (cast / log / close / abort / serve) completes.
pub type SaikuroStatusCb = extern "C" fn(c_int, *mut c_void);

/// Called with the next stream/channel item. `item` is null when the stream is
/// exhausted or an error occurred (see `saikuro_last_error_message`). `done` is
/// 0 when `item` holds a value, 1 otherwise.
pub type SaikuroItemCb = extern "C" fn(*mut c_char, c_int, *mut c_void);

// Handles.

#[cfg(feature = "std")]
struct ClientHandle {
    client: Option<Client>,
}

#[cfg(feature = "std")]
impl ClientHandle {
    fn client(&self) -> &Client {
        self.client.as_ref().expect("client already closed")
    }
}

#[cfg(feature = "std")]
struct StreamHandle {
    stream: SaikuroStream,
}

#[cfg(feature = "std")]
struct ChannelHandle {
    channel: SaikuroChannel,
}

/// C callback for provider functions.
///
/// # Safety
/// The returned pointer must be an owned C string allocated via
/// `saikuro_string_dup` (or `CString::into_raw`-compatible allocation). Ownership
/// is transferred to Rust, which reclaims it with `CString::from_raw`. Returning
/// strings from `malloc`/`strdup` is undefined behavior because allocator
/// ownership does not match `CString::from_raw` expectations.
type ProviderHandler = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char;

struct ProviderHandle {
    provider: Option<Provider>,
}

// Parsing / serialisation helpers.

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

#[cfg(feature = "std")]
fn parse_json_array_arg(raw: &str, arg_name: &str) -> Result<Vec<Value>, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("{arg_name} must be valid JSON: {e}"))?;
    match parsed {
        serde_json::Value::Array(items) => Ok(items),
        _ => Err(format!("{arg_name} must be a JSON array")),
    }
}

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
fn c_json_array(ptr: *const c_char) -> Result<Vec<Value>, String> {
    let s = cstr_to_string(ptr, "args_json")?;
    parse_json_array_arg(&s, "args_json")
}

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
fn int_saikuro(result: Result<(), saikuro::Error>, op: &str) -> c_int {
    match result {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("{op} failed: {e}"));
            1
        }
    }
}

// Lifetime-safe handle accessors for spawned futures.
#[cfg(feature = "std")]
fn client_ref(h: *mut c_void) -> &'static ClientHandle {
    unsafe { &*(h as *const ClientHandle) }
}

// String lifecycle

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

/// # Safety
/// `ptr` must be either null or a pointer previously returned by
/// [`saikuro_string_dup`], [`saikuro_last_error_message`], or another Saikuro C
/// API function that transfers ownership of a heap string to the caller.
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
    let msg = last_error_string();
    into_c_string_ptr(&msg)
}

// Client lifecycle (async).

/// # Safety
/// `cb` must not be null.
#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_client_connect_async(
    address: *const c_char,
    cb: Option<SaikuroConnectCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    let address = match cstr_to_string(address, "address") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return;
        }
    };

    let handle = Box::into_raw(Box::new(ClientHandle { client: None }));
    let handle_addr = handle as usize;
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        match Client::connect(address).await {
            Ok(client) => {
                let handle = handle_addr as *mut ClientHandle;
                unsafe {
                    (*handle).client = Some(client);
                }
                cb(handle as *mut c_void, user_data_addr as *mut c_void);
            }
            Err(e) => {
                set_last_error(format!("failed to connect client: {e}"));
                unsafe {
                    let _ = Box::from_raw(handle_addr as *mut ClientHandle);
                }
                cb(ptr::null_mut(), user_data_addr as *mut c_void);
            }
        }
    });
}

#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_client_free(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    let _ = unsafe { Box::from_raw(handle as *mut ClientHandle) };
}

/// # Safety
/// `cb` must not be null. The handle must not be freed while a close is in flight.
#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_client_close_async(
    handle: *mut c_void,
    cb: Option<SaikuroStatusCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        cb(1, user_data);
        return;
    }

    let handle_ref = unsafe { &mut *(handle as *mut ClientHandle) };
    let client = handle_ref.client.take();
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let status = match client {
            Some(client) => int_saikuro(client.close().await, "close"),
            None => 0,
        };
        cb(status, user_data_addr as *mut c_void);
    });
}

// Client RPC (async).

/// # Safety
/// `cb` must not be null. The client handle must remain valid until `cb` fires.
#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_client_call_json_async(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
    cb: Option<SaikuroResultCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        cb(ptr::null_mut(), user_data);
        return;
    }
    let h = match cstr_to_string(target, "target")
        .and_then(|t| c_json_array(args_json).map(|a| (t, a)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return;
        }
    };
    let (target, args) = h;

    let handle_addr = handle as usize;
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let handle = handle_addr as *mut c_void;
        let h = client_ref(handle);
        let res = h.client().call(target, args).await;
        let out = ptr_saikuro(res, "call");
        cb(out, user_data_addr as *mut c_void);
    });
}

/// # Safety
/// `cb` must not be null. The client handle must remain valid until `cb` fires.
#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_client_call_json_timeout_async(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
    timeout_ms: c_int,
    cb: Option<SaikuroResultCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        cb(ptr::null_mut(), user_data);
        return;
    }
    if timeout_ms < 0 {
        set_last_error("timeout_ms must be non-negative");
        return;
    }
    let parsed = match cstr_to_string(target, "target")
        .and_then(|t| c_json_array(args_json).map(|a| (t, a)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return;
        }
    };
    let (target, args) = parsed;
    let timeout = core::time::Duration::from_millis(timeout_ms as u64);

    let handle_addr = handle as usize;
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let handle = handle_addr as *mut c_void;
        let h = client_ref(handle);
        let res = h
            .client()
            .call_with_timeout(target, args, Some(timeout))
            .await;
        let out = ptr_saikuro(res, "call");
        cb(out, user_data_addr as *mut c_void);
    });
}

/// # Safety
/// `cb` must not be null. The client handle must remain valid until `cb` fires.
#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_client_cast_json_async(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
    cb: Option<SaikuroStatusCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        cb(1, user_data);
        return;
    }
    let parsed = match cstr_to_string(target, "target")
        .and_then(|t| c_json_array(args_json).map(|a| (t, a)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            cb(1, user_data);
            return;
        }
    };
    let (target, args) = parsed;

    let handle_addr = handle as usize;
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let handle = handle_addr as *mut c_void;
        let h = client_ref(handle);
        let res = h.client().cast(target, args).await;
        cb(int_saikuro(res, "cast"), user_data_addr as *mut c_void);
    });
}

/// # Safety
/// `cb` must not be null. The client handle must remain valid until `cb` fires.
#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_client_batch_json_async(
    handle: *mut c_void,
    calls_json: *const c_char,
    cb: Option<SaikuroResultCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        cb(ptr::null_mut(), user_data);
        return;
    }
    let raw = match cstr_to_string(calls_json, "calls_json") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return;
        }
    };
    let calls = match parse_batch_calls(&raw) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(e);
            return;
        }
    };

    let handle_addr = handle as usize;
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let handle = handle_addr as *mut c_void;
        let h = client_ref(handle);
        match h.client().batch(calls).await {
            Ok(v) => match serde_json::to_string(&v) {
                Ok(json) => cb(into_c_string_ptr(&json), user_data_addr as *mut c_void),
                Err(e) => {
                    set_last_error(format!("failed to serialize result: {e}"));
                    cb(ptr::null_mut(), user_data_addr as *mut c_void);
                }
            },
            Err(e) => {
                set_last_error(format!("batch failed: {e}"));
                cb(ptr::null_mut(), user_data_addr as *mut c_void);
            }
        }
    });
}

/// # Safety
/// `cb` must not be null. The client handle must remain valid until `cb` fires.
#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_client_resource_json_async(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
    cb: Option<SaikuroResultCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        cb(ptr::null_mut(), user_data);
        return;
    }
    let parsed = match cstr_to_string(target, "target")
        .and_then(|t| c_json_array(args_json).map(|a| (t, a)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return;
        }
    };
    let (target, args) = parsed;

    let handle_addr = handle as usize;
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let handle = handle_addr as *mut c_void;
        let h = client_ref(handle);
        let res = h.client().resource(target, args).await;
        let out = ptr_saikuro(res, "resource");
        cb(out, user_data_addr as *mut c_void);
    });
}

/// # Safety
/// `cb` must not be null. The client handle must remain valid until `cb` fires.
#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_client_log_async(
    handle: *mut c_void,
    level: *const c_char,
    name: *const c_char,
    msg: *const c_char,
    fields_json: *const c_char,
    cb: Option<SaikuroStatusCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        cb(1, user_data);
        return;
    }
    let parsed = match cstr_to_string(level, "level")
        .and_then(|l| cstr_to_string(name, "name").map(|n| (l, n)))
        .and_then(|(l, n)| cstr_to_string(msg, "msg").map(|m| (l, n, m)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            cb(1, user_data);
            return;
        }
    };
    let (level, name, msg) = parsed;
    let fields = if fields_json.is_null() {
        None
    } else {
        match cstr_to_string(fields_json, "fields_json")
            .and_then(|raw| parse_json_object_arg(&raw, "fields_json").map(Value::Object))
        {
            Ok(v) => Some(v),
            Err(e) => {
                set_last_error(e);
                cb(1, user_data);
                return;
            }
        }
    };

    let handle_addr = handle as usize;
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let handle = handle_addr as *mut c_void;
        let h = client_ref(handle);
        let res = h.client().log(level, name, msg, fields).await;
        cb(int_saikuro(res, "log"), user_data_addr as *mut c_void);
    });
}

// Streams (async open + async next).

/// # Safety
/// `cb` must not be null. The client handle must remain valid until `cb` fires.
#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_client_stream_json_async(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
    cb: Option<SaikuroConnectCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        cb(ptr::null_mut(), user_data);
        return;
    }
    let parsed = match cstr_to_string(target, "target")
        .and_then(|t| c_json_array(args_json).map(|a| (t, a)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return;
        }
    };
    let (target, args) = parsed;

    let handle_addr = handle as usize;
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let handle = handle_addr as *mut c_void;
        let h = client_ref(handle);
        match h.client().stream(target, args).await {
            Ok(stream) => {
                let sh = Box::into_raw(Box::new(StreamHandle { stream }));
                cb(sh as *mut c_void, user_data_addr as *mut c_void);
            }
            Err(e) => {
                set_last_error(format!("stream open failed: {e}"));
                cb(ptr::null_mut(), user_data_addr as *mut c_void);
            }
        }
    });
}

#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_stream_free(stream: *mut c_void) {
    if stream.is_null() {
        return;
    }
    let _ = unsafe { Box::from_raw(stream as *mut StreamHandle) };
}

/// # Safety
/// `cb` must not be null. The stream handle must remain valid until `cb` fires,
/// and `saikuro_stream_next_json_async` must not be called concurrently on the
/// same stream.
#[cfg(feature = "std")]
#[no_mangle]
pub unsafe extern "C" fn saikuro_stream_next_json_async(
    stream: *mut c_void,
    cb: Option<SaikuroItemCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if stream.is_null() {
        set_last_error("stream must not be null");
        return;
    }

    let stream_addr = stream as usize;
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let stream = stream_addr as *mut StreamHandle;
        let s = unsafe { &mut *stream };
        match s.stream.next().await {
            Some(Ok(value)) => match serde_json::to_string(&value) {
                Ok(json) => cb(into_c_string_ptr(&json), 0, user_data_addr as *mut c_void),
                Err(e) => {
                    set_last_error(format!("failed to serialize stream item: {e}"));
                    cb(ptr::null_mut(), 1, user_data_addr as *mut c_void);
                }
            },
            Some(Err(e)) => {
                set_last_error(format!("stream receive failed: {e}"));
                cb(ptr::null_mut(), 1, user_data_addr as *mut c_void);
            }
            None => cb(ptr::null_mut(), 1, user_data_addr as *mut c_void),
        }
    });
}

// Channels (async open + async send/next).

/// # Safety
/// `cb` must not be null. The client handle must remain valid until `cb` fires.
#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_client_channel_json_async(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
    cb: Option<SaikuroConnectCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        cb(ptr::null_mut(), user_data);
        return;
    }
    let parsed = match cstr_to_string(target, "target")
        .and_then(|t| c_json_array(args_json).map(|a| (t, a)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return;
        }
    };
    let (target, args) = parsed;

    let handle_addr = handle as usize;
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let handle = handle_addr as *mut c_void;
        let h = client_ref(handle);
        match h.client().channel(target, args).await {
            Ok(channel) => {
                let ch = Box::into_raw(Box::new(ChannelHandle { channel }));
                cb(ch as *mut c_void, user_data_addr as *mut c_void);
            }
            Err(e) => {
                set_last_error(format!("channel open failed: {e}"));
                cb(ptr::null_mut(), user_data_addr as *mut c_void);
            }
        }
    });
}

/// # Safety
/// `cb` must not be null. The channel handle must remain valid until `cb` fires.
#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_channel_send_json_async(
    channel: *mut c_void,
    item_json: *const c_char,
    cb: Option<SaikuroStatusCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if channel.is_null() {
        set_last_error("channel must not be null");
        cb(1, user_data);
        return;
    }
    let item_json = match cstr_to_string(item_json, "item_json") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            cb(1, user_data);
            return;
        }
    };
    let item: Value = match serde_json::from_str(&item_json) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("item_json must be valid JSON: {e}"));
            cb(1, user_data);
            return;
        }
    };

    let channel_addr = channel as usize;
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let channel = channel_addr as *mut ChannelHandle;
        let c = unsafe { &mut *channel };
        let res = c.channel.send(item).await;
        cb(
            int_saikuro(res, "channel send"),
            user_data_addr as *mut c_void,
        );
    });
}

#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_channel_close_async(
    channel: *mut c_void,
    cb: Option<SaikuroStatusCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if channel.is_null() {
        set_last_error("channel must not be null");
        cb(1, user_data);
        return;
    }

    let channel = unsafe { Box::from_raw(channel as *mut ChannelHandle) };
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let res = channel.channel.close().await;
        cb(
            int_saikuro(res, "channel close"),
            user_data_addr as *mut c_void,
        );
    });
}

#[cfg(feature = "std")]
#[no_mangle]
pub extern "C" fn saikuro_channel_abort_async(
    channel: *mut c_void,
    cb: Option<SaikuroStatusCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if channel.is_null() {
        set_last_error("channel must not be null");
        cb(1, user_data);
        return;
    }

    let channel = unsafe { Box::from_raw(channel as *mut ChannelHandle) };
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let res = channel.channel.abort().await;
        cb(
            int_saikuro(res, "channel abort"),
            user_data_addr as *mut c_void,
        );
    });
}

/// # Safety
/// `cb` must not be null. The channel handle must remain valid until `cb` fires,
/// and `saikuro_channel_next_json_async` must not be called concurrently on the
/// same channel.
#[cfg(feature = "std")]
#[no_mangle]
pub unsafe extern "C" fn saikuro_channel_next_json_async(
    channel: *mut c_void,
    cb: Option<SaikuroItemCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if channel.is_null() {
        set_last_error("channel must not be null");
        return;
    }

    let channel_addr = channel as usize;
    let user_data_addr = user_data as usize;
    spawn_future(async move {
        let channel = channel_addr as *mut ChannelHandle;
        let c = unsafe { &mut *channel };
        match c.channel.next().await {
            Some(Ok(value)) => match serde_json::to_string(&value) {
                Ok(json) => cb(into_c_string_ptr(&json), 0, user_data_addr as *mut c_void),
                Err(e) => {
                    set_last_error(format!("failed to serialize channel item: {e}"));
                    cb(ptr::null_mut(), 1, user_data_addr as *mut c_void);
                }
            },
            Some(Err(e)) => {
                set_last_error(format!("channel receive failed: {e}"));
                cb(ptr::null_mut(), 1, user_data_addr as *mut c_void);
            }
            None => cb(ptr::null_mut(), 1, user_data_addr as *mut c_void),
        }
    });
}

// Provider lifecycle (sync register; async serve).

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

    Box::into_raw(Box::new(ProviderHandle {
        provider: Some(Provider::new(&namespace)),
    })) as *mut c_void
}

/// Safety: The `user_data` pointer is captured and later used inside asynchronous
/// callbacks registered with the provider. Callers must ensure that `user_data`
/// remains valid for the entire lifetime of the registered provider (until
/// `saikuro_provider_free` is called).
async fn invoke_c_handler(
    callback: ProviderHandler,
    user_data_addr: usize,
    args: Vec<Value>,
) -> Result<Value, saikuro::Error> {
    let args_json = serde_json::to_string(&args)
        .map_err(|e| saikuro::Error::InvalidState(format!("args encode failed: {e}")))?;
    let args_c = CString::new(args_json)
        .map_err(|_| saikuro::Error::InvalidState("args contain NUL byte".to_owned()))?;

    let result_ptr = unsafe { (callback)(user_data_addr as *mut c_void, args_c.as_ptr()) };
    if result_ptr.is_null() {
        return Err(saikuro::Error::InvalidState(
            "C handler returned null".to_owned(),
        ));
    }

    let result_owned = unsafe { CString::from_raw(result_ptr) };
    let result_str = result_owned
        .to_str()
        .map_err(|_| saikuro::Error::InvalidState("C handler returned non-UTF8".to_owned()))?
        .to_owned();

    let value: Value = serde_json::from_str(&result_str).map_err(|e| {
        saikuro::Error::InvalidState(format!("C handler returned invalid JSON: {e}"))
    })?;

    Ok(value)
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
        set_last_error(ERR_HANDLE_NULL);
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
        invoke_c_handler(callback, user_data_addr, args)
    });

    0
}

/// Register a handler with schema metadata.
///
/// `nargs` is the number of arguments the function accepts (each typed `Any`).
/// `return_type_json` is a JSON type name (e.g. `"Any"`, `"String"`)
/// or NULL for the default `"Any"`.
#[no_mangle]
pub extern "C" fn saikuro_provider_register_with_schema(
    handle: *mut c_void,
    name: *const c_char,
    callback: Option<ProviderHandler>,
    user_data: *mut c_void,
    nargs: c_int,
    return_type_json: *const c_char,
) -> c_int {
    clear_last_error();

    let handle = match unsafe { (handle as *mut ProviderHandle).as_mut() } {
        Some(h) => h,
        None => {
            set_last_error(ERR_HANDLE_NULL);
            return 1;
        }
    };

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

    let nargs = if nargs < 0 { 0 } else { nargs as usize };

    let return_type = if return_type_json.is_null() {
        TypeDescriptor::primitive(PrimitiveType::Any)
    } else {
        match cstr_to_string(return_type_json, "return_type_json") {
            Ok(s) => match s.to_lowercase().as_str() {
                "string" => TypeDescriptor::primitive(PrimitiveType::String),
                "i64" | "int" | "integer" => TypeDescriptor::primitive(PrimitiveType::I64),
                "f64" | "float" => TypeDescriptor::primitive(PrimitiveType::F64),
                "bool" | "boolean" => TypeDescriptor::primitive(PrimitiveType::Bool),
                "unit" => TypeDescriptor::primitive(PrimitiveType::Unit),
                _ => TypeDescriptor::primitive(PrimitiveType::Any),
            },
            Err(_) => TypeDescriptor::primitive(PrimitiveType::Any),
        }
    };

    let provider = match handle.provider.as_mut() {
        Some(p) => p,
        None => {
            set_last_error("provider has already started serving");
            return 1;
        }
    };

    let user_data_addr = user_data as usize;

    let args: Vec<ArgDescriptor> = (0..nargs)
        .map(|i| ArgDescriptor {
            name: format!("arg{i}"),
            r#type: TypeDescriptor::primitive(PrimitiveType::Any),
            optional: false,
            doc: None,
        })
        .collect();

    let schema = FunctionSchema {
        doc: Some(format!(
            "C/C++ function `{name}` ({nargs} arg(s), returns {return_type:?})"
        )),
        args,
        returns: Some(return_type),
        ..Default::default()
    };

    provider.register_with_options(
        name,
        move |args: Vec<Value>| invoke_c_handler(callback, user_data_addr, args),
        RegisterOptions {
            schema: Some(schema),
        },
    );

    0
}

/// # Safety
/// `cb` must not be null. The provider handle must remain valid until `cb` fires.
#[no_mangle]
pub extern "C" fn saikuro_provider_serve_async(
    handle: *mut c_void,
    address: *const c_char,
    cb: Option<SaikuroStatusCb>,
    user_data: *mut c_void,
) {
    clear_last_error();
    let cb = match cb {
        Some(c) => c,
        None => {
            set_last_error("callback must not be null");
            return;
        }
    };
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        cb(1, user_data);
        return;
    }

    let address = match cstr_to_string(address, "address") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            cb(1, user_data);
            return;
        }
    };

    let handle = unsafe { &mut *(handle as *mut ProviderHandle) };
    let provider = match handle.provider.take() {
        Some(p) => p,
        None => {
            set_last_error("provider has already started serving");
            cb(1, user_data);
            return;
        }
    };

    let user_data_addr = user_data as usize;
    spawn_future(async move {
        match provider.serve(address).await {
            Ok(()) => cb(0, user_data_addr as *mut c_void),
            Err(e) => {
                set_last_error(format!("provider serve failed: {e}"));
                cb(1, user_data_addr as *mut c_void);
            }
        }
    });
}

#[no_mangle]
pub extern "C" fn saikuro_provider_free(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }

    // Drop the provider (and any pending handlers) before freeing the box.
    let mut boxed = unsafe { Box::from_raw(handle as *mut ProviderHandle) };
    let _ = boxed.provider.take();
}

// Synchronous blocking API
// These block the calling thread on the global tokio runtime.

#[cfg(all(feature = "std", feature = "native"))]
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

    let client = match block_on_future(saikuro::Client::connect(address)) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("failed to connect client: {e}"));
            return ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(ClientHandle {
        client: Some(client),
    })) as *mut c_void
}

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_client_close(handle: *mut c_void) -> c_int {
    clear_last_error();
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        return 1;
    }
    let handle_ref = unsafe { &mut *(handle as *mut ClientHandle) };
    let client = match handle_ref.client.take() {
        Some(c) => c,
        None => return 0,
    };
    int_saikuro(block_on_future(client.close()), "close")
}

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_client_call_json(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        return ptr::null_mut();
    }
    let (target, args) = match cstr_to_string(target, "target")
        .and_then(|t| c_json_array(args_json).map(|a| (t, a)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    let h = unsafe { &*(handle as *const ClientHandle) };
    ptr_saikuro(block_on_future(h.client().call(target, args)), "call")
}

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_client_call_json_timeout(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
    timeout_ms: c_int,
) -> *mut c_char {
    clear_last_error();
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        return ptr::null_mut();
    }
    if timeout_ms < 0 {
        set_last_error("timeout_ms must be non-negative");
        return ptr::null_mut();
    }
    let (target, args) = match cstr_to_string(target, "target")
        .and_then(|t| c_json_array(args_json).map(|a| (t, a)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    let timeout = core::time::Duration::from_millis(timeout_ms as u64);
    let h = unsafe { &*(handle as *const ClientHandle) };
    ptr_saikuro(
        block_on_future(h.client().call_with_timeout(target, args, Some(timeout))),
        "call",
    )
}

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_client_cast_json(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
) -> c_int {
    clear_last_error();
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        return 1;
    }
    let (target, args) = match cstr_to_string(target, "target")
        .and_then(|t| c_json_array(args_json).map(|a| (t, a)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return 1;
        }
    };
    let h = unsafe { &*(handle as *const ClientHandle) };
    int_saikuro(block_on_future(h.client().cast(target, args)), "cast")
}

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_client_batch_json(
    handle: *mut c_void,
    calls_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        return ptr::null_mut();
    }
    let raw = match cstr_to_string(calls_json, "calls_json") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    let calls = match parse_batch_calls(&raw) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    let h = unsafe { &*(handle as *const ClientHandle) };
    let res = block_on_future(h.client().batch(calls));
    match res {
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

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_client_stream_json(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
) -> *mut c_void {
    clear_last_error();
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        return ptr::null_mut();
    }
    let (target, args) = match cstr_to_string(target, "target")
        .and_then(|t| c_json_array(args_json).map(|a| (t, a)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    let h = unsafe { &*(handle as *const ClientHandle) };
    match block_on_future(h.client().stream(target, args)) {
        Ok(stream) => Box::into_raw(Box::new(StreamHandle { stream })) as *mut c_void,
        Err(e) => {
            set_last_error(format!("stream open failed: {e}"));
            ptr::null_mut()
        }
    }
}

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_client_channel_json(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
) -> *mut c_void {
    clear_last_error();
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        return ptr::null_mut();
    }
    let (target, args) = match cstr_to_string(target, "target")
        .and_then(|t| c_json_array(args_json).map(|a| (t, a)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    let h = unsafe { &*(handle as *const ClientHandle) };
    match block_on_future(h.client().channel(target, args)) {
        Ok(channel) => Box::into_raw(Box::new(ChannelHandle { channel })) as *mut c_void,
        Err(e) => {
            set_last_error(format!("channel open failed: {e}"));
            ptr::null_mut()
        }
    }
}

#[cfg(all(feature = "std", feature = "native"))]
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
    let c = unsafe { &mut *(channel as *mut ChannelHandle) };
    int_saikuro(block_on_future(c.channel.send(item)), "channel send")
}

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_channel_close(channel: *mut c_void) -> c_int {
    clear_last_error();
    if channel.is_null() {
        set_last_error("channel must not be null");
        return 1;
    }
    let c = unsafe { &mut *(channel as *mut ChannelHandle) };
    int_saikuro(block_on_future(c.channel.close()), "channel close")
}

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_channel_abort(channel: *mut c_void) -> c_int {
    clear_last_error();
    if channel.is_null() {
        set_last_error("channel must not be null");
        return 1;
    }
    let c = unsafe { &mut *(channel as *mut ChannelHandle) };
    int_saikuro(block_on_future(c.channel.abort()), "channel abort")
}

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_channel_next_json(
    channel: *mut c_void,
    out_item_json: *mut *mut c_char,
    out_done: *mut c_int,
) -> c_int {
    clear_last_error();
    if channel.is_null() {
        set_last_error("channel must not be null");
        return 1;
    }
    let c = unsafe { &mut *(channel as *mut ChannelHandle) };
    match block_on_future(c.channel.next()) {
        Some(Ok(value)) => match serde_json::to_string(&value) {
            Ok(json) => {
                unsafe {
                    *out_item_json = into_c_string_ptr(&json);
                    *out_done = 0;
                }
                0
            }
            Err(e) => {
                set_last_error(format!("failed to serialize channel item: {e}"));
                unsafe {
                    *out_item_json = ptr::null_mut();
                    *out_done = 1;
                }
                1
            }
        },
        Some(Err(e)) => {
            set_last_error(format!("channel receive failed: {e}"));
            unsafe {
                *out_item_json = ptr::null_mut();
                *out_done = 1;
            }
            1
        }
        None => {
            unsafe {
                *out_item_json = ptr::null_mut();
                *out_done = 1;
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

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_stream_next_json(
    stream: *mut c_void,
    out_item_json: *mut *mut c_char,
    out_done: *mut c_int,
) -> c_int {
    clear_last_error();
    if stream.is_null() {
        set_last_error("stream must not be null");
        return 1;
    }
    let s = unsafe { &mut *(stream as *mut StreamHandle) };
    match block_on_future(s.stream.next()) {
        Some(Ok(value)) => match serde_json::to_string(&value) {
            Ok(json) => {
                unsafe {
                    *out_item_json = into_c_string_ptr(&json);
                    *out_done = 0;
                }
                0
            }
            Err(e) => {
                set_last_error(format!("failed to serialize stream item: {e}"));
                unsafe {
                    *out_item_json = ptr::null_mut();
                    *out_done = 1;
                }
                1
            }
        },
        Some(Err(e)) => {
            set_last_error(format!("stream receive failed: {e}"));
            unsafe {
                *out_item_json = ptr::null_mut();
                *out_done = 1;
            }
            1
        }
        None => {
            unsafe {
                *out_item_json = ptr::null_mut();
                *out_done = 1;
            }
            0
        }
    }
}

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_client_resource_json(
    handle: *mut c_void,
    target: *const c_char,
    args_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        return ptr::null_mut();
    }
    let (target, args) = match cstr_to_string(target, "target")
        .and_then(|t| c_json_array(args_json).map(|a| (t, a)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    let h = unsafe { &*(handle as *const ClientHandle) };
    ptr_saikuro(
        block_on_future(h.client().resource(target, args)),
        "resource",
    )
}

#[cfg(all(feature = "std", feature = "native"))]
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
        set_last_error(ERR_HANDLE_NULL);
        return 1;
    }
    let (level, name, msg) = match cstr_to_string(level, "level")
        .and_then(|l| cstr_to_string(name, "name").map(|n| (l, n)))
        .and_then(|(l, n)| cstr_to_string(msg, "msg").map(|m| (l, n, m)))
    {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return 1;
        }
    };
    let fields = if fields_json.is_null() {
        None
    } else {
        match cstr_to_string(fields_json, "fields_json")
            .and_then(|raw| parse_json_object_arg(&raw, "fields_json").map(Value::Object))
        {
            Ok(v) => Some(v),
            Err(e) => {
                set_last_error(e);
                return 1;
            }
        }
    };
    let h = unsafe { &*(handle as *const ClientHandle) };
    int_saikuro(
        block_on_future(h.client().log(level, name, msg, fields)),
        "log",
    )
}

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_provider_serve(handle: *mut c_void, address: *const c_char) -> c_int {
    clear_last_error();
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        return 1;
    }
    let address = match cstr_to_string(address, "address") {
        Ok(s) => s,
        Err(e) => {
            set_last_error(e);
            return 1;
        }
    };
    let handle_ref = unsafe { &mut *(handle as *mut ProviderHandle) };
    let provider = match handle_ref.provider.take() {
        Some(p) => p,
        None => {
            set_last_error("provider has already started serving");
            return 1;
        }
    };
    int_saikuro(block_on_future(provider.serve(address)), "provider serve")
}

#[cfg(all(feature = "std", feature = "native"))]
#[no_mangle]
pub extern "C" fn saikuro_provider_close(handle: *mut c_void) -> c_int {
    clear_last_error();
    if handle.is_null() {
        set_last_error(ERR_HANDLE_NULL);
        return 1;
    }
    let handle_ref = unsafe { &mut *(handle as *mut ProviderHandle) };
    let _ = handle_ref.provider.take();
    0
}
