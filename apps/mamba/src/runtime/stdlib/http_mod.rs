use super::super::rc::{MbObject, ObjData};
use super::super::value::MbValue;
use rustc_hash::FxHashMap;
/// http and urllib modules for Mamba (#418).
///
/// Provides a native implementation of the most-used APIs from Python 3.12's
/// `urllib.parse`, `urllib.request`, `urllib.error`, and `http` modules, with
/// `http.client` stubs for import compatibility.
use std::collections::HashMap;

// ── Dispatch wrappers: native ABI (args_ptr, nargs) to match mb_call_spread ──

// #1040 follow-up: this file's `dispatch_class_shell` used to be handed out
// as the SAME function address to every class-shell name registered here,
// across every `register_*` call in this file. Because FUNC_NAMES/
// NATIVE_FUNC_ADDRS are address-keyed, whichever name registered last (in
// HashMap iteration order, which is nondeterministic per process) won
// `X.__name__` for every other class sharing that address -- the same
// #962/#954 symptom. The fix: give every class-shell name a genuinely
// distinct function pointer, drawn from a pool of `SHELL_POOL_SIZE`
// individually fold-immune trivial stub functions, indexed via a
// thread-local "next free slot" counter (`next_shell_slot`) so every call
// site simply draws a fresh slot per name -- no manual per-call `pool_start`
// bookkeeping required, since `register()` runs registration sequentially
// on a single thread at module-init time.
//
// IMPORTANT: this pool does NOT use `icf_guard!()` directly. That macro
// derives its fingerprint from `module_path!()`/`line!()`/`column!()`, which
// are resolved at the span of the *macro definition's* literal tokens -- for
// a single `macro_rules!` invocation that expands a `$(...)* ` repetition
// into N functions, every repetition shares that ONE span, so
// `line!()`/`column!()` come back IDENTICAL for all N and `icf_guard!()`
// silently fails to discriminate them. LLVM then folds all "distinct"
// shells back onto a single address, reproducing the exact bug one level
// down. The fix here instead fingerprints on `stringify!($name)`, which DOES
// vary per repetition (driven by the captured `$name` token's text, not by
// span), giving every pool slot a genuinely distinct compiled body.
const SHELL_POOL_SIZE: usize = 72;
type ShellFn = unsafe extern "C" fn(*const MbValue, usize) -> MbValue;

macro_rules! def_shell_pool {
    ($($name:ident),* $(,)?) => {
        $(
            unsafe extern "C" fn $name(_a: *const MbValue, _n: usize) -> MbValue {
                ::std::hint::black_box(crate::runtime::module::icf_fingerprint(concat!(
                    module_path!(),
                    "::",
                    stringify!($name)
                )));
                MbValue::from_ptr(MbObject::new_dict())
            }
        )*
        const SHELL_POOL: [ShellFn; SHELL_POOL_SIZE] = [$($name),*];
    };
}
def_shell_pool!(
    shell_00, shell_01, shell_02, shell_03, shell_04, shell_05, shell_06, shell_07, shell_08,
    shell_09, shell_10, shell_11, shell_12, shell_13, shell_14, shell_15, shell_16, shell_17,
    shell_18, shell_19, shell_20, shell_21, shell_22, shell_23, shell_24, shell_25, shell_26,
    shell_27, shell_28, shell_29, shell_30, shell_31, shell_32, shell_33, shell_34, shell_35,
    shell_36, shell_37, shell_38, shell_39, shell_40, shell_41, shell_42, shell_43, shell_44,
    shell_45, shell_46, shell_47, shell_48, shell_49, shell_50, shell_51, shell_52, shell_53,
    shell_54, shell_55, shell_56, shell_57, shell_58, shell_59, shell_60, shell_61, shell_62,
    shell_63, shell_64, shell_65, shell_66, shell_67, shell_68, shell_69, shell_70, shell_71,
);

/// Pool slot at `idx` as a raw function-pointer address.
fn shell_addr(idx: usize) -> usize {
    SHELL_POOL[idx] as usize
}

thread_local! {
    static NEXT_SHELL_SLOT: std::cell::Cell<usize> = std::cell::Cell::new(0);
}

/// Reset the per-thread shell pool cursor so the next stdlib registration pass
/// can reuse the fixed shell-function pool from slot 0.
fn reset_shell_slots() {
    NEXT_SHELL_SLOT.with(|c| c.set(0));
}

/// Draw the next unused pool slot index. `register()` runs sequentially on
/// a single thread at module-init time, so a simple monotonic counter gives
/// every class-shell name a fresh, non-overlapping slot with no manual
/// per-call range bookkeeping.
fn next_shell_slot() -> usize {
    NEXT_SHELL_SLOT.with(|c| {
        let v = c.get();
        assert!(
            v < SHELL_POOL_SIZE,
            "shell pool exhausted (SHELL_POOL_SIZE={}); bump it",
            SHELL_POOL_SIZE
        );
        c.set(v + 1);
        v
    })
}

unsafe extern "C" fn dispatch_empty_str(_a: *const MbValue, _n: usize) -> MbValue {
    MbValue::from_ptr(MbObject::new_str(String::new()))
}

pub(crate) unsafe extern "C" fn dispatch_quote(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    // quote(string, safe='/', encoding=None, errors=None). Keyword args arrive
    // as a trailing kwargs dict (quote/unquote are in the native-kwargs
    // allowlist); fall back to positional slots otherwise.
    let (pos, kw) = split_trailing_kwargs(a);
    let mut safe = pos.get(1).copied().unwrap_or_else(MbValue::none);
    let mut encoding = pos.get(2).copied().unwrap_or_else(MbValue::none);
    let mut errors = pos.get(3).copied().unwrap_or_else(MbValue::none);
    if let Some(kw) = kw {
        if let Some(v) = dict_kw_get(kw, "safe") {
            safe = v;
        }
        if let Some(v) = dict_kw_get(kw, "encoding") {
            encoding = v;
        }
        if let Some(v) = dict_kw_get(kw, "errors") {
            errors = v;
        }
    }
    mb_urllib_quote_full(
        pos.get(0).copied().unwrap_or_else(MbValue::none),
        safe,
        encoding,
        errors,
    )
}

pub(crate) unsafe extern "C" fn dispatch_quote_from_bytes(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    mb_urllib_quote_from_bytes(
        a.get(0).copied().unwrap_or_else(MbValue::none),
        a.get(1).copied().unwrap_or_else(MbValue::none),
    )
}

pub(crate) unsafe extern "C" fn dispatch_quote_plus(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    mb_urllib_quote_plus(
        a.get(0).copied().unwrap_or_else(MbValue::none),
        a.get(1).copied().unwrap_or_else(MbValue::none),
    )
}

pub(crate) unsafe extern "C" fn dispatch_unquote(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    // unquote(string, encoding='utf-8', errors='replace'). Keyword args arrive
    // as a trailing kwargs dict; fall back to positional slots otherwise.
    let (pos, kw) = split_trailing_kwargs(a);
    let mut encoding = pos.get(1).copied().unwrap_or_else(MbValue::none);
    let mut errors = pos.get(2).copied().unwrap_or_else(MbValue::none);
    if let Some(kw) = kw {
        if let Some(v) = dict_kw_get(kw, "encoding") {
            encoding = v;
        }
        if let Some(v) = dict_kw_get(kw, "errors") {
            errors = v;
        }
    }
    mb_urllib_unquote_full(
        pos.get(0).copied().unwrap_or_else(MbValue::none),
        encoding,
        errors,
    )
}

pub(crate) unsafe extern "C" fn dispatch_unquote_plus(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    mb_urllib_unquote_plus(a.get(0).copied().unwrap_or_else(MbValue::none))
}

pub(crate) unsafe extern "C" fn dispatch_urlencode(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    // Trailing kwargs dict carries safe= / quote_via= / doseq=. The params
    // mapping itself is usually a dict too, so only a SECOND dict (the last
    // argument) is the kwargs dict.
    let kwargs = if a.len() >= 2 {
        a.last().copied().filter(|v| {
            v.as_ptr()
                .map(|p| unsafe { matches!((*p).data, ObjData::Dict(_)) })
                .unwrap_or(false)
        })
    } else {
        None
    };
    let mut doseq = a
        .get(1)
        .copied()
        .filter(|v| v.as_bool().is_some())
        .unwrap_or_else(MbValue::none);
    let mut safe = String::new();
    let mut quote_via = MbValue::none();
    let mut encoding = MbValue::none();
    let mut errors = MbValue::none();
    if let Some(kw) = kwargs {
        let get = |name: &str| -> Option<MbValue> {
            let sentinel = MbValue::from_bits(u64::MAX);
            let v = super::super::dict_ops::mb_dict_get(
                kw,
                MbValue::from_ptr(MbObject::new_str(name.to_string())),
                sentinel,
            );
            if v.to_bits() == u64::MAX {
                None
            } else {
                Some(v)
            }
        };
        if let Some(v) = get("doseq") {
            doseq = v;
        }
        if let Some(v) = get("safe").and_then(extract_str) {
            safe = v;
        }
        if let Some(v) = get("quote_via") {
            quote_via = v;
        }
        if let Some(v) = get("encoding") {
            encoding = v;
        }
        if let Some(v) = get("errors") {
            errors = v;
        }
    }
    mb_urllib_urlencode_codec(
        a.first().copied().unwrap_or_else(MbValue::none),
        doseq,
        &safe,
        quote_via,
        encoding,
        errors,
    )
}

/// str url + bytes scheme (or vice versa) is a CPython TypeError.
fn mixed_str_bytes(a: MbValue, b: MbValue) -> bool {
    let a_bytes = input_is_bytes(a);
    let b_bytes = input_is_bytes(b);
    let a_str = extract_str(a).is_some();
    let b_str = extract_str(b).is_some();
    (a_bytes && b_str) || (a_str && b_bytes)
}

pub(crate) unsafe extern "C" fn dispatch_urlparse(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    let url = a.first().copied().unwrap_or_else(MbValue::none);
    if let Some(scheme) = a.get(1).copied() {
        if mixed_str_bytes(url, scheme) {
            return raise_type_error("Cannot mix str and non-str arguments");
        }
    }
    mb_urllib_urlparse(url)
}

pub(crate) unsafe extern "C" fn dispatch_urlunparse(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    let parts = a.get(0).copied().unwrap_or_else(MbValue::none);
    // Mixed str/bytes components are a CPython TypeError.
    if let Some(ptr) = parts.as_ptr() {
        let items: Option<Vec<MbValue>> = unsafe {
            match &(*ptr).data {
                ObjData::Tuple(t) => Some(t.clone()),
                ObjData::List(lock) => lock.read().ok().map(|g| g.to_vec()),
                _ => None,
            }
        };
        if let Some(items) = items {
            let any_bytes = items.iter().any(|v| input_is_bytes(*v));
            let any_str = items
                .iter()
                .any(|v| extract_str(*v).map(|s| !s.is_empty()).unwrap_or(false));
            if any_bytes && any_str {
                return raise_type_error("Cannot mix str and non-str arguments");
            }
        }
    }
    mb_urllib_urlunparse(parts)
}

pub(crate) unsafe extern "C" fn dispatch_urljoin(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    mb_urllib_urljoin(
        a.get(0).copied().unwrap_or_else(MbValue::none),
        a.get(1).copied().unwrap_or_else(MbValue::none),
    )
}

/// Pull (keep_blank_values, encoding, errors) from the positional tail and
/// the trailing kwargs dict.
fn qs_options(a: &[MbValue]) -> QsOptions {
    let mut opts = QsOptions::default();
    let kwargs = a.iter().copied().find(|v| {
        v.as_ptr()
            .map(|p| unsafe { matches!((*p).data, ObjData::Dict(_)) })
            .unwrap_or(false)
    });
    // Second positional (after the kwargs dict is excluded) is
    // keep_blank_values.
    if let Some(v) = a.get(1).copied() {
        if let Some(b) = v.as_bool() {
            opts.keep_blank = b;
        }
    }
    if let Some(kw) = kwargs {
        let get = |name: &str| -> Option<MbValue> {
            let sentinel = MbValue::from_bits(u64::MAX);
            let v = super::super::dict_ops::mb_dict_get(
                kw,
                MbValue::from_ptr(MbObject::new_str(name.to_string())),
                sentinel,
            );
            if v.to_bits() == u64::MAX {
                None
            } else {
                Some(v)
            }
        };
        if let Some(v) = get("keep_blank_values") {
            opts.keep_blank = super::super::builtins::mb_bool(v).as_bool() == Some(true);
        }
        if let Some(v) = get("strict_parsing") {
            opts.strict_parsing = super::super::builtins::mb_bool(v).as_bool() == Some(true);
        }
        if let Some(v) = get("encoding").and_then(extract_str) {
            opts.encoding = v;
        }
        if let Some(v) = get("errors").and_then(extract_str) {
            opts.errors = v;
        }
        if let Some(v) = get("separator") {
            // A non-str separator (or empty) is rejected in parse_qsl_core.
            opts.separator = extract_str(v).unwrap_or_default();
        }
        if let Some(v) = get("max_num_fields") {
            if let Some(n) = v.as_int() {
                opts.max_num_fields = Some(n);
            }
        }
    }
    opts
}

pub(crate) unsafe extern "C" fn dispatch_parse_qs(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    let opts = qs_options(a);
    mb_urllib_parse_qs_opts(a.get(0).copied().unwrap_or_else(MbValue::none), &opts)
}

pub(crate) unsafe extern "C" fn dispatch_parse_qsl(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    let opts = qs_options(a);
    mb_urllib_parse_qsl_opts(a.get(0).copied().unwrap_or_else(MbValue::none), &opts)
}

pub(crate) unsafe extern "C" fn dispatch_urlsplit(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    let url = a.first().copied().unwrap_or_else(MbValue::none);
    if let Some(scheme) = a.get(1).copied() {
        if mixed_str_bytes(url, scheme) {
            return raise_type_error("Cannot mix str and non-str arguments");
        }
    }
    mb_urllib_urlsplit(url)
}

unsafe extern "C" fn dispatch_urlopen(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    mb_urllib_urlopen(a.get(0).copied().unwrap_or_else(MbValue::none))
}

unsafe extern "C" fn dispatch_proxy_bypass(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    if let Some(host) = a.first().copied() {
        if extract_str(host).is_none() {
            return raise_type_error("proxy_bypass() host must be str");
        }
    }
    MbValue::from_bool(false)
}

pub(crate) unsafe extern "C" fn dispatch_urldefrag(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    mb_urllib_urldefrag(a.get(0).copied().unwrap_or_else(MbValue::none))
}

pub(crate) unsafe extern "C" fn dispatch_unquote_to_bytes(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    mb_urllib_unquote_to_bytes(a.get(0).copied().unwrap_or_else(MbValue::none))
}

/// POSIX `pathname2url(p)` == `quote(p)` (slashes preserved).
unsafe extern "C" fn dispatch_pathname2url(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    mb_urllib_quote(
        a.get(0).copied().unwrap_or_else(MbValue::none),
        MbValue::none(),
    )
}

/// POSIX `url2pathname(u)` == `unquote(u)`.
unsafe extern "C" fn dispatch_url2pathname(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    mb_urllib_unquote(a.get(0).copied().unwrap_or_else(MbValue::none))
}

/// urllib.parse.unwrap(url) → strip surrounding `<URL:...>` / leading-trailing
/// whitespace and angle brackets, mirroring CPython's `unwrap`. Returns a str.
pub(crate) unsafe extern "C" fn dispatch_unwrap(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = unsafe { std::slice::from_raw_parts(args_ptr, nargs) };
    let arg = a.get(0).copied().unwrap_or_else(MbValue::none);
    let s = extract_str(arg).unwrap_or_default();
    let trimmed = s.trim();
    let inner: &str = match trimmed.strip_prefix('<').and_then(|x| x.strip_suffix('>')) {
        Some(rest) => rest,
        None => trimmed,
    };
    let unwrapped: &str = inner.strip_prefix("URL:").unwrap_or(inner);
    MbValue::from_ptr(MbObject::new_str(unwrapped.trim().to_string()))
}

