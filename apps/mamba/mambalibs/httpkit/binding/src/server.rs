//! Uvicorn-style native run surface for `mambalibs.http`.
//!
//! This is an additive mambalibs API. It returns a native server handle over
//! the App metadata and HostConfig; it does not mutate CPython stdlib behavior
//! or the third-party `uvicorn` shim.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use mamba_registry::{
    convert::{mb_unwrap_native_ref, mb_wrap_native_typed},
    ops, rt_sym, FromMbValue, MbValue, ModuleRegistrar,
};
use mambalibs_http::app::App;
use mambalibs_http::host::HostConfig;

type NativeFn = unsafe extern "C" fn(*const MbValue, usize) -> MbValue;

thread_local! {
    static ACTIVE_APP: RefCell<Option<MbValue>> = const { RefCell::new(None) };
    static ACTIVE_SERVER: RefCell<Option<MbValue>> = const { RefCell::new(None) };
}

#[derive(Clone)]
pub struct MbServer {
    pub app: MbValue,
    pub config: HostConfig,
    pub url: String,
    pub openapi_json: String,
    pub endpoint_count: usize,
    running: Arc<AtomicBool>,
}

impl MbServer {
    fn new(app_value: MbValue, app: &App, host: String, port: u16) -> Self {
        let mut config = HostConfig::default();
        config.bind_host = host;
        config.bind_port = port;
        let url = format!("http://{}:{}", config.bind_host, config.bind_port);
        Self {
            app: app_value,
            config,
            url,
            openapi_json: app.openapi_json(),
            endpoint_count: app.endpoint_count(),
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

#[inline]
unsafe fn read(args: *const MbValue, nargs: usize, index: usize) -> MbValue {
    if index < nargs {
        unsafe { *args.add(index) }
    } else {
        MbValue::none()
    }
}

unsafe fn read_app(value: MbValue) -> Option<&'static App> {
    unsafe { mb_unwrap_native_ref::<App>(value) }
}

unsafe fn read_server(value: MbValue) -> Option<&'static MbServer> {
    unsafe { mb_unwrap_native_ref::<MbServer>(value) }
}

fn read_string(value: MbValue) -> Option<String> {
    String::from_mb_value(value).ok()
}

fn read_u16(value: MbValue) -> Option<u16> {
    i64::from_mb_value(value)
        .ok()
        .and_then(|value| u16::try_from(value).ok())
}

fn read_host_and_port(args: *const MbValue, nargs: usize, host_index: usize) -> (String, u16) {
    let host = read_string(unsafe { read(args, nargs, host_index) })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = read_u16(unsafe { read(args, nargs, host_index + 1) }).unwrap_or(8000);
    (host, port)
}

fn native_func(func: NativeFn) -> MbValue {
    MbValue::from_func(func as usize)
}

fn bind_app(receiver: MbValue) {
    ACTIVE_APP.with(|slot| {
        *slot.borrow_mut() = Some(receiver);
    });
}

fn active_app() -> Option<MbValue> {
    ACTIVE_APP.with(|slot| *slot.borrow())
}

fn bind_server(receiver: MbValue) {
    ACTIVE_SERVER.with(|slot| {
        *slot.borrow_mut() = Some(receiver);
    });
}

fn active_server() -> Option<MbValue> {
    ACTIVE_SERVER.with(|slot| *slot.borrow())
}

fn server_value(app_value: MbValue, host: String, port: u16) -> MbValue {
    let Some(app) = (unsafe { read_app(app_value) }) else {
        return MbValue::none();
    };
    mb_wrap_native_typed("Server", MbServer::new(app_value, app, host, port))
}

#[no_mangle]
pub unsafe extern "C" fn server_new(args: *const MbValue, nargs: usize) -> MbValue {
    server_run(args, nargs)
}

#[no_mangle]
pub unsafe extern "C" fn server_run(args: *const MbValue, nargs: usize) -> MbValue {
    let app_value = unsafe { read(args, nargs, 0) };
    let (host, port) = read_host_and_port(args, nargs, 1);
    server_value(app_value, host, port)
}

#[no_mangle]
pub unsafe extern "C" fn app_run_bound(args: *const MbValue, nargs: usize) -> MbValue {
    let Some(app_value) = active_app() else {
        return MbValue::none();
    };
    let (host, port) = read_host_and_port(args, nargs, 0);
    server_value(app_value, host, port)
}

pub unsafe extern "C" fn get_app_run(args: *const MbValue, nargs: usize) -> MbValue {
    bind_app(unsafe { read(args, nargs, 0) });
    native_func(app_run_bound)
}

#[no_mangle]
pub unsafe extern "C" fn server_stop_bound(_args: *const MbValue, _nargs: usize) -> MbValue {
    let Some(server_value) = active_server() else {
        return MbValue::none();
    };
    if let Some(server) = unsafe { read_server(server_value) } {
        server.stop();
    }
    server_value
}

pub unsafe extern "C" fn get_server_url(args: *const MbValue, nargs: usize) -> MbValue {
    let server_value = unsafe { read(args, nargs, 0) };
    let Some(server) = (unsafe { read_server(server_value) }) else {
        return MbValue::none();
    };
    (ops().str_new)(&server.url)
}

pub unsafe extern "C" fn get_server_host(args: *const MbValue, nargs: usize) -> MbValue {
    let server_value = unsafe { read(args, nargs, 0) };
    let Some(server) = (unsafe { read_server(server_value) }) else {
        return MbValue::none();
    };
    (ops().str_new)(&server.config.bind_host)
}

pub unsafe extern "C" fn get_server_port(args: *const MbValue, nargs: usize) -> MbValue {
    let server_value = unsafe { read(args, nargs, 0) };
    let Some(server) = (unsafe { read_server(server_value) }) else {
        return MbValue::none();
    };
    MbValue::from_int(i64::from(server.config.bind_port))
}

pub unsafe extern "C" fn get_server_running(args: *const MbValue, nargs: usize) -> MbValue {
    let server_value = unsafe { read(args, nargs, 0) };
    let Some(server) = (unsafe { read_server(server_value) }) else {
        return MbValue::from_bool(false);
    };
    MbValue::from_bool(server.is_running())
}

pub unsafe extern "C" fn get_server_endpoint_count(args: *const MbValue, nargs: usize) -> MbValue {
    let server_value = unsafe { read(args, nargs, 0) };
    let Some(server) = (unsafe { read_server(server_value) }) else {
        return MbValue::from_int(0);
    };
    MbValue::from_int(server.endpoint_count as i64)
}

pub unsafe extern "C" fn get_server_openapi(args: *const MbValue, nargs: usize) -> MbValue {
    let server_value = unsafe { read(args, nargs, 0) };
    let Some(server) = (unsafe { read_server(server_value) }) else {
        return MbValue::none();
    };
    (ops().str_new)(&server.openapi_json)
}

pub unsafe extern "C" fn get_server_stop(args: *const MbValue, nargs: usize) -> MbValue {
    bind_server(unsafe { read(args, nargs, 0) });
    native_func(server_stop_bound)
}

fn register_getter(type_name: &str, attr: &str, getter: NativeFn) {
    if let Some(o) = mamba_registry::ops::OBJECT_OPS.get() {
        (o.register_getter)(type_name, attr, getter);
    }
}

pub fn register(r: &mut ModuleRegistrar) {
    r.add_symbols([
        rt_sym!(
            "Server",
            server_new,
            "Server(app, host: str = '127.0.0.1', port: int = 8000) -> Server"
        ),
        rt_sym!(
            "run",
            server_run,
            "run(app, host: str = '127.0.0.1', port: int = 8000) -> Server"
        ),
    ]);

    register_getter("App", "run", get_app_run);
    register_getter("Server", "url", get_server_url);
    register_getter("Server", "host", get_server_host);
    register_getter("Server", "port", get_server_port);
    register_getter("Server", "running", get_server_running);
    register_getter("Server", "endpoint_count", get_server_endpoint_count);
    register_getter("Server", "openapi", get_server_openapi);
    register_getter("Server", "stop", get_server_stop);
}