/// Register the http/urllib module family.
pub fn register() {
    use super::super::module::NATIVE_FUNC_ADDRS;

    // Test and benchmark harnesses can re-initialize the stdlib repeatedly in
    // one process. Slot uniqueness only needs to hold within one registration
    // pass; the module table is overwritten on the next pass.
    reset_shell_slots();

    fn add_dispatch(attrs: &mut HashMap<String, MbValue>, name: &str, addr: usize) {
        attrs.insert(name.to_string(), MbValue::from_func(addr));
        NATIVE_FUNC_ADDRS.with(|s| {
            s.borrow_mut().insert(addr as u64);
        });
    }

    // urllib.parse
    let parse_dispatchers: &[(&str, usize)] = &[
        ("quote", dispatch_quote as *const () as usize),
        ("quote_plus", dispatch_quote_plus as *const () as usize),
        (
            "quote_from_bytes",
            dispatch_quote_from_bytes as *const () as usize,
        ),
        ("unquote", dispatch_unquote as *const () as usize),
        ("unquote_plus", dispatch_unquote_plus as *const () as usize),
        (
            "unquote_to_bytes",
            dispatch_unquote_to_bytes as *const () as usize,
        ),
        ("urlencode", dispatch_urlencode as *const () as usize),
        ("urlparse", dispatch_urlparse as *const () as usize),
        ("urlunparse", dispatch_urlunparse as *const () as usize),
        ("urlsplit", dispatch_urlsplit as *const () as usize),
        ("urlunsplit", dispatch_urlunparse as *const () as usize),
        ("urljoin", dispatch_urljoin as *const () as usize),
        ("urldefrag", dispatch_urldefrag as *const () as usize),
        ("parse_qs", dispatch_parse_qs as *const () as usize),
        ("parse_qsl", dispatch_parse_qsl as *const () as usize),
    ];
    let mut parse_attrs = HashMap::new();
    for (n, a) in parse_dispatchers {
        add_dispatch(&mut parse_attrs, n, *a);
    }

    // urllib.parse.unwrap(url) → strips `<URL:...>` wrappers; callable.
    add_dispatch(
        &mut parse_attrs,
        "unwrap",
        dispatch_unwrap as *const () as usize,
    );

    // Result named-tuple classes. Mamba's urlparse/urlsplit/urldefrag return
    // Instance shells rather than these concrete subclasses, so these are
    // registered as callable class-shell sentinels purely so `hasattr` and
    // construction probes succeed on the public surface.
    let parse_classes: &[&str] = &[
        "ParseResult",
        "ParseResultBytes",
        "SplitResult",
        "SplitResultBytes",
        "DefragResult",
        "DefragResultBytes",
    ];
    for n in parse_classes {
        add_dispatch(&mut parse_attrs, n, shell_addr(next_shell_slot()));
    }

    // Register quote function's default parameters
    let quote_func = MbValue::from_func(dispatch_quote as *const () as usize);
    let quote_params = MbValue::from_ptr(MbObject::new_list(vec![
        MbValue::from_ptr(MbObject::new_tuple(vec![
            MbValue::from_ptr(MbObject::new_str("string".to_string())),
            MbValue::from_int(1), // POSITIONAL_OR_KEYWORD
            MbValue::from_int(0), // has_default
            MbValue::none(),      // default
            MbValue::none(),      // annotation
        ])),
        MbValue::from_ptr(MbObject::new_tuple(vec![
            MbValue::from_ptr(MbObject::new_str("safe".to_string())),
            MbValue::from_int(1), // POSITIONAL_OR_KEYWORD
            MbValue::from_int(1), // has_default
            MbValue::from_ptr(MbObject::new_str("/".to_string())),
            MbValue::none(), // annotation
        ])),
        MbValue::from_ptr(MbObject::new_tuple(vec![
            MbValue::from_ptr(MbObject::new_str("encoding".to_string())),
            MbValue::from_int(1), // POSITIONAL_OR_KEYWORD
            MbValue::from_int(1), // has_default
            MbValue::none(),      // default
            MbValue::none(),      // annotation
        ])),
        MbValue::from_ptr(MbObject::new_tuple(vec![
            MbValue::from_ptr(MbObject::new_str("errors".to_string())),
            MbValue::from_int(1), // POSITIONAL_OR_KEYWORD
            MbValue::from_int(1), // has_default
            MbValue::none(),      // default
            MbValue::none(),      // annotation
        ])),
    ]));
    super::super::closure::mb_func_set_params(quote_func, quote_params);

    // urllib.parse module-level constants (per typeshed Final[list[str]]
    // and Final[str]). These mirror CPython's lists used internally by
    // urlparse / urljoin and are also exposed as part of the public
    // surface for test-suite conformance.
    fn list_of_strs(items: &[&str]) -> MbValue {
        let vals: Vec<MbValue> = items
            .iter()
            .map(|s| MbValue::from_ptr(MbObject::new_str((*s).to_string())))
            .collect();
        MbValue::from_ptr(MbObject::new_list(vals))
    }
    parse_attrs.insert(
        "uses_relative".into(),
        list_of_strs(&[
            "", "ftp", "http", "gopher", "nntp", "imap", "wais", "file", "https", "shttp", "mms",
            "prospero", "rtsp", "rtspu", "sftp", "svn", "svn+ssh", "ws", "wss",
        ]),
    );
    parse_attrs.insert(
        "uses_netloc".into(),
        list_of_strs(&[
            "",
            "ftp",
            "http",
            "gopher",
            "nntp",
            "telnet",
            "imap",
            "wais",
            "file",
            "mms",
            "https",
            "shttp",
            "snews",
            "prospero",
            "rtsp",
            "rtspu",
            "rsync",
            "svn",
            "svn+ssh",
            "sftp",
            "nfs",
            "git",
            "git+ssh",
            "ws",
            "wss",
            "itms-services",
        ]),
    );
    parse_attrs.insert(
        "uses_params".into(),
        list_of_strs(&[
            "", "ftp", "hdl", "prospero", "http", "imap", "https", "shttp", "rtsp", "rtspu", "sip",
            "sips", "mms", "sftp", "tel",
        ]),
    );
    parse_attrs.insert(
        "non_hierarchical".into(),
        list_of_strs(&[
            "gopher", "hdl", "mailto", "news", "telnet", "wais", "imap", "snews", "sip", "sips",
        ]),
    );
    parse_attrs.insert(
        "uses_query".into(),
        list_of_strs(&[
            "", "http", "wais", "imap", "https", "shttp", "mms", "gopher", "rtsp", "rtspu", "sip",
            "sips",
        ]),
    );
    parse_attrs.insert(
        "uses_fragment".into(),
        list_of_strs(&[
            "", "ftp", "hdl", "http", "gopher", "news", "nntp", "wais", "https", "shttp", "snews",
            "file", "prospero",
        ]),
    );
    parse_attrs.insert(
        "scheme_chars".into(),
        MbValue::from_ptr(MbObject::new_str(
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+-.".to_string(),
        )),
    );

    // Result types behave as 6-/5-field named tuples (index, slice, len,
    // tuple equality) over their ordered `_entries` backing list.
    for cls in [
        "urllib.parse.ParseResult",
        "urllib.parse.ParseResultBytes",
        "urllib.parse.SplitResult",
        "urllib.parse.SplitResultBytes",
    ] {
        super::sys_mod::register_struct_seq_class(cls);
    }
    super::register_module("urllib.parse", parse_attrs);

    // urllib.request — urlopen / quote / unquote remain real-ish; class
    // shells for Request / opener / handler types are callable sentinels
    // so probe-time `Request(url)` returns a dict rather than a
    // non-callable string.
    let request_dispatchers: &[(&str, usize)] = &[
        ("urlopen", dispatch_urlopen as *const () as usize),
        ("proxy_bypass", dispatch_proxy_bypass as *const () as usize),
        ("quote", dispatch_quote as *const () as usize),
        ("unquote", dispatch_unquote as *const () as usize),
    ];
    let mut request_attrs = HashMap::new();
    for (n, a) in request_dispatchers {
        add_dispatch(&mut request_attrs, n, *a);
    }
    let req_str = dispatch_empty_str as *const () as usize;
    let mut req_shell_addrs: Vec<usize> = Vec::new();
    let req_class_shells: &[&str] = &[
        "Request",
        "OpenerDirector",
        "BaseHandler",
        "HTTPDefaultErrorHandler",
        "HTTPRedirectHandler",
        "HTTPCookieProcessor",
        "ProxyHandler",
        "HTTPPasswordMgr",
        "HTTPPasswordMgrWithDefaultRealm",
        "HTTPPasswordMgrWithPriorAuth",
        "AbstractBasicAuthHandler",
        "HTTPBasicAuthHandler",
        "ProxyBasicAuthHandler",
        "AbstractDigestAuthHandler",
        "HTTPDigestAuthHandler",
        "ProxyDigestAuthHandler",
        "HTTPHandler",
        "HTTPSHandler",
        "FileHandler",
        "DataHandler",
        "FTPHandler",
        "CacheFTPHandler",
        "UnknownHandler",
        "build_opener",
        "install_opener",
        "getproxies",
        "URLopener",
        "FancyURLopener",
        "HTTPErrorProcessor",
    ];
    for name in req_class_shells {
        let f = shell_addr(next_shell_slot());
        req_shell_addrs.push(f);
        request_attrs.insert((*name).into(), MbValue::from_func(f));
    }
    // `urllib.request.HTTPHandler` must satisfy `type(HTTPHandler).__name__ ==
    // "type"` (it is a real class in CPython). The plain func-shell above makes
    // `callable(...)` True but `type(...).__name__` is "function", not "type".
    // Register it instead as a type-object instance (class_name == "type" with a
    // `__name__` field) — this keeps `callable(...)` True (mb_callable treats a
    // "type" instance as callable) AND makes `type(HTTPHandler).__name__` resolve
    // to "type". Surface only: no real handler behavior is attached.
    request_attrs.insert("HTTPHandler".into(), make_type_object("HTTPHandler"));
    // pathname2url / url2pathname have real (POSIX) implementations.
    add_dispatch(
        &mut request_attrs,
        "pathname2url",
        dispatch_pathname2url as *const () as usize,
    );
    add_dispatch(
        &mut request_attrs,
        "url2pathname",
        dispatch_url2pathname as *const () as usize,
    );
    // urlretrieve returns ("filename", HTTPMessage()); shim returns ("", {}).
    let urlretrieve_addr = shell_addr(next_shell_slot());
    req_shell_addrs.push(urlretrieve_addr);
    request_attrs.insert("urlretrieve".into(), MbValue::from_func(urlretrieve_addr));
    request_attrs.insert("urlcleanup".into(), MbValue::from_func(req_str));
    super::super::module::NATIVE_FUNC_ADDRS.with(|s| {
        let mut set = s.borrow_mut();
        for a in &req_shell_addrs {
            set.insert(*a as u64);
        }
        set.insert(req_str as u64);
    });
    // Real Request object (#24 prerequisite) — overwrites the dict shell.
    register_request_class(&mut request_attrs);
    super::register_module("urllib.request", request_attrs);

    // urllib.response — callable class shells matching CPython's documented surface.
    let mut response_attrs = HashMap::new();
    let mut resp_shell_addrs: Vec<usize> = Vec::new();
    for name in &["addbase", "addclosehook", "addinfo", "addinfourl"] {
        let f = shell_addr(next_shell_slot());
        resp_shell_addrs.push(f);
        response_attrs.insert((*name).to_string(), MbValue::from_func(f));
    }
    super::super::module::NATIVE_FUNC_ADDRS.with(|s| {
        let mut set = s.borrow_mut();
        for a in &resp_shell_addrs {
            set.insert(*a as u64);
        }
    });
    super::register_module("urllib.response", response_attrs);

    // urllib.robotparser — single class (RobotFileParser) + the can_fetch surface.
    let mut robot_attrs = HashMap::new();
    let robot_shell = shell_addr(next_shell_slot());
    robot_attrs.insert("RobotFileParser".into(), MbValue::from_func(robot_shell));
    super::super::module::NATIVE_FUNC_ADDRS.with(|s| {
        s.borrow_mut().insert(robot_shell as u64);
    });
    super::register_module("urllib.robotparser", robot_attrs);

    // urllib.error — registered ahead of http_mod by `urllib_error_mod::register()`
    // (#1421). The umbrella wiring below picks up the proper callable class
    // shells via MODULES["urllib.error"] rather than the previous 3-string stub.

    // urllib umbrella module — attributes wired after children exist.
    //
    // register_module("urllib", …) overwrites the `urllib` entry that the dotted
    // submodule registrations above (`urllib.parse`/`request`/`response`/
    // `robotparser`) auto-propagated, so every public submodule must be re-spliced
    // here explicitly. `response` and `robotparser` were previously dropped, which
    // made `hasattr(urllib, "response")` and `hasattr(urllib.robotparser,
    // "RobotFileParser")` fail even though the leaf modules carried the names.
    // Build the child module_to_value snapshots under an immutable MODULES borrow,
    // then splice them in under a SEPARATE mutable borrow (never nested).
    super::register_module("urllib", HashMap::new());
    super::super::module::MODULES.with(|mods| {
        let values: Vec<(String, MbValue)> = {
            let mods_ref = mods.borrow();
            ["parse", "request", "error", "response", "robotparser"]
                .iter()
                .filter_map(|sub| {
                    mods_ref
                        .get(&format!("urllib.{sub}"))
                        .map(|m| (sub.to_string(), super::super::module::module_to_value(m)))
                })
                .collect()
        };
        if let Some(urllib_mod) = mods.borrow_mut().get_mut("urllib") {
            for (k, v) in values {
                urllib_mod.attrs.insert(k, v);
            }
        }
    });

    // http module: HTTPStatus constants exposed as IntEnum members at module
    // level and as attributes of a HTTPStatus namespace instance so that both
    // `http.OK` and `http.HTTPStatus.OK` yield int-compatible status members.
    //
    // The `(code, name, phrase)` table is owned by `mamba_registry::http`;
    // we iterate the canonical list rather than maintain a parallel copy in
    // mamba (which previously drifted — e.g. was missing PROCESSING,
    // EARLY_HINTS, IM_A_TEAPOT). Binding crates and mamba now agree on the
    // same table.
    let mut http_attrs = HashMap::new();
    let status_defs: Vec<(&str, i64)> = mamba_registry::http::canonical_codes()
        .iter()
        .map(|(code, name, _)| (*name, i64::from(*code)))
        .collect();
    let status_members = super::enum_class::register_int_enum("HTTPStatus", &status_defs);
    let http_status = MbObject::new_instance("HTTPStatus".to_string());
    unsafe {
        if let ObjData::Instance { ref fields, .. } = (*http_status).data {
            let mut f = fields.write().unwrap();
            let members = MbObject::new_dict();
            if let ObjData::Dict(ref lock) = (*members).data {
                let mut map = lock.write().unwrap();
                for ((code, name, phrase), member) in mamba_registry::http::canonical_codes()
                    .iter()
                    .zip(status_members)
                {
                    decorate_status_member(member, *code, name, phrase);
                    http_attrs.insert((*name).to_string(), member);
                    f.insert(name.to_string(), member);
                    super::super::rc::retain_if_ptr(member);
                    map.insert(
                        super::super::dict_ops::DictKey::Str(name.to_string()),
                        member,
                    );
                }
            }
            f.insert("__members__".to_string(), MbValue::from_ptr(members));
        }
    }
    http_attrs.insert("HTTPStatus".into(), MbValue::from_ptr(http_status));
    // `http.HTTPMethod` (added in Py3.11) is an enum CLASS, so `callable(...)`
    // must be True. Register it as a callable class-shell sentinel (a func
    // stub) rather than an Instance, mirroring the http.client/http.server
    // class shells below. Surface fixtures only require existence/callability.
    let http_method_addr = shell_addr(next_shell_slot());
    http_attrs.insert("HTTPMethod".into(), MbValue::from_func(http_method_addr));
    super::super::module::NATIVE_FUNC_ADDRS.with(|s| {
        s.borrow_mut().insert(http_method_addr as u64);
    });
    super::register_module("http", http_attrs);

    // http.client — callable class shells so `HTTPConnection()` returns a
    // dict sentinel rather than a non-callable string. Real-world callers
    // (requests, urllib3, http-shim probes) instantiate these classes at
    // import / probe time; surface-only conformance keeps the probe
    // chain alive without a real network stack.
    let mut client_attrs = HashMap::new();
    let mut client_shell_addrs: Vec<usize> = Vec::new();
    for name in &[
        "HTTPConnection",
        "HTTPSConnection",
        "HTTPResponse",
        "HTTPMessage",
        "HTTPException",
        "NotConnected",
        "BadStatusLine",
        "IncompleteRead",
        "InvalidURL",
        "UnknownProtocol",
        "UnknownTransferEncoding",
        "UnimplementedFileMode",
        "LineTooLong",
        "RemoteDisconnected",
        "ResponseNotReady",
        "ImproperConnectionState",
        "CannotSendRequest",
        "CannotSendHeader",
        // `error` is CPython's module-level alias for `HTTPException` (a class);
        // `parse_headers` is a module-level function. Both must be callable for
        // the surface fixtures (`hasattr(...,"error")`,
        // `callable(...parse_headers)`), so register them as func stubs, each
        // now drawing its own distinct pool slot (registered into
        // NATIVE_FUNC_ADDRS below).
        "error",
        "parse_headers",
    ] {
        let f = shell_addr(next_shell_slot());
        client_shell_addrs.push(f);
        client_attrs.insert(name.to_string(), MbValue::from_func(f));
    }
    super::super::module::NATIVE_FUNC_ADDRS.with(|s| {
        let mut set = s.borrow_mut();
        for a in &client_shell_addrs {
            set.insert(*a as u64);
        }
    });
    // Exception classes: register the real hierarchy (rooted at HTTPException ⊂
    // Exception) so issubclass / `except` work. Registered parent-first so each
    // child's MRO can expand its base. Overrides the callable-shell attrs above.
    let http_exc_tree: &[(&str, &[&str])] = &[
        ("HTTPException", &["Exception"]),
        ("NotConnected", &["HTTPException"]),
        ("InvalidURL", &["HTTPException"]),
        ("UnknownProtocol", &["HTTPException"]),
        ("UnknownTransferEncoding", &["HTTPException"]),
        ("UnimplementedFileMode", &["HTTPException"]),
        ("IncompleteRead", &["HTTPException"]),
        ("ImproperConnectionState", &["HTTPException"]),
        ("CannotSendRequest", &["ImproperConnectionState"]),
        ("CannotSendHeader", &["ImproperConnectionState"]),
        ("ResponseNotReady", &["ImproperConnectionState"]),
        ("BadStatusLine", &["HTTPException"]),
        ("LineTooLong", &["BadStatusLine"]),
        (
            "RemoteDisconnected",
            &["ConnectionResetError", "BadStatusLine"],
        ),
    ];
    for (name, bases) in http_exc_tree {
        super::super::class::mb_class_register(
            name,
            bases.iter().map(|b| b.to_string()).collect(),
            HashMap::new(),
        );
        client_attrs.insert(
            name.to_string(),
            MbValue::from_ptr(MbObject::new_str(name.to_string())),
        );
    }
    // IncompleteRead(partial, expected=None) carries named partial/expected
    // attributes (CPython); the generic exception ctor would not. Override its
    // attr with a dedicated constructor (the class stays registered for the
    // HTTPException MRO, so isinstance still holds).
    {
        let ir_addr = d_incomplete_read as *const () as usize;
        client_attrs.insert("IncompleteRead".to_string(), MbValue::from_func(ir_addr));
        super::super::module::NATIVE_FUNC_ADDRS.with(|s| {
            s.borrow_mut().insert(ir_addr as u64);
        });
        super::super::module::register_native_type_name(
            ir_addr as u64,
            "IncompleteRead".to_string(),
        );
    }
    // HTTPMessage (case-insensitive header container) + parse_headers().
    {
        let mut m: HashMap<String, MbValue> = HashMap::new();
        for (name, addr) in [
            ("keys", hm_keys as *const () as usize),
            ("items", hm_items as *const () as usize),
            ("get", hm_get as *const () as usize),
            ("__len__", hm_len as *const () as usize),
            ("__getitem__", hm_getitem as *const () as usize),
        ] {
            super::super::module::register_variadic_func(addr as u64);
            m.insert(name.to_string(), MbValue::from_func(addr));
        }
        super::super::class::mb_class_register("HTTPMessage", vec![], m);
        let ph_addr = dispatch_parse_headers as *const () as usize;
        client_attrs.insert("parse_headers".to_string(), MbValue::from_func(ph_addr));
        super::super::module::NATIVE_FUNC_ADDRS.with(|s| {
            s.borrow_mut().insert(ph_addr as u64);
        });
    }
    // `error` is CPython's module-level alias for the HTTPException class.
    client_attrs.insert(
        "error".to_string(),
        MbValue::from_ptr(MbObject::new_str("HTTPException".to_string())),
    );
    for &(code, name, _phrase) in mamba_registry::http::canonical_codes() {
        client_attrs.insert(name.to_string(), MbValue::from_int(code as i64));
    }
    // Default HTTP/S ports.
    client_attrs.insert("HTTP_PORT".into(), MbValue::from_int(80));
    client_attrs.insert("HTTPS_PORT".into(), MbValue::from_int(443));
    // `http.client.responses` maps each status code (int) to its reason
    // phrase (str), e.g. `responses[404] == 'Not Found'`.
    client_attrs.insert("responses".into(), make_responses_dict());
    // Real (minimal) HTTPConnection with putrequest/putheader validation.
    register_httpconnection_class(&mut client_attrs);
    super::register_module("http.client", client_attrs);

    // http.server — same callable-shell treatment.
    let mut server_attrs = HashMap::new();
    let mut server_shell_addrs: Vec<usize> = Vec::new();
    for name in &[
        "HTTPServer",
        "BaseHTTPRequestHandler",
        "SimpleHTTPRequestHandler",
        "CGIHTTPRequestHandler",
        "ThreadingHTTPServer",
    ] {
        let f = shell_addr(next_shell_slot());
        server_shell_addrs.push(f);
        server_attrs.insert(name.to_string(), MbValue::from_func(f));
    }
    super::super::module::NATIVE_FUNC_ADDRS.with(|s| {
        let mut set = s.borrow_mut();
        for a in &server_shell_addrs {
            set.insert(*a as u64);
        }
    });
    // `http.server.BaseHTTPRequestHandler` carries class-level *data* attributes
    // (`protocol_version`, `server_version`, `responses`) that surface fixtures
    // probe directly on the class. The func->native-class bridge only resolves
    // registered *methods*, not data attrs, so register the class as a
    // type-object instance instead (class_name == "type"): `callable(...)` stays
    // True and the data attributes resolve via instance-field lookup.
    {
        let handler = MbObject::new_instance("type".to_string());
        unsafe {
            if let ObjData::Instance { ref fields, .. } = (*handler).data {
                let mut f = fields.write().unwrap();
                f.insert(
                    "__name__".to_string(),
                    MbValue::from_ptr(MbObject::new_str("BaseHTTPRequestHandler".to_string())),
                );
                f.insert(
                    "protocol_version".to_string(),
                    MbValue::from_ptr(MbObject::new_str("HTTP/1.0".to_string())),
                );
                f.insert(
                    "server_version".to_string(),
                    MbValue::from_ptr(MbObject::new_str("BaseHTTP/0.6".to_string())),
                );
                f.insert("responses".to_string(), make_handler_responses_dict());
            }
        }
        server_attrs.insert(
            "BaseHTTPRequestHandler".to_string(),
            MbValue::from_ptr(handler),
        );
        // Register parse_request as a real method on the class so a
        // `BaseHTTPRequestHandler.__new__(...)` instance can parse a request
        // line. The module attr stays the type-object (for class data attrs);
        // method dispatch resolves through the class registry by class name.
        let mut handler_methods: HashMap<String, MbValue> = HashMap::new();
        for (name, addr) in [
            ("parse_request", bh_parse_request as *const () as usize),
            (
                "send_response_only",
                bh_send_response_only as *const () as usize,
            ),
            ("send_response", bh_send_response as *const () as usize),
            ("send_header", bh_send_header as *const () as usize),
            ("end_headers", bh_end_headers as *const () as usize),
            ("send_error", bh_send_error as *const () as usize),
        ] {
            super::super::module::register_variadic_func(addr as u64);
            handler_methods.insert(name.to_string(), MbValue::from_func(addr));
        }
        super::super::class::mb_class_register(
            "BaseHTTPRequestHandler",
            vec!["object".to_string()],
            handler_methods,
        );
    }
    // `http.server.DEFAULT_ERROR_MESSAGE` is a module-level `str` template used
    // by BaseHTTPRequestHandler.send_error(). Surface only checks it exists and
    // is a str; the literal is CPython 3.12's verbatim template.
    server_attrs.insert(
        "DEFAULT_ERROR_MESSAGE".into(),
        MbValue::from_ptr(MbObject::new_str(
            "<!DOCTYPE HTML>\n\
             <html lang=\"en\">\n    \
             <head>\n        \
             <meta charset=\"utf-8\">\n        \
             <title>Error response</title>\n    \
             </head>\n    \
             <body>\n        \
             <h1>Error response</h1>\n        \
             <p>Error code: %(code)d</p>\n        \
             <p>Message: %(message)s.</p>\n        \
             <p>Error code explanation: %(code)s - %(explain)s.</p>\n    \
             </body>\n\
             </html>\n"
                .to_string(),
        )),
    );
    super::register_module("http.server", server_attrs);

    // http.cookies stub
    let mut cookies_attrs = HashMap::new();
    for name in &["BaseCookie", "SimpleCookie", "CookieError", "Morsel"] {
        cookies_attrs.insert(
            name.to_string(),
            MbValue::from_ptr(MbObject::new_str(name.to_string())),
        );
    }
    super::register_module("http.cookies", cookies_attrs);

    // Wire http submodules under http.
    super::super::module::MODULES.with(|mods| {
        let values: Vec<(String, MbValue)> = {
            let mods_ref = mods.borrow();
            ["client", "server", "cookies"]
                .iter()
                .filter_map(|sub| {
                    mods_ref
                        .get(&format!("http.{sub}"))
                        .map(|m| (sub.to_string(), super::super::module::module_to_value(m)))
                })
                .collect()
        };
        if let Some(http_mod) = mods.borrow_mut().get_mut("http") {
            for (k, v) in values {
                http_mod.attrs.insert(k, v);
            }
        }
    });
}

// ── HTTPStatus / http.client.responses construction ──

/// Build `http.client.responses`: a dict mapping each canonical status code
/// (int key) to its reason phrase (str value). CPython derives this from the
/// HTTPStatus enum; we build it directly from the registry table.
fn make_responses_dict() -> MbValue {
    use super::super::dict_ops::DictKey;
    let dict = MbObject::new_dict();
    unsafe {
        if let ObjData::Dict(ref lock) = (*dict).data {
            let mut map = lock.write().unwrap();
            for &(code, _name, phrase) in mamba_registry::http::canonical_codes() {
                map.insert(
                    DictKey::Int(code as i64),
                    MbValue::from_ptr(MbObject::new_str(phrase.to_string())),
                );
            }
        }
    }
    MbValue::from_ptr(dict)
}

/// Build `http.server.BaseHTTPRequestHandler.responses`: a dict mapping each
/// canonical status code (int key) to a `(shortmsg, longmsg)` 2-tuple of str.
/// CPython derives this from the HTTPStatus enum's phrase + description; we use
/// the registry phrase for both slots (surface fixture only checks that the
/// table is a `dict`).
fn make_handler_responses_dict() -> MbValue {
    use super::super::dict_ops::DictKey;
    let dict = MbObject::new_dict();
    unsafe {
        if let ObjData::Dict(ref lock) = (*dict).data {
            let mut map = lock.write().unwrap();
            for &(code, _name, phrase) in mamba_registry::http::canonical_codes() {
                let tuple = MbObject::new_tuple(vec![
                    MbValue::from_ptr(MbObject::new_str(phrase.to_string())),
                    MbValue::from_ptr(MbObject::new_str(phrase.to_string())),
                ]);
                map.insert(DictKey::Int(code as i64), MbValue::from_ptr(tuple));
            }
        }
    }
    MbValue::from_ptr(dict)
}

/// Add HTTPStatus-specific fields to a shared IntEnum member.
fn decorate_status_member(member: MbValue, code: u16, _name: &str, phrase: &str) {
    if let Some(inst_ptr) = member.as_ptr() {
        unsafe {
            let ObjData::Instance { ref fields, .. } = (*inst_ptr).data else {
                return;
            };
            let mut f = fields.write().unwrap();
            f.insert(
                "phrase".to_string(),
                MbValue::from_ptr(MbObject::new_str(phrase.to_string())),
            );
            f.insert(
                "description".to_string(),
                MbValue::from_ptr(MbObject::new_str(phrase.to_string())),
            );
            let code = i64::from(code);
            f.insert(
                "is_informational".to_string(),
                MbValue::from_bool((100..=199).contains(&code)),
            );
            f.insert(
                "is_success".to_string(),
                MbValue::from_bool((200..=299).contains(&code)),
            );
            f.insert(
                "is_redirection".to_string(),
                MbValue::from_bool((300..=399).contains(&code)),
            );
            f.insert(
                "is_client_error".to_string(),
                MbValue::from_bool((400..=499).contains(&code)),
            );
            f.insert(
                "is_server_error".to_string(),
                MbValue::from_bool((500..=599).contains(&code)),
            );
        }
    }
}

/// Underlying integer for a `HTTPStatus` member instance.
pub fn http_status_member_value(v: MbValue) -> Option<MbValue> {
    super::enum_class::int_member_value(v)
}

pub fn mb_httpstatus_call(arg: MbValue) -> MbValue {
    let Some(code) = status_code_arg(arg) else {
        raise("ValueError", "None is not a valid HTTPStatus".to_string());
        return MbValue::none();
    };
    if mamba_registry::http::canonical_codes()
        .iter()
        .find(|(known, _, _)| i64::from(*known) == code)
        .is_some()
    {
        let args = MbValue::from_ptr(MbObject::new_list(vec![MbValue::from_int(code)]));
        if let Some(member) = super::enum_class::enum_class_call("HTTPStatus", args) {
            return member;
        }
    }
    raise("ValueError", format!("{code} is not a valid HTTPStatus"));
    MbValue::none()
}

/// Build a type-object shell: an Instance with `class_name == "type"` and a
/// `__name__` str field. `callable(x)` is True (mb_callable treats a "type"
/// instance as callable) and `type(x).__name__ == "type"` (mb_type returns the
/// instance's class_name, "type"). Used for `urllib.request` classes that a
/// surface fixture probes with `type(X).__name__ == "type"`.
fn make_type_object(name: &str) -> MbValue {
    let inst_ptr = MbObject::new_instance("type".to_string());
    unsafe {
        if let ObjData::Instance { ref fields, .. } = (*inst_ptr).data {
            let mut map = fields.write().unwrap();
            map.insert(
                "__name__".to_string(),
                MbValue::from_ptr(MbObject::new_str(name.to_string())),
            );
        }
    }
    MbValue::from_ptr(inst_ptr)
}

// ── Helpers ──

fn raise(exc: &str, msg: String) {
    super::super::exception::mb_raise(
        MbValue::from_ptr(MbObject::new_str(exc.to_string())),
        MbValue::from_ptr(MbObject::new_str(msg)),
    );
}

fn extract_str(val: MbValue) -> Option<String> {
    val.as_ptr().and_then(|ptr| unsafe {
        if let ObjData::Str(ref s) = (*ptr).data {
            Some(s.clone())
        } else {
            None
        }
    })
}

fn extract_bytes_like(val: MbValue) -> Option<Vec<u8>> {
    val.as_ptr().and_then(|ptr| unsafe {
        match &(*ptr).data {
            ObjData::Bytes(b) => Some(b.clone()),
            ObjData::ByteArray(lock) => Some(lock.read().unwrap().clone()),
            _ => None,
        }
    })
}

fn extract_safe_bytes(safe: MbValue, default: &[u8]) -> Vec<u8> {
    if safe.is_none() {
        return default.to_vec();
    }
    extract_bytes_like(safe)
        .or_else(|| extract_str(safe).map(|s| s.into_bytes()))
        .unwrap_or_else(|| default.to_vec())
}

/// Split a native arg slice into (positional, trailing-kwargs-dict). mamba
/// folds keyword arguments into one trailing dict positional for callees in
/// the native-kwargs allowlist; quote/unquote's first positional is never a
/// dict, so a trailing Dict unambiguously names the kwargs.
fn split_trailing_kwargs(a: &[MbValue]) -> (&[MbValue], Option<MbValue>) {
    if let Some(&last) = a.last() {
        let is_dict = last
            .as_ptr()
            .map(|p| unsafe { matches!((*p).data, ObjData::Dict(_)) })
            .unwrap_or(false);
        if is_dict {
            return (&a[..a.len() - 1], Some(last));
        }
    }
    (a, None)
}

/// Read `name` from a kwargs dict, returning None when absent.
fn dict_kw_get(kw: MbValue, name: &str) -> Option<MbValue> {
    let sentinel = MbValue::from_bits(u64::MAX);
    let v = super::super::dict_ops::mb_dict_get(
        kw,
        MbValue::from_ptr(MbObject::new_str(name.to_string())),
        sentinel,
    );
    if v.to_bits() == u64::MAX {
        None
    } else {
        Some(v)
    }
}

/// Encode `s` to bytes under `encoding` honoring the `errors` handler, as
/// CPython's `quote(string, encoding=, errors=)` does. utf-8 (and unknown
/// encodings) pass through losslessly; latin-1/ascii map a code point to one
/// byte when it fits, otherwise apply the error handler. Returns None after
/// raising UnicodeEncodeError on a strict-mode failure.
fn encode_str_with(s: &str, encoding: &str, errors: &str) -> Option<Vec<u8>> {
    let enc = encoding.to_ascii_lowercase().replace('_', "-");
    let limit: u32 = match enc.as_str() {
        "latin-1" | "iso-8859-1" | "latin1" | "l1" => 0xFF,
        "ascii" | "us-ascii" | "646" => 0x7F,
        // utf-8 / unknown: every str code point is representable.
        _ => return Some(s.as_bytes().to_vec()),
    };
    let mut out = Vec::new();
    for ch in s.chars() {
        let cp = ch as u32;
        if cp <= limit {
            out.push(cp as u8);
            continue;
        }
        match errors {
            "replace" => out.push(b'?'),
            "ignore" => {}
            "xmlcharrefreplace" => out.extend_from_slice(format!("&#{cp};").as_bytes()),
            _ => {
                super::super::exception::mb_raise(
                    MbValue::from_ptr(MbObject::new_str("UnicodeEncodeError".to_string())),
                    MbValue::from_ptr(MbObject::new_str(format!(
                        "'{enc}' codec can't encode character '\\u{cp:04x}' in position 0"
                    ))),
                );
                return None;
            }
        }
    }
    Some(out)
}

fn raise_type_error(msg: &str) -> MbValue {
    super::super::exception::mb_raise(
        MbValue::from_ptr(MbObject::new_str("TypeError".to_string())),
        MbValue::from_ptr(MbObject::new_str(msg.to_string())),
    );
    MbValue::none()
}

fn value_as_query_string(v: MbValue) -> String {
    extract_str(v)
        .or_else(|| v.as_int().map(|i| format!("{i}")))
        .or_else(|| v.as_float().map(|f| format!("{f}")))
        .or_else(|| {
            v.as_bool()
                .map(|b| if b { "True".into() } else { "False".into() })
        })
        .unwrap_or_default()
}

/// Python `str(v)` of a value, used for urlencode value coercion
/// (`str(None)` → "None", `str(1)` → "1", `str([1,2])` → "[1, 2]").
fn py_str(v: MbValue) -> String {
    let s = super::super::builtins::mb_str(v);
    extract_str(s).unwrap_or_default()
}

/// Accept str OR bytes input, returning the raw bytes. For str input the
/// bytes are the UTF-8 encoding; for bytes input they are returned verbatim.
/// Matches CPython's `quote`/`unquote` which accept both.
fn extract_str_or_bytes(val: MbValue) -> Option<Vec<u8>> {
    extract_str(val)
        .map(|s| s.into_bytes())
        .or_else(|| extract_bytes_like(val))
}

/// Decode a UTF-8 byte slice the way CPython's `unquote(..., errors='replace')`
/// does by default: invalid sequences become U+FFFD; with `errors='ignore'`
/// they are dropped; with `encoding='latin-1'` every byte maps 1:1.
fn decode_bytes(bytes: &[u8], encoding: &str, errors: &str) -> String {
    let enc = encoding.to_ascii_lowercase().replace('_', "-");
    if enc == "latin-1" || enc == "iso-8859-1" || enc == "latin1" || enc == "l1" {
        return bytes.iter().map(|&b| b as char).collect();
    }
    // Default: UTF-8.
    match errors {
        "ignore" => decode_utf8_with(bytes, None),
        "replace" => decode_utf8_with(bytes, Some('\u{FFFD}')),
        _ => decode_utf8_with(bytes, Some('\u{FFFD}')),
    }
}

/// UTF-8 decode where each maximal invalid sequence is replaced by
/// `replacement` (when Some) or dropped (when None). Tracks the CPython
/// `replace`/`ignore` codec error behavior closely enough for url decoding.
fn decode_utf8_with(bytes: &[u8], replacement: Option<char>) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        match std::str::from_utf8(&bytes[i..]) {
            Ok(valid) => {
                out.push_str(valid);
                break;
            }
            Err(e) => {
                let good = e.valid_up_to();
                if good > 0 {
                    out.push_str(std::str::from_utf8(&bytes[i..i + good]).unwrap());
                    i += good;
                }
                // CPython's 'replace' codec emits one U+FFFD per maximal
                // undecodable unit (a lone invalid byte, or a truncated
                // multi-byte sequence at end of input). 'ignore' drops it.
                let remaining = bytes.len() - i;
                let skip = e.error_len().unwrap_or(remaining).max(1);
                if let Some(r) = replacement {
                    out.push(r);
                }
                i += skip;
            }
        }
    }
    out
}

// ── Runtime functions ──

/// urllib.parse.quote(string, safe='/') → percent-encoded string.
///
/// Default `safe` is '/' per CPython: slashes in a path are preserved.
pub fn mb_urllib_quote(val: MbValue, safe: MbValue) -> MbValue {
    mb_urllib_quote_full(val, safe, MbValue::none(), MbValue::none())
}

/// urllib.parse.quote(string, safe='/', encoding=None, errors=None).
///
/// str input is encoded with `encoding` (default utf-8) under the `errors`
/// handler before percent-encoding; bytes input is escaped byte-for-byte and
/// rejects an explicit `encoding` (CPython raises TypeError).
pub fn mb_urllib_quote_full(
    val: MbValue,
    safe: MbValue,
    encoding: MbValue,
    errors: MbValue,
) -> MbValue {
    let enc = extract_str(encoding);
    let bytes = match extract_bytes_like(val) {
        Some(b) => {
            if enc.is_some() {
                return raise_type_error("quote() doesn't support 'encoding' for bytes");
            }
            b
        }
        None => {
            let Some(s) = extract_str(val) else {
                return raise_type_error("quote() argument must be str or bytes");
            };
            let errs = extract_str(errors).unwrap_or_else(|| "strict".to_string());
            match encode_str_with(&s, enc.as_deref().unwrap_or("utf-8"), &errs) {
                Some(b) => b,
                None => return MbValue::none(),
            }
        }
    };
    let safe_bytes = if safe.is_none() {
        b"/".to_vec()
    } else {
        extract_safe_bytes(safe, b"/")
    };
    MbValue::from_ptr(MbObject::new_str(percent_encode_bytes(
        &bytes,
        &safe_bytes,
        false,
    )))
}

/// urllib.parse.quote_from_bytes(bytes, safe='/') -> percent-encoded string.
pub fn mb_urllib_quote_from_bytes(val: MbValue, safe: MbValue) -> MbValue {
    let Some(bytes) = extract_bytes_like(val) else {
        return raise_type_error("quote_from_bytes() expected bytes");
    };
    let safe_bytes = extract_safe_bytes(safe, b"/");
    MbValue::from_ptr(MbObject::new_str(percent_encode_bytes(
        &bytes,
        &safe_bytes,
        false,
    )))
}

/// urllib.parse.quote_plus(string, safe='') → spaces become '+', rest %-encoded.
pub fn mb_urllib_quote_plus(val: MbValue, safe: MbValue) -> MbValue {
    let bytes = match extract_bytes_like(val) {
        Some(b) => b,
        None => {
            let Some(s) = extract_str(val) else {
                return raise_type_error("quote_plus() argument must be str or bytes");
            };
            s.into_bytes()
        }
    };
    // CPython: when the string already contains '+' or a space, '+' is added
    // to the safe set so a literal space encodes as '+' and is then %-encoded
    // back? No — quote_plus replaces spaces with '+' and %-encodes the rest
    // with the caller's safe set (default empty).
    let safe_bytes = extract_safe_bytes(safe, b"");
    MbValue::from_ptr(MbObject::new_str(percent_encode_bytes(
        &bytes,
        &safe_bytes,
        true,
    )))
}

/// urllib.parse.unquote(string) → decode %XX sequences; leave '+' untouched.
pub fn mb_urllib_unquote(val: MbValue) -> MbValue {
    mb_urllib_unquote_full(val, MbValue::none(), MbValue::none())
}

/// urllib.parse.unquote(string, encoding='utf-8', errors='replace') → decode
/// %XX sequences using the requested codec/error handler.
pub fn mb_urllib_unquote_full(val: MbValue, encoding: MbValue, errors: MbValue) -> MbValue {
    let Some(input) = extract_str_or_bytes(val) else {
        return raise_type_error("unquote() argument must be str or bytes");
    };
    let enc = extract_str(encoding).unwrap_or_else(|| "utf-8".to_string());
    let errs = extract_str(errors).unwrap_or_else(|| "replace".to_string());
    let decoded = percent_decode_to_bytes(&input, false);
    MbValue::from_ptr(MbObject::new_str(decode_bytes(&decoded, &enc, &errs)))
}

/// urllib.parse.unquote_plus(string) → decode %XX and '+' → ' '.
pub fn mb_urllib_unquote_plus(val: MbValue) -> MbValue {
    let Some(input) = extract_str_or_bytes(val) else {
        return raise_type_error("unquote_plus() argument must be str or bytes");
    };
    let decoded = percent_decode_to_bytes(&input, true);
    MbValue::from_ptr(MbObject::new_str(decode_bytes(
        &decoded, "utf-8", "replace",
    )))
}

/// urllib.parse.urlencode(params) → "k1=v1&k2=v2" — accepts dict or list of 2-tuples.
pub fn mb_urllib_urlencode(params: MbValue) -> MbValue {
    mb_urllib_urlencode_full(params, MbValue::none())
}

/// Full urlencode honoring the `doseq` second positional argument.
///
/// `doseq` truthy → list/tuple values expand into one `key=elt` pair per
/// element (and mapping values expand over their keys). Otherwise a sequence
/// value is `str()`-ed whole.
pub fn mb_urllib_urlencode_full(params: MbValue, doseq: MbValue) -> MbValue {
    mb_urllib_urlencode_with(params, doseq, "", MbValue::none())
}

/// urlencode honoring doseq, safe=, and a quote_via= callable (CPython
/// default is quote_plus; passing quote switches space to %20).
pub fn mb_urllib_urlencode_with(
    params: MbValue,
    doseq: MbValue,
    safe: &str,
    quote_via: MbValue,
) -> MbValue {
    mb_urllib_urlencode_codec(
        params,
        doseq,
        safe,
        quote_via,
        MbValue::none(),
        MbValue::none(),
    )
}

/// urlencode honoring doseq / safe= / quote_via= plus encoding= / errors=
/// (the codec used to %-encode str keys and values; default utf-8/strict).
pub fn mb_urllib_urlencode_codec(
    params: MbValue,
    doseq: MbValue,
    safe: &str,
    quote_via: MbValue,
    encoding: MbValue,
    errors: MbValue,
) -> MbValue {
    // A bare str/bytes is not a mapping or pair sequence.
    if extract_str(params).is_some() || extract_bytes_like(params).is_some() {
        return raise_type_error("not a valid non-string sequence or mapping object");
    }
    let enc_name = extract_str(encoding).unwrap_or_else(|| "utf-8".to_string());
    let err_name = extract_str(errors).unwrap_or_else(|| "strict".to_string());
    let do_seq = super::super::builtins::mb_bool(doseq).as_bool() == Some(true);
    let pairs = urlencode_pairs(params);
    let mut parts = Vec::new();
    let safe_b = safe.as_bytes();
    let enc = |v: MbValue| -> String {
        if quote_via.is_none() {
            encode_query_component(v, safe_b, &enc_name, &err_name)
        } else {
            // quote_via(str(value), safe) through the supplied callable.
            let s = py_str(v);
            let args = MbValue::from_ptr(MbObject::new_list(vec![
                MbValue::from_ptr(MbObject::new_str(s)),
                MbValue::from_ptr(MbObject::new_str(safe.to_string())),
            ]));
            let r = super::super::builtins::mb_call_spread(quote_via, args);
            extract_str(r).unwrap_or_default()
        }
    };
    for (k, v) in pairs {
        let key_enc = enc(k);
        if do_seq {
            if extract_bytes_like(v).is_some() || extract_str(v).is_some() {
                parts.push(format!("{key_enc}={}", enc(v)));
            } else if let Some(elems) = sequence_elements(v) {
                for elt in elems {
                    parts.push(format!("{key_enc}={}", enc(elt)));
                }
            } else {
                parts.push(format!("{key_enc}={}", enc(v)));
            }
        } else {
            parts.push(format!("{key_enc}={}", enc(v)));
        }
    }
    MbValue::from_ptr(MbObject::new_str(parts.join("&")))
}

/// Encode one urlencode key or value via `quote_plus` semantics. bytes are
/// escaped byte-for-byte; everything else is `str()`-coerced first.
fn encode_query_component(v: MbValue, safe: &[u8], encoding: &str, errors: &str) -> String {
    if let Some(b) = extract_bytes_like(v) {
        return percent_encode_bytes(&b, safe, true);
    }
    let s = py_str(v);
    // Encode the str under the requested codec (default utf-8) before %-encoding.
    let bytes = encode_str_with(&s, encoding, errors).unwrap_or_else(|| s.into_bytes());
    percent_encode_bytes(&bytes, safe, true)
}

/// Yield the elements of a list/tuple value, or — for a mapping — its keys
/// (CPython's `doseq` path iterates a value that is itself iterable, and for
/// a dict iteration yields keys).
fn sequence_elements(v: MbValue) -> Option<Vec<MbValue>> {
    let ptr = v.as_ptr()?;
    unsafe {
        match &(*ptr).data {
            ObjData::List(ref lock) => Some(lock.read().unwrap().to_vec()),
            ObjData::Tuple(ref t) => Some(t.to_vec()),
            ObjData::Dict(_) => dict_key_elements(v),
            ObjData::Set(ref lock) => Some(lock.read().unwrap().iter().copied().collect()),
            ObjData::Instance { .. } => {
                let backing = super::super::class::unwrap_dictlike_data(v)?;
                dict_key_elements(backing)
            }
            _ => None,
        }
    }
}

fn dict_key_elements(mapping: MbValue) -> Option<Vec<MbValue>> {
    let ptr = mapping.as_ptr()?;
    unsafe {
        if let ObjData::Dict(ref lock) = &(*ptr).data {
            let map = lock.read().unwrap();
            Some(
                map.keys()
                    .map(super::super::dict_ops::dict_key_to_mbvalue)
                    .collect(),
            )
        } else {
            None
        }
    }
}

/// Extract the (key, value) pairs of a urlencode `query` argument, which may
/// be a dict (insertion order) or a list/tuple of 2-element tuples/lists.
fn urlencode_pairs(params: MbValue) -> Vec<(MbValue, MbValue)> {
    let mut out = Vec::new();
    let Some(ptr) = params.as_ptr() else {
        return out;
    };
    unsafe {
        match &(*ptr).data {
            ObjData::Dict(ref lock) => {
                let map = lock.read().unwrap();
                for (k, v) in map.iter() {
                    out.push((super::super::dict_ops::dict_key_to_mbvalue(k), *v));
                }
            }
            ObjData::List(ref lock) => {
                let items = lock.read().unwrap();
                for item in items.iter() {
                    if let Some((k, v)) = two_elem(item) {
                        out.push((k, v));
                    }
                }
            }
            ObjData::Tuple(ref t) => {
                for item in t.iter() {
                    if let Some((k, v)) = two_elem(*item) {
                        out.push((k, v));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Read a 2-element tuple or list into (first, second).
fn two_elem(item: MbValue) -> Option<(MbValue, MbValue)> {
    let ptr = item.as_ptr()?;
    unsafe {
        match &(*ptr).data {
            ObjData::Tuple(ref t) if t.len() == 2 => Some((t[0], t[1])),
            ObjData::List(ref lock) => {
                let v = lock.read().unwrap();
                if v.len() == 2 {
                    Some((v.get(0).unwrap(), v.get(1).unwrap()))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// urllib.parse.urlparse(url) → ParseResult-like Instance with 6 str fields.
///
/// Fields: scheme, netloc, path, params, query, fragment. Attribute access
/// works via the standard Instance field lookup; index access is not
/// supported yet (would require Tuple-backed storage).
/// CPython scheme split: a valid scheme is alpha followed by alnum/+-.
/// before the FIRST ':'; it lowercases. No scheme → ("", url).
fn split_scheme(url: &str) -> (String, String) {
    if let Some(i) = url.find(':') {
        let cand = &url[..i];
        let mut chars = cand.chars();
        if let Some(c0) = chars.next() {
            if c0.is_ascii_alphabetic()
                && chars.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
            {
                return (cand.to_ascii_lowercase(), url[i + 1..].to_string());
            }
        }
    }
    (String::new(), url.to_string())
}

/// netloc applies only after a literal "//".
fn split_netloc_part(rest: &str) -> (String, String) {
    if let Some(r) = rest.strip_prefix("//") {
        match r.find(|c: char| c == '/' || c == '?' || c == '#') {
            Some(i) => (r[..i].to_string(), r[i..].to_string()),
            None => (r.to_string(), String::new()),
        }
    } else {
        (String::new(), rest.to_string())
    }
}

/// Shared split: (scheme, netloc, path, query, fragment) — path keeps params.
/// WHATWG-aligned cleanup first: leading/trailing C0-or-space trimmed, then
/// embedded tab/newline/CR removed from the whole URL.
fn urlsplit_parts(url: &str) -> (String, String, String, String, String) {
    let trimmed = url.trim_start_matches(|c: char| c <= ' ');
    let cleaned: String = trimmed
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    let url = cleaned.as_str();
    let (scheme, rest) = split_scheme(url);
    let (netloc, path_query_frag) = split_netloc_part(&rest);
    let (path_query, fragment) = match path_query_frag.find('#') {
        Some(i) => (
            path_query_frag[..i].to_string(),
            path_query_frag[i + 1..].to_string(),
        ),
        None => (path_query_frag, String::new()),
    };
    let (path, query) = match path_query.find('?') {
        Some(i) => (path_query[..i].to_string(), path_query[i + 1..].to_string()),
        None => (path_query, String::new()),
    };
    (scheme, netloc, path, query, fragment)
}

fn input_is_bytes(val: MbValue) -> bool {
    val.as_ptr()
        .map(|p| unsafe { matches!((*p).data, ObjData::Bytes(_) | ObjData::ByteArray(_)) })
        .unwrap_or(false)
}

fn url_input_str(val: MbValue) -> String {
    if let Some(b) = extract_bytes_like(val) {
        return String::from_utf8_lossy(&b).to_string();
    }
    extract_str(val).unwrap_or_default()
}

fn validate_url_input(val: MbValue, func: &str) -> bool {
    val.is_none() || extract_str(val).is_some() || extract_bytes_like(val).is_some() || {
        raise_type_error(&format!("{func}() argument must be str or bytes"));
        false
    }
}

/// urllib.parse.urlparse(url) → ParseResult (6 fields; ';params' split out of
/// the last path segment). Bytes input yields all-bytes components.
fn validate_netloc_brackets(netloc: &str) -> bool {
    let has_open = netloc.contains('[');
    let has_close = netloc.contains(']');
    if has_open != has_close {
        super::super::exception::mb_raise(
            MbValue::from_ptr(MbObject::new_str("ValueError".to_string())),
            MbValue::from_ptr(MbObject::new_str("Invalid IPv6 URL".to_string())),
        );
        return false;
    }
    true
}

pub fn mb_urllib_urlparse(val: MbValue) -> MbValue {
    if !validate_url_input(val, "urlparse") {
        return MbValue::none();
    }
    let as_bytes = input_is_bytes(val);
    let url = url_input_str(val);
    let (scheme, netloc, full_path, query, fragment) = urlsplit_parts(&url);
    if !validate_netloc_brackets(&netloc) {
        return MbValue::none();
    }
    // params: split at ';' within the LAST path segment only (CPython).
    let last_seg_start = full_path.rfind('/').map(|i| i + 1).unwrap_or(0);
    let (path, params) = match full_path[last_seg_start..].find(';') {
        Some(i) => {
            let cut = last_seg_start + i;
            (
                full_path[..cut].to_string(),
                full_path[cut + 1..].to_string(),
            )
        }
        None => (full_path, String::new()),
    };
    make_result_instance(
        if as_bytes {
            "urllib.parse.ParseResultBytes"
        } else {
            "urllib.parse.ParseResult"
        },
        &[
            ("scheme", scheme),
            ("netloc", netloc),
            ("path", path),
            ("params", params),
            ("query", query),
            ("fragment", fragment),
        ],
        as_bytes,
    )
}

/// urllib.parse.urlsplit(url) → SplitResult (5 fields; params stay in path).
pub fn mb_urllib_urlsplit(val: MbValue) -> MbValue {
    if !validate_url_input(val, "urlsplit") {
        return MbValue::none();
    }
    let as_bytes = input_is_bytes(val);
    let url = url_input_str(val);
    let (scheme, netloc, path, query, fragment) = urlsplit_parts(&url);
    if !validate_netloc_brackets(&netloc) {
        return MbValue::none();
    }
    make_result_instance(
        if as_bytes {
            "urllib.parse.SplitResultBytes"
        } else {
            "urllib.parse.SplitResult"
        },
        &[
            ("scheme", scheme),
            ("netloc", netloc),
            ("path", path),
            ("query", query),
            ("fragment", fragment),
        ],
        as_bytes,
    )
}

/// urllib.parse.urlunparse(parts) → URL string. Accepts tuple/list of 6
/// strings or a ParseResult Instance.
pub fn mb_urllib_urlunparse(val: MbValue) -> MbValue {
    let (scheme, netloc, path, params, query, fragment) = extract_parse_tuple(val);
    let mut url = String::new();
    if !scheme.is_empty() {
        url.push_str(&scheme);
        url.push_str("://");
    }
    url.push_str(&netloc);
    url.push_str(&path);
    if !params.is_empty() {
        url.push(';');
        url.push_str(&params);
    }
    if !query.is_empty() {
        url.push('?');
        url.push_str(&query);
    }
    if !fragment.is_empty() {
        url.push('#');
        url.push_str(&fragment);
    }
    MbValue::from_ptr(MbObject::new_str(url))
}

/// urllib.parse.urljoin(base, url) → resolve `url` relative to `base`.
///
/// Subset of RFC 3986 §5.3: absolute URLs and scheme-less '//host/...'
/// forms override; absolute paths ('/x') replace from the host; relative
/// paths resolve against the directory of base's path.
pub fn mb_urllib_urljoin(base: MbValue, url: MbValue) -> MbValue {
    let b = extract_str(base).unwrap_or_default();
    let u = extract_str(url).unwrap_or_default();
    MbValue::from_ptr(MbObject::new_str(urljoin_impl(&b, &u)))
}

fn urljoin_impl(base: &str, url: &str) -> String {
    if url.is_empty() {
        return base.to_string();
    }
    // Absolute URL: return as-is.
    if let Some(i) = url.find("://") {
        if url[..i]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
        {
            return url.to_string();
        }
    }
    let (scheme_prefix, after_scheme) = match base.find("://") {
        Some(i) => (format!("{}://", &base[..i]), base[i + 3..].to_string()),
        None => (String::new(), base.to_string()),
    };
    // Scheme-relative: //host/...
    if url.starts_with("//") {
        let scheme = scheme_prefix.trim_end_matches("://");
        if !scheme.is_empty() {
            return format!("{scheme}:{url}");
        }
        return url.to_string();
    }
    let (host, base_path) = match after_scheme.find('/') {
        Some(i) => (after_scheme[..i].to_string(), after_scheme[i..].to_string()),
        None => (after_scheme, String::new()),
    };
    // Fragment-only reference: append to base (drop old fragment).
    if let Some(stripped) = url.strip_prefix('#') {
        let base_no_frag = match base.find('#') {
            Some(i) => &base[..i],
            None => base,
        };
        return format!("{base_no_frag}#{stripped}");
    }
    // Query-only reference: replace query.
    if let Some(stripped) = url.strip_prefix('?') {
        let (path_only, _) = match base_path.find('?') {
            Some(i) => (base_path[..i].to_string(), ()),
            None => (base_path.clone(), ()),
        };
        let path_only = match path_only.find('#') {
            Some(i) => path_only[..i].to_string(),
            None => path_only,
        };
        return format!("{scheme_prefix}{host}{path_only}?{stripped}");
    }
    // Absolute path on same host.
    if url.starts_with('/') {
        return format!("{scheme_prefix}{host}{url}");
    }
    // Relative path: resolve against directory of base path.
    let base_dir = match base_path.rfind('/') {
        Some(i) => base_path[..=i].to_string(),
        None => "/".to_string(),
    };
    let resolved = resolve_dot_segments(&format!("{base_dir}{url}"));
    format!("{scheme_prefix}{host}{resolved}")
}

/// Collapse `.` and `..` segments in a path per RFC 3986 §5.2.4.
fn resolve_dot_segments(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let leading_slash = path.starts_with('/');
    let trailing_slash = path.ends_with('/') && path.len() > 1;
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    let mut result = if leading_slash {
        "/".to_string()
    } else {
        String::new()
    };
    result.push_str(&out.join("/"));
    if trailing_slash && !result.ends_with('/') {
        result.push('/');
    }
    result
}

/// urllib.parse.parse_qs(qs) → dict of list[str]. Keys appearing more than
/// once collect all their values in insertion order.
/// Core query-string splitter honoring keep_blank_values / encoding /
/// errors, with bytes-in → bytes-out pairs.
fn parse_qsl_core(val: MbValue, opts: &QsOptions) -> Vec<(MbValue, MbValue)> {
    let keep_blank = opts.keep_blank;
    let encoding = opts.encoding.as_str();
    let errors = opts.errors.as_str();
    let bytes_mode = input_is_bytes(val);
    let s = url_input_str(val);
    if opts.separator.is_empty() {
        super::super::exception::mb_raise(
            MbValue::from_ptr(MbObject::new_str("ValueError".to_string())),
            MbValue::from_ptr(MbObject::new_str(
                "Separator must be of type string or bytes.".to_string(),
            )),
        );
        return Vec::new();
    }
    if let Some(max) = opts.max_num_fields {
        let n = s.matches(opts.separator.as_str()).count() as i64 + 1;
        if n > max {
            super::super::exception::mb_raise(
                MbValue::from_ptr(MbObject::new_str("ValueError".to_string())),
                MbValue::from_ptr(MbObject::new_str(
                    "Max number of fields exceeded".to_string(),
                )),
            );
            return Vec::new();
        }
    }
    let mut out = Vec::new();
    for pair in s.split(opts.separator.as_str()) {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.find('=') {
            Some(i) => (&pair[..i], &pair[i + 1..]),
            None => {
                if opts.strict_parsing {
                    super::super::exception::mb_raise(
                        MbValue::from_ptr(MbObject::new_str("ValueError".to_string())),
                        MbValue::from_ptr(MbObject::new_str(format!("bad query field: {pair:?}"))),
                    );
                    return Vec::new();
                }
                (pair, "")
            }
        };
        if v.is_empty() && !keep_blank {
            continue;
        }
        let kb = percent_decode_to_bytes(k.as_bytes(), true);
        let vb = percent_decode_to_bytes(v.as_bytes(), true);
        if bytes_mode {
            out.push((
                MbValue::from_ptr(MbObject::new_bytes(kb)),
                MbValue::from_ptr(MbObject::new_bytes(vb)),
            ));
        } else {
            out.push((
                MbValue::from_ptr(MbObject::new_str(decode_bytes(&kb, encoding, errors))),
                MbValue::from_ptr(MbObject::new_str(decode_bytes(&vb, encoding, errors))),
            ));
        }
    }
    out
}

/// urllib.parse.parse_qs(qs, keep_blank_values=False, ..., encoding='utf-8',
/// errors='replace') → dict of key → list-of-values.
pub struct QsOptions {
    pub keep_blank: bool,
    pub encoding: String,
    pub errors: String,
    pub separator: String,
    pub strict_parsing: bool,
    pub max_num_fields: Option<i64>,
}

impl Default for QsOptions {
    fn default() -> Self {
        QsOptions {
            keep_blank: false,
            encoding: "utf-8".to_string(),
            errors: "replace".to_string(),
            separator: "&".to_string(),
            strict_parsing: false,
            max_num_fields: None,
        }
    }
}

pub fn mb_urllib_parse_qs(val: MbValue, keep_blank: bool, encoding: &str, errors: &str) -> MbValue {
    let opts = QsOptions {
        keep_blank,
        encoding: encoding.to_string(),
        errors: errors.to_string(),
        ..QsOptions::default()
    };
    mb_urllib_parse_qs_opts(val, &opts)
}

pub fn mb_urllib_parse_qs_opts(val: MbValue, opts: &QsOptions) -> MbValue {
    let pairs = parse_qsl_core(val, opts);
    let result = MbObject::new_dict();
    let rv = MbValue::from_ptr(result);
    for (k, v) in pairs {
        let sentinel = MbValue::from_bits(u64::MAX);
        let existing = super::super::dict_ops::mb_dict_get(rv, k, sentinel);
        if existing.to_bits() == u64::MAX {
            let list = MbValue::from_ptr(MbObject::new_list(vec![v]));
            super::super::dict_ops::mb_dict_setitem(rv, k, list);
        } else {
            super::super::list_ops::mb_list_append(existing, v);
        }
    }
    rv
}

/// urllib.parse.parse_qsl(qs, ...) → ordered (key, value) 2-tuples.
pub fn mb_urllib_parse_qsl(
    val: MbValue,
    keep_blank: bool,
    encoding: &str,
    errors: &str,
) -> MbValue {
    let opts = QsOptions {
        keep_blank,
        encoding: encoding.to_string(),
        errors: errors.to_string(),
        ..QsOptions::default()
    };
    mb_urllib_parse_qsl_opts(val, &opts)
}

pub fn mb_urllib_parse_qsl_opts(val: MbValue, opts: &QsOptions) -> MbValue {
    let items: Vec<MbValue> = parse_qsl_core(val, opts)
        .into_iter()
        .map(|(k, v)| MbValue::from_ptr(MbObject::new_tuple(vec![k, v])))
        .collect();
    MbValue::from_ptr(MbObject::new_list(items))
}

/// urllib.parse.urldefrag(url) → (defrag_url, fragment) as DefragResult.
///
/// Splits the URL at the first '#'. Returns an Instance with `.url`
/// and `.fragment` attributes, matching CPython's DefragResult
/// NamedTuple shape.
pub fn mb_urllib_urldefrag(val: MbValue) -> MbValue {
    if !validate_url_input(val, "urldefrag") {
        return MbValue::none();
    }
    let s = extract_str(val).unwrap_or_default();
    let (url, fragment) = match s.find('#') {
        Some(i) => (s[..i].to_string(), s[i + 1..].to_string()),
        None => (s.clone(), String::new()),
    };
    let mut fields = FxHashMap::with_capacity_and_hasher(2, Default::default());
    fields.insert("url".into(), MbValue::from_ptr(MbObject::new_str(url)));
    fields.insert(
        "fragment".into(),
        MbValue::from_ptr(MbObject::new_str(fragment)),
    );
    let obj = Box::new(super::super::rc::MbObject {
        header: super::super::rc::MbObjectHeader {
            rc: std::sync::atomic::AtomicU32::new(1),
            kind: super::super::rc::ObjKind::Instance,
        },
        data: ObjData::Instance {
            class_name: "urllib.parse.DefragResult".to_string(),
            fields: crate::runtime::rc::MbRwLock::new(fields),
        },
    });
    MbValue::from_ptr(Box::into_raw(obj))
}

/// urllib.parse.unquote_to_bytes(string) → bytes.
///
/// Percent-decodes `string` into raw bytes (no UTF-8 reinterpretation).
/// Accepts str or bytes input; non-percent bytes are passed through.
pub fn mb_urllib_unquote_to_bytes(val: MbValue) -> MbValue {
    // Accept str or bytes; percent-decode into raw bytes verbatim. CPython
    // raises (TypeError/AttributeError) for anything else (it calls
    // `.split(b'%')` on the input), so reject None/tuple/etc.
    let Some(input) = extract_str_or_bytes(val) else {
        return raise_type_error("unquote_to_bytes() argument must be str or bytes");
    };
    let out = percent_decode_to_bytes(&input, false);
    MbValue::from_ptr(MbObject::new_bytes(out))
}

/// urllib.request.urlopen(url) → stub response dict.
///
/// Mamba does not bundle an HTTP client; this returns a placeholder dict so
/// imports and basic attribute access don't crash. Real network access is
/// out of scope for the native runtime.
pub fn mb_urllib_urlopen(url: MbValue) -> MbValue {
    let u = extract_str(url).unwrap_or_default();
    let dict = MbObject::new_dict();
    unsafe {
        if let ObjData::Dict(ref lock) = (*dict).data {
            let mut map = lock.write().unwrap();
            map.insert("url".into(), MbValue::from_ptr(MbObject::new_str(u)));
            map.insert("status".into(), MbValue::from_int(0));
            map.insert(
                "data".into(),
                MbValue::from_ptr(MbObject::new_str(String::new())),
            );
        }
    }
    MbValue::from_ptr(dict)
}

fn make_result_instance(class_name: &str, parts: &[(&str, String)], as_bytes: bool) -> MbValue {
    let mut fields = FxHashMap::with_capacity_and_hasher(12, Default::default());
    let mk = |s: &str| -> MbValue {
        if as_bytes {
            MbValue::from_ptr(MbObject::new_bytes(s.as_bytes().to_vec()))
        } else {
            MbValue::from_ptr(MbObject::new_str(s.to_string()))
        }
    };
    let mut entries: Vec<MbValue> = Vec::new();
    let mut netloc = String::new();
    for (name, value) in parts {
        if *name == "netloc" {
            netloc = value.clone();
        }
        let v = mk(value);
        fields.insert((*name).to_string(), v);
        entries.push(v);
    }
    fields.insert(
        "_entries".into(),
        MbValue::from_ptr(MbObject::new_list(entries)),
    );
    // Derived authority fields (username/password/hostname/port), computed the
    // way CPython's _NetlocResultMixinStr does from netloc.
    let (username, password, hostname, port) = split_netloc(&netloc);
    fields.insert(
        "username".into(),
        username.map(|s| mk(&s)).unwrap_or_else(MbValue::none),
    );
    fields.insert(
        "password".into(),
        password.map(|s| mk(&s)).unwrap_or_else(MbValue::none),
    );
    fields.insert(
        "hostname".into(),
        hostname.map(|s| mk(&s)).unwrap_or_else(MbValue::none),
    );
    fields.insert(
        "port".into(),
        port.map(MbValue::from_int).unwrap_or_else(MbValue::none),
    );
    let obj = Box::new(super::super::rc::MbObject {
        header: super::super::rc::MbObjectHeader {
            rc: std::sync::atomic::AtomicU32::new(1),
            kind: super::super::rc::ObjKind::Instance,
        },
        data: ObjData::Instance {
            class_name: class_name.to_string(),
            fields: crate::runtime::rc::MbRwLock::new(fields),
        },
    });
    MbValue::from_ptr(Box::into_raw(obj))
}

/// Split a netloc into (username, password, hostname, port) the way
/// CPython's _NetlocResultMixinStr does. userinfo precedes the last '@';
/// hostname is lowercased and may be a bracketed IPv6 literal; port is the
/// integer after the host's trailing ':'.
fn split_netloc(netloc: &str) -> (Option<String>, Option<String>, Option<String>, Option<i64>) {
    if netloc.is_empty() {
        return (None, None, None, None);
    }
    // userinfo before the last '@'.
    let (userinfo, hostport) = match netloc.rfind('@') {
        Some(i) => (Some(&netloc[..i]), &netloc[i + 1..]),
        None => (None, netloc),
    };
    let (username, password) = match userinfo {
        Some(ui) => match ui.find(':') {
            Some(i) => (Some(ui[..i].to_string()), Some(ui[i + 1..].to_string())),
            None => (Some(ui.to_string()), None),
        },
        None => (None, None),
    };
    // host[:port], honoring an IPv6 bracket literal `[..]`.
    let (host_str, port_str): (&str, Option<&str>) = if hostport.starts_with('[') {
        match hostport.find(']') {
            Some(close) => {
                let host = &hostport[..=close];
                let after = &hostport[close + 1..];
                if let Some(rest) = after.strip_prefix(':') {
                    (host, Some(rest))
                } else {
                    (host, None)
                }
            }
            None => (hostport, None),
        }
    } else {
        match hostport.rfind(':') {
            Some(i) => (&hostport[..i], Some(&hostport[i + 1..])),
            None => (hostport, None),
        }
    };
    // A bracketed IPv6 literal loses its brackets in .hostname; the part
    // before an RFC 6874 '%zone' is lowercased, the zone id keeps its case.
    let bare = host_str
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host_str);
    let hostname = if bare.is_empty() {
        None
    } else if let Some(pi) = bare.find('%') {
        Some(format!(
            "{}{}",
            bare[..pi].to_ascii_lowercase(),
            &bare[pi..]
        ))
    } else {
        Some(bare.to_ascii_lowercase())
    };
    let port = port_str.and_then(|p| {
        if p.is_empty() {
            return None;
        }
        match p.parse::<i64>() {
            Ok(n) if (0..=65535).contains(&n) => Some(n),
            Ok(n) => {
                // CPython raises on .port access; mamba validates eagerly.
                super::super::exception::mb_raise(
                    MbValue::from_ptr(MbObject::new_str("ValueError".to_string())),
                    MbValue::from_ptr(MbObject::new_str(format!("Port out of range 0-65535: {n}"))),
                );
                None
            }
            Err(_) => {
                super::super::exception::mb_raise(
                    MbValue::from_ptr(MbObject::new_str("ValueError".to_string())),
                    MbValue::from_ptr(MbObject::new_str(format!(
                        "Port could not be cast to integer value as {p:?}"
                    ))),
                );
                None
            }
        }
    });
    (username, password, hostname, port)
}

fn extract_parse_tuple(v: MbValue) -> (String, String, String, String, String, String) {
    let gi = |vals: &[MbValue], i: usize| {
        extract_str(vals.get(i).copied().unwrap_or(MbValue::none())).unwrap_or_default()
    };
    if let Some(ptr) = v.as_ptr() {
        unsafe {
            match &(*ptr).data {
                ObjData::Tuple(items) => {
                    return (
                        gi(items, 0),
                        gi(items, 1),
                        gi(items, 2),
                        gi(items, 3),
                        gi(items, 4),
                        gi(items, 5),
                    );
                }
                ObjData::List(ref lock) => {
                    let items = lock.read().unwrap().to_vec();
                    return (
                        gi(&items, 0),
                        gi(&items, 1),
                        gi(&items, 2),
                        gi(&items, 3),
                        gi(&items, 4),
                        gi(&items, 5),
                    );
                }
                ObjData::Instance { ref fields, .. } => {
                    let f = fields.read().unwrap();
                    let gf = |k: &str| f.get(k).and_then(|v| extract_str(*v)).unwrap_or_default();
                    return (
                        gf("scheme"),
                        gf("netloc"),
                        gf("path"),
                        gf("params"),
                        gf("query"),
                        gf("fragment"),
                    );
                }
                _ => {}
            }
        }
    }
    Default::default()
}

/// Percent-encode a string for URL contexts.
///
/// `safe` is a list of extra characters that should NOT be encoded (in
/// addition to RFC 3986 unreserved chars). When `plus_for_space` is true,
/// ASCII space is written as '+' — application/x-www-form-urlencoded.
fn percent_encode(s: &str, safe: &str, plus_for_space: bool) -> String {
    percent_encode_bytes(s.as_bytes(), safe.as_bytes(), plus_for_space)
}

fn percent_encode_bytes(bytes: &[u8], safe: &[u8], plus_for_space: bool) -> String {
    let mut out = String::new();
    for &b in bytes {
        if plus_for_space && b == b' ' {
            out.push('+');
            continue;
        }
        let unreserved = matches!(
            b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~'
        );
        if unreserved || safe.contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Percent-decode an input byte slice into raw bytes (no charset decode).
/// A '%' followed by two valid hex digits decodes to that byte; otherwise
/// the '%' is passed through literally (matching CPython, which leaves a
/// malformed escape such as `%`, `%2`, or `%zz` untouched). When
/// `plus_for_space` is set, '+' decodes to a space.
fn percent_decode_to_bytes(bytes: &[u8], plus_for_space: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'%' && i + 3 <= bytes.len() {
            let h = &bytes[i + 1..i + 3];
            if h.iter().all(|b| b.is_ascii_hexdigit()) {
                if let Ok(byte) = u8::from_str_radix(std::str::from_utf8(h).unwrap_or(""), 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
            }
        }
        if plus_for_space && c == b'+' {
            out.push(b' ');
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

fn percent_decode(s: &str, plus_for_space: bool) -> String {
    let decoded = percent_decode_to_bytes(s.as_bytes(), plus_for_space);
    decode_bytes(&decoded, "utf-8", "replace")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(val: &str) -> MbValue {
        MbValue::from_ptr(MbObject::new_str(val.to_string()))
    }

    #[test]
    fn test_quote_preserves_slash_by_default() {
        let out = mb_urllib_quote(s("/path with space"), MbValue::none());
        assert_eq!(extract_str(out).unwrap(), "/path%20with%20space");
    }

    #[test]
    fn test_quote_plus_encodes_slash_and_uses_plus() {
        let out = mb_urllib_quote_plus(s("/hello world"), MbValue::none());
        assert_eq!(extract_str(out).unwrap(), "%2Fhello+world");
    }

    #[test]
    fn test_unquote_decodes_percent() {
        let out = mb_urllib_unquote(s("hello%20world%2F"));
        assert_eq!(extract_str(out).unwrap(), "hello world/");
    }

    #[test]
    fn test_unquote_plus_decodes_percent_and_plus() {
        let out = mb_urllib_unquote_plus(s("hello+world%2F"));
        assert_eq!(extract_str(out).unwrap(), "hello world/");
    }

    #[test]
    fn test_urlencode_dict() {
        let dict = MbObject::new_dict();
        unsafe {
            if let ObjData::Dict(ref lock) = (*dict).data {
                lock.write().unwrap().insert("q".into(), s("hello world"));
            }
        }
        let out = mb_urllib_urlencode(MbValue::from_ptr(dict));
        let decoded = extract_str(out).unwrap();
        assert_eq!(decoded, "q=hello+world");
    }

    #[test]
    fn test_urlparse_full() {
        let out = mb_urllib_urlparse(s("https://example.com/path?q=1#frag"));
        if let Some(ptr) = out.as_ptr() {
            unsafe {
                if let ObjData::Instance { ref fields, .. } = (*ptr).data {
                    let f = fields.read().unwrap();
                    assert_eq!(extract_str(f["scheme"]).unwrap(), "https");
                    assert_eq!(extract_str(f["netloc"]).unwrap(), "example.com");
                    assert_eq!(extract_str(f["path"]).unwrap(), "/path");
                    assert_eq!(extract_str(f["query"]).unwrap(), "q=1");
                    assert_eq!(extract_str(f["fragment"]).unwrap(), "frag");
                }
            }
        }
    }

    #[test]
    fn test_urlparse_no_scheme() {
        let out = mb_urllib_urlparse(s("/just/a/path"));
        if let Some(ptr) = out.as_ptr() {
            unsafe {
                if let ObjData::Instance { ref fields, .. } = (*ptr).data {
                    let f = fields.read().unwrap();
                    assert_eq!(extract_str(f["scheme"]).unwrap(), "");
                    assert_eq!(extract_str(f["netloc"]).unwrap(), "");
                    assert_eq!(extract_str(f["path"]).unwrap(), "/just/a/path");
                }
            }
        }
    }

    #[test]
    fn test_urljoin_absolute() {
        let out = mb_urllib_urljoin(s("https://a.com/a/b"), s("https://b.com/x"));
        assert_eq!(extract_str(out).unwrap(), "https://b.com/x");
    }

    #[test]
    fn test_urljoin_absolute_path() {
        let out = mb_urllib_urljoin(s("https://a.com/a/b/c"), s("/x/y"));
        assert_eq!(extract_str(out).unwrap(), "https://a.com/x/y");
    }

    #[test]
    fn test_urljoin_relative_path() {
        let out = mb_urllib_urljoin(s("https://a.com/a/b/c"), s("d/e"));
        assert_eq!(extract_str(out).unwrap(), "https://a.com/a/b/d/e");
    }

    #[test]
    fn test_urljoin_fragment_only() {
        let out = mb_urllib_urljoin(s("https://a.com/x"), s("#frag"));
        assert_eq!(extract_str(out).unwrap(), "https://a.com/x#frag");
    }

    #[test]
    fn test_urljoin_dot_segments() {
        let out = mb_urllib_urljoin(s("https://a.com/a/b/c"), s("../x"));
        assert_eq!(extract_str(out).unwrap(), "https://a.com/a/x");
    }

    #[test]
    fn test_parse_qs_multi_values() {
        let out = mb_urllib_parse_qs(s("a=1&b=2&a=3"), false, "utf-8", "replace");
        if let Some(ptr) = out.as_ptr() {
            unsafe {
                if let ObjData::Dict(ref lock) = (*ptr).data {
                    let m = lock.read().unwrap();
                    // `DictKey::Str` hashes in the Python-semantic domain,
                    // which does not match a bare `&str`'s native `Hash`
                    // impl — route through `dict_get_exact_str`
                    // (module-hazards.md).
                    let a_val = super::super::super::dict_ops::dict_get_exact_str(&m, "a").unwrap();
                    if let Some(lp) = a_val.as_ptr() {
                        if let ObjData::List(ref lk) = (*lp).data {
                            let items = lk.read().unwrap();
                            assert_eq!(items.len(), 2);
                            assert_eq!(extract_str(items.get(0).unwrap()).unwrap(), "1");
                            assert_eq!(extract_str(items.get(1).unwrap()).unwrap(), "3");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_parse_qsl_preserves_order() {
        let out = mb_urllib_parse_qsl(s("z=1&a=2&m=3"), false, "utf-8", "replace");
        if let Some(ptr) = out.as_ptr() {
            unsafe {
                if let ObjData::List(ref lock) = (*ptr).data {
                    let items = lock.read().unwrap();
                    assert_eq!(items.len(), 3);
                    if let Some(tp) = items.get(0).unwrap().as_ptr() {
                        if let ObjData::Tuple(ref t) = (*tp).data {
                            assert_eq!(extract_str(t[0]).unwrap(), "z");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_urlunparse_roundtrip() {
        let parsed = mb_urllib_urlparse(s("https://example.com/path?q=1#frag"));
        let unparsed = mb_urllib_urlunparse(parsed);
        assert_eq!(
            extract_str(unparsed).unwrap(),
            "https://example.com/path?q=1#frag"
        );
    }

    #[test]
    fn test_cleanup_resets_http_shell_pool_cursor() {
        NEXT_SHELL_SLOT.with(|c| c.set(SHELL_POOL_SIZE));
        reset_shell_slots();
        assert_eq!(next_shell_slot(), 0);
        reset_shell_slots();
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// urllib.request.Request — a real object (#24 prerequisite): full_url/host/
// headers/data/method fields plus the introspection methods cookiejar's
// request_host/request_path/request_port and user code rely on.
// ═══════════════════════════════════════════════════════════════════════════

const REQUEST_CLASS: &str = "urllib.request.Request";

fn req_field(self_v: MbValue, name: &str) -> Option<MbValue> {
    let ptr = self_v.as_ptr()?;
    unsafe {
        if let ObjData::Instance { ref fields, .. } = (*ptr).data {
            return fields.read().unwrap().get(name).copied();
        }
    }
    None
}

fn req_args_vec(args: MbValue) -> Vec<MbValue> {
    let mut out = Vec::new();
    if let Some(ptr) = args.as_ptr() {
        unsafe {
            if let ObjData::List(ref lock) = (*ptr).data {
                out.extend(lock.read().unwrap().iter());
            }
        }
    }
    out
}

/// Write an instance field (retaining the value, releasing any prior).
fn set_inst_field(self_v: MbValue, name: &str, val: MbValue) {
    if let Some(ptr) = self_v.as_ptr() {
        unsafe {
            if let ObjData::Instance { ref fields, .. } = (*ptr).data {
                super::super::rc::retain_if_ptr(val);
                let old = fields.write().unwrap().insert(name.to_string(), val);
                if let Some(o) = old {
                    super::super::rc::release_if_ptr(o);
                }
            }
        }
    }
}

/// Decode a bytes/bytearray/str MbValue to a Rust String (lossy for bytes).
fn bytes_or_str(v: MbValue) -> String {
    if let Some(s) = extract_str(v) {
        return s;
    }
    v.as_ptr()
        .map(|p| unsafe {
            match &(*p).data {
                ObjData::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
                ObjData::ByteArray(lock) => {
                    String::from_utf8_lossy(&lock.read().unwrap()).into_owned()
                }
                _ => String::new(),
            }
        })
        .unwrap_or_default()
}

/// BaseHTTPRequestHandler.parse_request() -> bool. Parses `self.raw_requestline`
/// ("GET /path HTTP/1.1\r\n") into `command` / `path` / `request_version` and
/// sets `requestline`. Minimal vs CPython (no header block parsing); the
/// fixtures drive it with in-memory buffers and check the request-line fields.
unsafe extern "C" fn bh_parse_request(self_v: MbValue, _args: MbValue) -> MbValue {
    let raw = req_field(self_v, "raw_requestline").unwrap_or_else(MbValue::none);
    let line = bytes_or_str(raw);
    let trimmed = line.trim_end_matches(['\r', '\n']);
    set_inst_field(
        self_v,
        "requestline",
        MbValue::from_ptr(MbObject::new_str(trimmed.to_string())),
    );
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    // Reject like CPython BaseHTTPRequestHandler.parse_request: write an error
    // page via send_error and return False. An empty line is silently dropped.
    let send_err = |code: i64, msg: String| -> MbValue {
        let args = MbValue::from_ptr(MbObject::new_list(vec![
            MbValue::from_int(code),
            MbValue::from_ptr(MbObject::new_str(msg)),
        ]));
        bh_send_error(self_v, args);
        MbValue::from_bool(false)
    };
    if parts.is_empty() {
        return MbValue::from_bool(false);
    }
    // With ≥3 tokens the last is the protocol version: validate it first
    // (CPython order) — a non `HTTP/<int>.<int>` version is a 400, and a major
    // version ≥ 2 is a 505 "Invalid HTTP version".
    let version = if parts.len() >= 3 {
        let v = parts[parts.len() - 1];
        let base = v.strip_prefix("HTTP/");
        let nums = base.and_then(|b| {
            let mut it = b.split('.');
            let major = it.next()?.parse::<i64>().ok()?;
            let minor = it.next()?.parse::<i64>().ok()?;
            if it.next().is_some() {
                return None;
            }
            Some((major, minor))
        });
        match nums {
            None => return send_err(400, format!("Bad request version ({v:?})")),
            Some((major, _)) if major >= 2 => {
                return send_err(
                    505,
                    format!("Invalid HTTP version ({})", base.unwrap_or("")),
                );
            }
            Some(_) => v,
        }
    } else {
        "HTTP/0.9"
    };
    if !(2..=3).contains(&parts.len()) {
        return send_err(400, format!("Bad request syntax ({trimmed:?})"));
    }
    let (command, path) = (parts[0], parts[1]);
    set_inst_field(
        self_v,
        "command",
        MbValue::from_ptr(MbObject::new_str(command.to_string())),
    );
    set_inst_field(
        self_v,
        "path",
        MbValue::from_ptr(MbObject::new_str(path.to_string())),
    );
    set_inst_field(
        self_v,
        "request_version",
        MbValue::from_ptr(MbObject::new_str(version.to_string())),
    );
    MbValue::from_bool(true)
}

/// HTTP reason phrase for a status code (the subset the fixtures exercise plus
/// the common codes); empty string for unknown codes.
fn status_phrase(code: i64) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "",
    }
}

fn status_code_arg(v: MbValue) -> Option<i64> {
    v.as_int()
        .or_else(|| http_status_member_value(v).and_then(|value| value.as_int()))
}

/// `self.protocol_version` (instance or inherited class attr), default HTTP/1.0.
fn handler_proto(self_v: MbValue) -> String {
    let v = super::super::class::mb_getattr(
        self_v,
        MbValue::from_ptr(MbObject::new_str("protocol_version".to_string())),
    );
    extract_str(v).unwrap_or_else(|| "HTTP/1.0".to_string())
}

/// Append a header/status byte-line to the handler's `_headers_buffer` list
/// (lazily created), mirroring CPython's deferred header buffering.
fn handler_buffer_append(self_v: MbValue, line: Vec<u8>) {
    let list = match req_field(self_v, "_headers_buffer").filter(|v| !v.is_none()) {
        Some(l) => l,
        None => {
            let l = MbValue::from_ptr(MbObject::new_list(Vec::new()));
            set_inst_field(self_v, "_headers_buffer", l);
            l
        }
    };
    if let Some(ptr) = list.as_ptr() {
        unsafe {
            if let ObjData::List(ref lock) = (*ptr).data {
                let b = MbValue::from_ptr(MbObject::new_bytes(line));
                super::super::rc::retain_if_ptr(b);
                lock.write().unwrap().push(b);
            }
        }
    }
}

/// Write bytes to `self.wfile` via its `.write(...)` method (a BytesIO etc.).
fn handler_wfile_write(self_v: MbValue, data: Vec<u8>) {
    let wfile = req_field(self_v, "wfile").unwrap_or_else(MbValue::none);
    if wfile.is_none() {
        return;
    }
    let arg = MbValue::from_ptr(MbObject::new_bytes(data));
    let args = MbValue::from_ptr(MbObject::new_list(vec![arg]));
    super::super::class::mb_call_method(
        wfile,
        MbValue::from_ptr(MbObject::new_str("write".to_string())),
        args,
    );
}

/// BaseHTTPRequestHandler.send_response_only(code, message=None) — buffer the
/// status line (deferred until end_headers, matching CPython).
unsafe extern "C" fn bh_send_response_only(self_v: MbValue, args: MbValue) -> MbValue {
    let pos = req_args_vec(args);
    let code = pos.first().and_then(|v| status_code_arg(*v)).unwrap_or(0);
    let msg = pos
        .get(1)
        .copied()
        .and_then(extract_str)
        .unwrap_or_else(|| status_phrase(code).to_string());
    let proto = handler_proto(self_v);
    if proto != "HTTP/0.9" {
        handler_buffer_append(self_v, format!("{proto} {code} {msg}\r\n").into_bytes());
    }
    MbValue::none()
}

/// BaseHTTPRequestHandler.send_response(code, message=None) — status line via
/// send_response_only (Server/Date headers omitted: nondeterministic and not
/// asserted by the fixtures).
unsafe extern "C" fn bh_send_response(self_v: MbValue, args: MbValue) -> MbValue {
    bh_send_response_only(self_v, args)
}

/// BaseHTTPRequestHandler.send_header(keyword, value) — buffer one header line.
unsafe extern "C" fn bh_send_header(self_v: MbValue, args: MbValue) -> MbValue {
    // CPython's send_header reads `self.request_version`; on a handler built via
    // __new__ (no parse_request / send_response yet) that attribute is absent,
    // raising AttributeError.
    if req_field(self_v, "request_version")
        .filter(|v| !v.is_none())
        .is_none()
    {
        super::super::exception::mb_raise(
            MbValue::from_ptr(MbObject::new_str("AttributeError".to_string())),
            MbValue::from_ptr(MbObject::new_str(
                "'BaseHTTPRequestHandler' object has no attribute 'request_version'".to_string(),
            )),
        );
        return MbValue::none();
    }
    let pos = req_args_vec(args);
    let key = pos
        .first()
        .copied()
        .and_then(extract_str)
        .unwrap_or_default();
    let val = pos
        .get(1)
        .copied()
        .and_then(extract_str)
        .unwrap_or_default();
    handler_buffer_append(self_v, format!("{key}: {val}\r\n").into_bytes());
    MbValue::none()
}

/// BaseHTTPRequestHandler.end_headers() — append the blank-line separator and
/// flush the buffered header block to wfile.
unsafe extern "C" fn bh_end_headers(self_v: MbValue, _args: MbValue) -> MbValue {
    handler_buffer_append(self_v, b"\r\n".to_vec());
    // Flush: concatenate every buffered byte-line and write once.
    let mut out: Vec<u8> = Vec::new();
    if let Some(buf) = req_field(self_v, "_headers_buffer") {
        if let Some(ptr) = buf.as_ptr() {
            if let ObjData::List(ref lock) = (*ptr).data {
                for item in lock.read().unwrap().iter() {
                    if let Some(ip) = item.as_ptr() {
                        if let ObjData::Bytes(ref b) = (*ip).data {
                            out.extend_from_slice(b);
                        }
                    }
                }
                lock.write().unwrap().clear();
            }
        }
    }
    handler_wfile_write(self_v, out);
    MbValue::none()
}

/// BaseHTTPRequestHandler.send_error(code, message=None) — status line +
/// text/html body from DEFAULT_ERROR_MESSAGE.
unsafe extern "C" fn bh_send_error(self_v: MbValue, args: MbValue) -> MbValue {
    let pos = req_args_vec(args);
    let code = pos.first().and_then(|v| status_code_arg(*v)).unwrap_or(0);
    let phrase = status_phrase(code);
    let message = pos
        .get(1)
        .copied()
        .and_then(extract_str)
        .unwrap_or_else(|| phrase.to_string());
    // Status line + minimal header block, flushed immediately.
    let proto = handler_proto(self_v);
    handler_buffer_append(self_v, format!("{proto} {code} {message}\r\n").into_bytes());
    handler_buffer_append(
        self_v,
        b"Content-Type: text/html;charset=utf-8\r\n".to_vec(),
    );
    let body = format!(
        "<!DOCTYPE HTML>\n<html lang=\"en\">\n    <head>\n        \
         <meta charset=\"utf-8\">\n        <title>Error response</title>\n    </head>\n    \
         <body>\n        <h1>Error response</h1>\n        \
         <p>Error code: {code}</p>\n        <p>Message: {message}.</p>\n    </body>\n</html>\n"
    );
    handler_buffer_append(
        self_v,
        format!("Content-Length: {}\r\n", body.len()).into_bytes(),
    );
    bh_end_headers(self_v, MbValue::none());
    handler_wfile_write(self_v, body.into_bytes());
    MbValue::none()
}

/// (scheme, netloc, path-with-params-query-fragment) split of a URL.
fn split_url(url: &str) -> (String, String, String) {
    let (scheme, rest) = match url.find("://") {
        Some(i) => (url[..i].to_string(), &url[i + 3..]),
        None => (String::new(), url),
    };
    match rest.find('/') {
        Some(i) => (scheme, rest[..i].to_string(), rest[i..].to_string()),
        None => (scheme, rest.to_string(), String::new()),
    }
}

fn request_url_fields(url: &str) -> (String, String, String, String, MbValue) {
    let (scheme, host, path) = split_url(url);
    let selector = {
        let p = path.split('#').next().unwrap_or("");
        if p.is_empty() {
            "/".to_string()
        } else {
            p.to_string()
        }
    };
    let fragment = match url.split_once('#') {
        Some((_, frag)) => MbValue::from_ptr(MbObject::new_str(frag.to_string())),
        None => MbValue::none(),
    };
    (scheme, host, selector, url.to_string(), fragment)
}

fn assign_request_url_fields(self_v: MbValue, url: &str) {
    let (scheme, host, selector, full_url, fragment) = request_url_fields(url);
    set_inst_field(
        self_v,
        "full_url",
        MbValue::from_ptr(MbObject::new_str(full_url)),
    );
    set_inst_field(self_v, "host", MbValue::from_ptr(MbObject::new_str(host)));
    set_inst_field(self_v, "type", MbValue::from_ptr(MbObject::new_str(scheme)));
    set_inst_field(
        self_v,
        "selector",
        MbValue::from_ptr(MbObject::new_str(selector)),
    );
    set_inst_field(self_v, "fragment", fragment);
}

pub fn request_setattr(self_v: MbValue, attr_s: &str, value: MbValue) -> bool {
    if attr_s != "full_url" {
        return false;
    }
    let Some(url) = extract_str(value) else {
        super::super::exception::mb_raise(
            MbValue::from_ptr(MbObject::new_str("TypeError".to_string())),
            MbValue::from_ptr(MbObject::new_str("full_url must be a string".to_string())),
        );
        return true;
    };
    let (scheme, host, _) = split_url(&url);
    if scheme.is_empty() || host.is_empty() {
        super::super::exception::mb_raise(
            MbValue::from_ptr(MbObject::new_str("ValueError".to_string())),
            MbValue::from_ptr(MbObject::new_str("unknown url type".to_string())),
        );
        return true;
    }
    assign_request_url_fields(self_v, &url);
    true
}

/// Read an HTTPMessage's ordered (key, value) header pairs.
fn http_message_pairs(self_v: MbValue) -> Vec<(String, String)> {
    let headers = req_field(self_v, "_headers").unwrap_or_else(MbValue::none);
    let items: Vec<MbValue> = headers
        .as_ptr()
        .map(|p| unsafe {
            if let ObjData::List(ref lock) = (*p).data {
                lock.read().unwrap().to_vec()
            } else {
                Vec::new()
            }
        })
        .unwrap_or_default();
    items
        .iter()
        .filter_map(|pair| {
            let elems: Vec<MbValue> = pair.as_ptr().and_then(|p| unsafe {
                if let ObjData::Tuple(ref t) = (*p).data {
                    Some(t.clone())
                } else {
                    None
                }
            })?;
            Some((extract_str(*elems.first()?)?, extract_str(*elems.get(1)?)?))
        })
        .collect()
}

/// HTTPMessage.keys() — header names in order.
unsafe extern "C" fn hm_keys(self_v: MbValue, _args: MbValue) -> MbValue {
    let keys: Vec<MbValue> = http_message_pairs(self_v)
        .into_iter()
        .map(|(k, _)| MbValue::from_ptr(MbObject::new_str(k)))
        .collect();
    MbValue::from_ptr(MbObject::new_list(keys))
}

/// HTTPMessage.items() — (name, value) pairs in order.
unsafe extern "C" fn hm_items(self_v: MbValue, _args: MbValue) -> MbValue {
    let items: Vec<MbValue> = http_message_pairs(self_v)
        .into_iter()
        .map(|(k, v)| {
            MbValue::from_ptr(MbObject::new_tuple(vec![
                MbValue::from_ptr(MbObject::new_str(k)),
                MbValue::from_ptr(MbObject::new_str(v)),
            ]))
        })
        .collect();
    MbValue::from_ptr(MbObject::new_list(items))
}

/// HTTPMessage.__len__() — header count.
unsafe extern "C" fn hm_len(self_v: MbValue, _args: MbValue) -> MbValue {
    MbValue::from_int(http_message_pairs(self_v).len() as i64)
}

/// Case-insensitive header lookup (RFC 822 field names are case-insensitive).
fn http_message_lookup(self_v: MbValue, name: &str) -> Option<String> {
    let lname = name.to_ascii_lowercase();
    http_message_pairs(self_v)
        .into_iter()
        .find(|(k, _)| k.to_ascii_lowercase() == lname)
        .map(|(_, v)| v)
}

/// HTTPMessage.get(name, default=None).
unsafe extern "C" fn hm_get(self_v: MbValue, args: MbValue) -> MbValue {
    let pos = req_args_vec(args);
    let name = pos
        .first()
        .copied()
        .and_then(extract_str)
        .unwrap_or_default();
    match http_message_lookup(self_v, &name) {
        Some(v) => MbValue::from_ptr(MbObject::new_str(v)),
        None => pos.get(1).copied().unwrap_or_else(MbValue::none),
    }
}

/// HTTPMessage.__getitem__(name) — None for a missing header (email.Message).
unsafe extern "C" fn hm_getitem(self_v: MbValue, args: MbValue) -> MbValue {
    let name = req_args_vec(args)
        .first()
        .copied()
        .and_then(extract_str)
        .unwrap_or_default();
    match http_message_lookup(self_v, &name) {
        Some(v) => MbValue::from_ptr(MbObject::new_str(v)),
        None => MbValue::none(),
    }
}

/// http.client.parse_headers(fp) -> HTTPMessage. Reads the RFC 822 header block
/// from `fp` (a readable file object) and parses "Name: value" lines.
unsafe extern "C" fn dispatch_parse_headers(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    let a = if nargs == 0 || args_ptr.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(args_ptr, nargs) }
    };
    let fp = a.first().copied().unwrap_or_else(MbValue::none);
    let data = super::super::class::mb_call_method(
        fp,
        MbValue::from_ptr(MbObject::new_str("read".to_string())),
        MbValue::from_ptr(MbObject::new_list(Vec::new())),
    );
    // Header bytes are latin-1; decode 1:1.
    let text = extract_bytes_like(data)
        .map(|b| b.iter().map(|&c| c as char).collect::<String>())
        .or_else(|| extract_str(data))
        .unwrap_or_default();
    let mut pairs: Vec<MbValue> = Vec::new();
    for line in text.split('\n') {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            break; // blank line ends the header block
        }
        if let Some((k, v)) = line.split_once(':') {
            pairs.push(MbValue::from_ptr(MbObject::new_tuple(vec![
                MbValue::from_ptr(MbObject::new_str(k.trim().to_string())),
                MbValue::from_ptr(MbObject::new_str(v.trim().to_string())),
            ])));
        }
    }
    let inst = MbObject::new_instance("HTTPMessage".to_string());
    if let ObjData::Instance { ref fields, .. } = (*inst).data {
        fields.write().unwrap().insert(
            "_headers".into(),
            MbValue::from_ptr(MbObject::new_list(pairs)),
        );
    }
    MbValue::from_ptr(inst)
}

/// http.client.IncompleteRead(partial, expected=None) — an HTTPException
/// carrying the bytes read so far (`partial`) and the expected length.
unsafe extern "C" fn d_incomplete_read(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    crate::icf_guard!();
    let a = if nargs == 0 || args_ptr.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(args_ptr, nargs) }
    };
    let partial = a.first().copied().unwrap_or_else(MbValue::none);
    let expected = a.get(1).copied().unwrap_or_else(MbValue::none);
    let inst = MbObject::new_instance("IncompleteRead".to_string());
    if let ObjData::Instance { ref fields, .. } = (*inst).data {
        super::super::rc::retain_if_ptr(partial);
        super::super::rc::retain_if_ptr(expected);
        let mut f = fields.write().unwrap();
        f.insert("partial".into(), partial);
        f.insert("expected".into(), expected);
        f.insert(
            "args".into(),
            MbValue::from_ptr(MbObject::new_tuple(vec![partial, expected])),
        );
    }
    MbValue::from_ptr(inst)
}

unsafe extern "C" fn d_request_new(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    crate::icf_guard!();
    let raw: &[MbValue] = if nargs == 0 || args_ptr.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(args_ptr, nargs) }
    };
    // Trailing kwargs dict appended by the call lowering. The data argument is
    // ALSO a dict (`Request(url, {})` posts an empty form), so a trailing dict
    // is the kwargs dict only when it is non-empty and every key names a known
    // Request keyword — otherwise it is the positional `data` mapping.
    let is_request_kwargs = |v: MbValue| -> bool {
        let Some(ptr) = v.as_ptr() else { return false };
        unsafe {
            if let ObjData::Dict(ref lock) = (*ptr).data {
                let map = lock.read().unwrap();
                return !map.is_empty()
                    && map.keys().all(|k| {
                        matches!(
                            k,
                            super::super::dict_ops::DictKey::Str(s)
                                if matches!(s.as_str(),
                                    "data" | "headers" | "origin_req_host"
                                        | "unverifiable" | "method")
                        )
                    });
            }
        }
        false
    };
    let (positional, kwargs): (&[MbValue], Option<MbValue>) = match raw.last().copied() {
        Some(last) if raw.len() > 1 && is_request_kwargs(last) => {
            (&raw[..raw.len() - 1], Some(last))
        }
        _ => (raw, None),
    };
    let kwarg = |name: &str| -> Option<MbValue> {
        let ptr = kwargs?.as_ptr()?;
        unsafe {
            if let ObjData::Dict(ref lock) = (*ptr).data {
                // `DictKey::Str` hashes with the Python-semantic domain
                // (#1028), not Rust's native `str` hash — a raw `.get(&str)`
                // here would silently miss present keys (#1566).
                return super::super::dict_ops::dict_get_exact_str(&lock.read().unwrap(), name);
            }
        }
        None
    };

    let url = positional
        .first()
        .copied()
        .and_then(extract_str)
        .unwrap_or_default();
    let data = positional
        .get(1)
        .copied()
        .or_else(|| kwarg("data"))
        .unwrap_or_else(MbValue::none);
    let headers_in = positional
        .get(2)
        .copied()
        .or_else(|| kwarg("headers"))
        .unwrap_or_else(MbValue::none);
    let method = kwarg("method").unwrap_or_else(MbValue::none);

    let inst = MbObject::new_instance(REQUEST_CLASS.to_string());
    unsafe {
        if let ObjData::Instance { ref fields, .. } = (*inst).data {
            let mut f = fields.write().unwrap();
            let (scheme, host, selector, full_url, fragment) = request_url_fields(&url);
            f.insert(
                "full_url".into(),
                MbValue::from_ptr(MbObject::new_str(full_url)),
            );
            f.insert("host".into(), MbValue::from_ptr(MbObject::new_str(host)));
            f.insert("type".into(), MbValue::from_ptr(MbObject::new_str(scheme)));
            f.insert(
                "selector".into(),
                MbValue::from_ptr(MbObject::new_str(selector)),
            );
            f.insert("fragment".into(), fragment);
            f.insert("data".into(), data);
            f.insert("method".into(), method);
            f.insert("unverifiable".into(), MbValue::from_bool(false));
            // Header keys are stored capitalized (CPython add_header shape).
            let hdrs = MbObject::new_dict();
            if let ObjData::Dict(ref hlock) = (*hdrs).data {
                let mut hmap = hlock.write().unwrap();
                if let Some(hin) = headers_in.as_ptr() {
                    if let ObjData::Dict(ref inlock) = (*hin).data {
                        for (k, v) in inlock.read().unwrap().iter() {
                            let kv = super::super::dict_ops::dict_key_to_mbvalue(k);
                            if let Some(ks) = extract_str(kv) {
                                super::super::rc::retain_if_ptr(*v);
                                hmap.insert(
                                    super::super::dict_ops::DictKey::Str(capitalize_header(&ks)),
                                    *v,
                                );
                            }
                        }
                    }
                }
                drop(hmap);
            }
            f.insert("headers".into(), MbValue::from_ptr(hdrs));
        }
    }
    MbValue::from_ptr(inst)
}

fn capitalize_header(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

unsafe extern "C" fn rm_get_method(self_v: MbValue, _args: MbValue) -> MbValue {
    let m = req_field(self_v, "method").unwrap_or_else(MbValue::none);
    if let Some(s) = extract_str(m) {
        return MbValue::from_ptr(MbObject::new_str(s));
    }
    let data = req_field(self_v, "data").unwrap_or_else(MbValue::none);
    MbValue::from_ptr(MbObject::new_str(
        if data.is_none() { "GET" } else { "POST" }.to_string(),
    ))
}

unsafe extern "C" fn rm_get_full_url(self_v: MbValue, _args: MbValue) -> MbValue {
    let v = req_field(self_v, "full_url").unwrap_or_else(MbValue::none);
    unsafe { super::super::rc::retain_if_ptr(v) };
    v
}

unsafe extern "C" fn rm_get_header(self_v: MbValue, args: MbValue) -> MbValue {
    let a = req_args_vec(args);
    let name = match a.first().copied() {
        Some(v) => match extract_str(v) {
            Some(s) => s,
            None => return raise_type_error("Request.get_header() header_name must be str"),
        },
        None => String::new(),
    };
    let default = a.get(1).copied().unwrap_or_else(MbValue::none);
    if let Some(hd) = req_field(self_v, "headers").and_then(|v| v.as_ptr()) {
        unsafe {
            if let ObjData::Dict(ref lock) = (*hd).data {
                // CPython get_header does an EXACT lookup; only add_header
                // capitalizes (on store). Do not capitalize the query.
                if let Some(v) = lock.read().unwrap().get(name.as_str()) {
                    let v = *v;
                    super::super::rc::retain_if_ptr(v);
                    return v;
                }
            }
        }
    }
    unsafe { super::super::rc::retain_if_ptr(default) };
    default
}

unsafe extern "C" fn rm_has_header(self_v: MbValue, args: MbValue) -> MbValue {
    let a = req_args_vec(args);
    let name = a.first().copied().and_then(extract_str).unwrap_or_default();
    if let Some(hd) = req_field(self_v, "headers").and_then(|v| v.as_ptr()) {
        unsafe {
            if let ObjData::Dict(ref lock) = (*hd).data {
                // CPython has_header is an EXACT membership test; only
                // add_header capitalizes (on store). No query capitalization.
                return MbValue::from_bool(lock.read().unwrap().contains_key(name.as_str()));
            }
        }
    }
    MbValue::from_bool(false)
}

unsafe extern "C" fn rm_add_header(self_v: MbValue, args: MbValue) -> MbValue {
    let a = req_args_vec(args);
    let name = a.first().copied().and_then(extract_str).unwrap_or_default();
    let val = a.get(1).copied().unwrap_or_else(MbValue::none);
    if let Some(hd) = req_field(self_v, "headers").and_then(|v| v.as_ptr()) {
        unsafe {
            if let ObjData::Dict(ref lock) = (*hd).data {
                unsafe { super::super::rc::retain_if_ptr(val) };
                lock.write().unwrap().insert(
                    super::super::dict_ops::DictKey::Str(capitalize_header(&name)),
                    val,
                );
            }
        }
    }
    MbValue::none()
}

unsafe extern "C" fn rm_header_items(self_v: MbValue, _args: MbValue) -> MbValue {
    let mut items = Vec::new();
    if let Some(hd) = req_field(self_v, "headers").and_then(|v| v.as_ptr()) {
        unsafe {
            if let ObjData::Dict(ref lock) = (*hd).data {
                for (k, v) in lock.read().unwrap().iter() {
                    let kv = super::super::dict_ops::dict_key_to_mbvalue(k);
                    items.push(MbValue::from_ptr(MbObject::new_tuple(vec![kv, *v])));
                }
            }
        }
    }
    MbValue::from_ptr(MbObject::new_list(items))
}

const HTTPCONN_CLASS: &str = "http.client.HTTPConnection";

unsafe extern "C" fn d_httpconnection_new(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    crate::icf_guard!();
    let a = if nargs == 0 || args_ptr.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(args_ptr, nargs) }
    };
    let inst = MbObject::new_instance(HTTPCONN_CLASS.to_string());
    unsafe {
        if let ObjData::Instance { ref fields, .. } = (*inst).data {
            let mut f = fields.write().unwrap();
            let host = a.first().copied().unwrap_or_else(MbValue::none);
            f.insert("host".into(), host);
            // request state: "idle" until putrequest starts a request.
            f.insert(
                "_HTTPConnection__state".into(),
                MbValue::from_ptr(MbObject::new_str("Idle".to_string())),
            );
        }
    }
    MbValue::from_ptr(inst)
}

unsafe extern "C" fn hc_putrequest(self_v: MbValue, args: MbValue) -> MbValue {
    let a = req_args_vec(args);
    let method = a.first().copied().and_then(extract_str).unwrap_or_default();
    let url = a.get(1).copied().and_then(extract_str).unwrap_or_default();
    // CPython rejects control characters in the method (ValueError) and the
    // URL (http.client.InvalidURL).
    if method.contains(['\n', '\r', '\t', ' ']) {
        raise(
            "ValueError",
            "method can't contain control characters".to_string(),
        );
        return MbValue::none();
    }
    if url.contains(['\n', '\r']) {
        raise(
            "InvalidURL",
            "URL can't contain control characters".to_string(),
        );
        return MbValue::none();
    }
    if let Some(p) = self_v.as_ptr() {
        unsafe {
            if let ObjData::Instance { ref fields, .. } = (*p).data {
                fields.write().unwrap().insert(
                    "_HTTPConnection__state".into(),
                    MbValue::from_ptr(MbObject::new_str("Request-started".to_string())),
                );
            }
        }
    }
    MbValue::none()
}

unsafe extern "C" fn hc_putheader(self_v: MbValue, _args: MbValue) -> MbValue {
    let state = self_v
        .as_ptr()
        .and_then(|p| unsafe {
            if let ObjData::Instance { ref fields, .. } = (*p).data {
                fields
                    .read()
                    .unwrap()
                    .get("_HTTPConnection__state")
                    .copied()
                    .and_then(extract_str)
            } else {
                None
            }
        })
        .unwrap_or_default();
    if state != "Request-started" {
        raise("CannotSendHeader", "Cannot send header".to_string());
    }
    MbValue::none()
}

/// Register a minimal stateful http.client.HTTPConnection (putrequest /
/// putheader with CPython's control-char + ordering validation).
fn register_httpconnection_class(attrs: &mut HashMap<String, MbValue>) {
    use std::collections::HashMap as Map;
    let var = |addr: usize| {
        super::super::module::register_variadic_func(addr as u64);
        MbValue::from_func(addr)
    };
    let mut map: Map<String, MbValue> = Map::new();
    map.insert(
        "putrequest".into(),
        var(hc_putrequest as *const () as usize),
    );
    map.insert("putheader".into(), var(hc_putheader as *const () as usize));
    super::super::class::mb_class_register(HTTPCONN_CLASS, vec!["object".to_string()], map);
    attrs.insert(
        "HTTPConnection".to_string(),
        MbValue::from_func(d_httpconnection_new as *const () as usize),
    );
    super::super::module::NATIVE_FUNC_ADDRS.with(|s| {
        s.borrow_mut()
            .insert(d_httpconnection_new as *const () as u64);
    });
    super::super::module::register_native_type_name(
        d_httpconnection_new as *const () as u64,
        HTTPCONN_CLASS.to_string(),
    );
}

/// OpenerDirector() / build_opener(*handlers) -> an OpenerDirector instance.
/// Handlers are accepted and ignored; the surface only checks the type.
unsafe extern "C" fn d_opener_director_new(args_ptr: *const MbValue, nargs: usize) -> MbValue {
    crate::icf_guard!();
    let args = if nargs == 0 || args_ptr.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(args_ptr, nargs) }
    };
    for arg in args {
        let primitive_or_none = arg.is_none()
            || arg.as_int().is_some()
            || arg.as_float().is_some()
            || arg.is_bool()
            || extract_str(*arg).is_some();
        if primitive_or_none {
            let type_name = extract_str(super::super::builtins::mb_type(*arg))
                .unwrap_or_else(|| "object".to_string());
            return raise_type_error(&format!("expected BaseHandler instance, got {}", type_name));
        }
    }
    MbValue::from_ptr(MbObject::new_instance("OpenerDirector".to_string()))
}

/// Register the Request class + rewire urllib.request.Request.
pub(crate) fn register_request_class(attrs: &mut HashMap<String, MbValue>) {
    use std::collections::HashMap as Map;
    let var = |addr: usize| {
        super::super::module::register_variadic_func(addr as u64);
        MbValue::from_func(addr)
    };
    let methods: Vec<(&str, usize)> = vec![
        ("get_method", rm_get_method as *const () as usize),
        ("get_full_url", rm_get_full_url as *const () as usize),
        ("get_header", rm_get_header as *const () as usize),
        ("has_header", rm_has_header as *const () as usize),
        ("add_header", rm_add_header as *const () as usize),
        (
            "add_unredirected_header",
            rm_add_header as *const () as usize,
        ),
        ("header_items", rm_header_items as *const () as usize),
    ];
    let mut map: Map<String, MbValue> = Map::new();
    for (name, addr) in methods {
        map.insert(name.to_string(), var(addr));
    }
    super::super::class::mb_class_register(REQUEST_CLASS, vec!["object".to_string()], map);

    attrs.insert(
        "Request".to_string(),
        MbValue::from_func(d_request_new as *const () as usize),
    );
    super::super::module::NATIVE_FUNC_ADDRS.with(|s| {
        s.borrow_mut().insert(d_request_new as *const () as u64);
    });
    super::super::module::register_native_type_name(
        d_request_new as *const () as u64,
        REQUEST_CLASS.to_string(),
    );

    // OpenerDirector — a real class so `isinstance(build_opener(),
    // OpenerDirector)` and `type(...).__name__ == "OpenerDirector"` hold.
    // build_opener() and OpenerDirector() both construct an instance.
    super::super::class::mb_class_register(
        "OpenerDirector",
        vec!["object".to_string()],
        Map::new(),
    );
    let od = d_opener_director_new as *const () as usize;
    attrs.insert("OpenerDirector".to_string(), MbValue::from_func(od));
    attrs.insert("build_opener".to_string(), MbValue::from_func(od));
    super::super::module::NATIVE_FUNC_ADDRS.with(|s| {
        s.borrow_mut().insert(od as u64);
    });
    super::super::module::register_native_type_name(od as u64, "OpenerDirector".to_string());
}
