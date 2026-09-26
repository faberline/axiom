// HANDWRITE-BEGIN reason: mambalibs-http-binding-runtime-surface until the generator can emit native object methods, module values, and decorator callbacks (primitive: mamba-native-method-binding).
//! Mamba interface for the native httpkit API toolkit.

use mamba_registry::{
    convert::{mb_unwrap_native_mut, mb_unwrap_native_ref, mb_wrap_native_typed, native_type_name},
    ops, rt_sym, FromMbValue, MbValue, ModuleRegistrar, RuntimeValue,
};
use schema_mamba::methods::{
    mb_schema_model_dump_json, model_dump_json_from_json_text, model_validation_detail_json,
    model_validation_detail_json_from_json_text,
};
use schema_mamba::types::MbBaseModel;
use mambalibs_di::ProviderKey;
use mambalibs_di_binding::{MbDiContainer, MbDiScope};
pub use mambalibs_http::app::{
    App, BackgroundTask, BackgroundTasks, CORSMiddleware, Endpoint, RequestContext, RouteParameter,
    Router, StaticFiles, StreamingResponse,
};
use serde_json::{Map, Number, Value as JsonValue};
use std::cell::RefCell;
use std::collections::HashMap;

type NativeFn = unsafe extern "C" fn(*const MbValue, usize) -> MbValue;

#[derive(Clone, Copy)]
struct BoundRouteTarget {
    receiver: MbValue,
    method: &'static str,
}

#[derive(Clone)]
struct PendingRoute {
    receiver: MbValue,
    method: &'static str,
    path: String,
    status_code: u16,
    dependency_keys: Vec<String>,
    parameters: Vec<RouteParameter>,
    request_model: Option<ModelMetadata>,
    response_model: Option<ModelMetadata>,
}

#[derive(Debug, Clone)]
struct ModelMetadata {
    name: String,
    schema_json: Option<String>,
    native_handle: Option<MbValue>,
}

thread_local! {
    static ACTIVE_ROUTE_TARGET: RefCell<Option<BoundRouteTarget>> = const { RefCell::new(None) };
    static PENDING_ROUTE: RefCell<Option<PendingRoute>> = const { RefCell::new(None) };
    static REQUEST_MODEL_HANDLES: RefCell<HashMap<(u64, String, String), MbValue>> = RefCell::new(HashMap::new());
    static RESPONSE_MODEL_HANDLES: RefCell<HashMap<(u64, String, String), MbValue>> = RefCell::new(HashMap::new());
    static ROUTE_HANDLERS: RefCell<HashMap<(u64, String, String), MbValue>> = RefCell::new(HashMap::new());
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#param
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    Depends,
    Query,
    Body,
    Header,
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#param
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub kind: ParamKind,
    pub default: MbValue,
    pub default_provided: bool,
    pub name: Option<String>,
    pub alias: Option<String>,
    pub description: Option<String>,
    pub required: bool,
    pub dependency_key: Option<ProviderKey>,
}

unsafe fn read(args: *const MbValue, nargs: usize, index: usize) -> MbValue {
    if index < nargs {
        *args.add(index)
    } else {
        MbValue::none()
    }
}

unsafe fn read_string(args: *const MbValue, nargs: usize, index: usize) -> Option<String> {
    String::from_mb_value(read(args, nargs, index)).ok()
}

unsafe fn read_string_list(args: *const MbValue, nargs: usize, index: usize) -> Vec<String> {
    Vec::<String>::from_mb_value(read(args, nargs, index)).unwrap_or_default()
}

unsafe fn read_string_map(
    args: *const MbValue,
    nargs: usize,
    index: usize,
) -> HashMap<String, String> {
    HashMap::<String, String>::from_mb_value(read(args, nargs, index)).unwrap_or_default()
}

unsafe fn read_status_code(args: *const MbValue, nargs: usize, index: usize) -> u16 {
    i64::from_mb_value(read(args, nargs, index))
        .ok()
        .and_then(|v| u16::try_from(v).ok())
        .unwrap_or(200)
}

fn read_status_code_value(value: MbValue) -> Option<u16> {
    i64::from_mb_value(value)
        .ok()
        .and_then(|v| u16::try_from(v).ok())
}

fn read_string_value(value: MbValue) -> Option<String> {
    String::from_mb_value(value).ok()
}

fn kwargs_get(kwargs: Option<MbValue>, key: &str) -> Option<MbValue> {
    kwargs.and_then(|kw| (ops().dict_get_str)(kw, key))
}

unsafe fn trailing_kwargs(args: *const MbValue, nargs: usize) -> (usize, Option<MbValue>) {
    if nargs == 0 {
        return (0, None);
    }
    let last = unsafe { read(args, nargs, nargs - 1) };
    if (ops().dict_iter_str_items)(last).is_some() {
        (nargs - 1, Some(last))
    } else {
        (nargs, None)
    }
}

fn dependency_key_from_value(value: MbValue) -> Option<String> {
    match native_type_name(value) {
        Some("Depends") => {
            let param = unsafe { mb_unwrap_native_ref::<Param>(value) }?;
            param
                .dependency_key
                .as_ref()
                .map(|key| key.as_str().to_string())
        }
        _ => read_string_value(value).and_then(non_empty_string),
    }
}

fn dependency_keys_from_value(value: MbValue) -> Vec<String> {
    if value.is_none() {
        return Vec::new();
    }
    if let Some(key) = dependency_key_from_value(value) {
        return vec![key];
    }
    let Some(len) = (ops().list_len)(value) else {
        return Vec::new();
    };
    (0..len)
        .filter_map(|idx| (ops().list_get)(value, idx))
        .filter_map(dependency_key_from_value)
        .collect()
}

fn route_parameter_from_value(value: MbValue) -> Option<RouteParameter> {
    let type_name = native_type_name(value)?;
    let param = unsafe { mb_unwrap_native_ref::<Param>(value) }?;
    let location = match param.kind {
        ParamKind::Query => "query",
        ParamKind::Header => "header",
        ParamKind::Body | ParamKind::Depends => return None,
    };
    let name = param
        .name
        .as_ref()
        .or(param.alias.as_ref())
        .and_then(|value| non_empty_string(value.clone()))?;
    let mut route_param = RouteParameter::new(name, location);
    if let Some(description) = &param.description {
        route_param = route_param.description(description.clone());
    }
    if param.default_provided && !param.required {
        route_param = route_param.default_json(
            serde_json::to_string(&mb_to_json_value(param.default))
                .unwrap_or_else(|_| "null".to_string()),
        );
    }
    if let Some(schema_json) = schema_json_for_value(param.default) {
        route_param = route_param.schema_json(schema_json);
    }
    route_param = route_param.required(param.required);
    if matches!(type_name, "Header" | "Query") {
        Some(route_param)
    } else {
        None
    }
}

fn route_parameters_from_value(value: MbValue) -> Vec<RouteParameter> {
    if value.is_none() {
        return Vec::new();
    }
    if let Some(parameter) = route_parameter_from_value(value) {
        return vec![parameter];
    }
    let Some(len) = (ops().list_len)(value) else {
        return Vec::new();
    };
    (0..len)
        .filter_map(|idx| (ops().list_get)(value, idx))
        .filter_map(route_parameter_from_value)
        .collect()
}

fn schema_json_for_value(value: MbValue) -> Option<String> {
    if value.is_none() {
        return None;
    }
    let schema = if value.as_bool().is_some() {
        serde_json::json!({ "type": "boolean" })
    } else if value.as_int().is_some() {
        serde_json::json!({ "type": "integer" })
    } else if value.as_float().is_some() {
        serde_json::json!({ "type": "number" })
    } else if (ops().list_len)(value).is_some() {
        serde_json::json!({ "type": "array" })
    } else if (ops().dict_iter_str_items)(value).is_some() {
        serde_json::json!({ "type": "object" })
    } else {
        serde_json::json!({ "type": "string" })
    };
    serde_json::to_string(&schema).ok()
}

fn model_metadata_from_value(value: MbValue) -> Option<ModelMetadata> {
    match native_type_name(value) {
        Some("BaseModel") => {
            let model = unsafe { mb_unwrap_native_ref::<MbBaseModel>(value) }?;
            Some(ModelMetadata {
                name: non_empty_string(model.name.clone())?,
                schema_json: Some(model.to_json_schema_string()),
                native_handle: Some(value),
            })
        }
        _ => read_string_value(value)
            .and_then(non_empty_string)
            .map(|name| ModelMetadata {
                name,
                schema_json: None,
                native_handle: None,
            }),
    }
}

fn non_empty_string(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn mb_to_json_value(value: MbValue) -> JsonValue {
    if value.is_none() {
        JsonValue::Null
    } else if let Some(bool_value) = value.as_bool() {
        JsonValue::Bool(bool_value)
    } else if let Some(int_value) = value.as_int() {
        JsonValue::Number(Number::from(int_value))
    } else if let Some(float_value) = value.as_float() {
        Number::from_f64(float_value)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null)
    } else if let Some(len) = (ops().list_len)(value) {
        JsonValue::Array(
            (0..len)
                .filter_map(|idx| (ops().list_get)(value, idx))
                .map(mb_to_json_value)
                .collect(),
        )
    } else if let Some(items) = (ops().dict_iter_str_items)(value) {
        let mut object = Map::new();
        for (key, item) in items {
            object.insert(key, mb_to_json_value(item));
        }
        JsonValue::Object(object)
    } else {
        (ops().str_read)(value)
            .map(JsonValue::String)
            .unwrap_or(JsonValue::Null)
    }
}

fn resolve_dependency_values(provider_value: MbValue, keys: &[String]) -> HashMap<String, String> {
    if keys.is_empty() {
        return HashMap::new();
    }

    match native_type_name(provider_value) {
        Some("Container") => {
            let Some(container) =
                (unsafe { mb_unwrap_native_ref::<MbDiContainer>(provider_value) })
            else {
                return HashMap::new();
            };
            let scope = container.inner.request_scope();
            keys.iter()
                .filter_map(|key| {
                    scope
                        .resolve::<String>(key.as_str())
                        .ok()
                        .map(|value| (key.clone(), value.as_str().to_string()))
                })
                .collect()
        }
        Some("RequestScope") => {
            let Some(scope) = (unsafe { mb_unwrap_native_ref::<MbDiScope>(provider_value) }) else {
                return HashMap::new();
            };
            keys.iter()
                .filter_map(|key| {
                    scope
                        .inner
                        .resolve::<String>(key.as_str())
                        .ok()
                        .map(|value| (key.clone(), value.as_str().to_string()))
                })
                .collect()
        }
        _ => read_dependency_map(provider_value),
    }
}

fn read_dependency_map(value: MbValue) -> HashMap<String, String> {
    let Some(items) = (ops().dict_iter_str_items)(value) else {
        return HashMap::new();
    };
    items
        .into_iter()
        .filter_map(|(key, value)| read_string_value(value).map(|value| (key, value)))
        .collect()
}

fn read_request_context_map(value: MbValue, key: &str) -> HashMap<String, String> {
    if value.is_none() {
        return HashMap::new();
    }
    let Some(part) = (ops().dict_get_str)(value, key) else {
        return HashMap::new();
    };
    read_string_map_value(part)
}

fn read_string_map_value(value: MbValue) -> HashMap<String, String> {
    let Some(items) = (ops().dict_iter_str_items)(value) else {
        return HashMap::new();
    };
    items
        .into_iter()
        .filter_map(|(key, value)| read_string_value(value).map(|value| (key, value)))
        .collect()
}

fn normalized_preflight_body(
    app_value: MbValue,
    method: &str,
    path: &str,
    body_value: MbValue,
) -> (JsonValue, ModelErrorReport) {
    let original = mb_to_json_value(body_value);
    let Some(model) = request_model_handle(app_value, method, path) else {
        return (original, ModelErrorReport::default());
    };

    let text = if let Some(body_text) = (ops().str_read)(body_value) {
        match model_dump_json_from_json_text(model, &body_text, MbValue::none()) {
            Ok(text) => text,
            Err(error) => {
                let detail = model_validation_detail_json_from_json_text(model, &body_text, "body")
                    .and_then(|json| serde_json::from_str::<JsonValue>(&json).ok())
                    .and_then(|value| match value {
                        JsonValue::Array(items) => Some(items),
                        _ => None,
                    })
                    .unwrap_or_default();
                return (original, ModelErrorReport::new(vec![error], detail));
            }
        }
    } else {
        let args = [model, body_value];
        let dumped = unsafe { mb_schema_model_dump_json(args.as_ptr(), args.len()) };
        let Some(text) = (ops().str_read)(dumped) else {
            return (
                original,
                ModelErrorReport::from_error(
                    "ValidationError: model_dump_json returned non-string result".to_string(),
                ),
            );
        };
        text
    };

    match serde_json::from_str::<JsonValue>(&text) {
        Ok(normalized) => (normalized, ModelErrorReport::default()),
        Err(_) => {
            let detail = model_validation_detail_json(model, body_value, "body")
                .and_then(|json| serde_json::from_str::<JsonValue>(&json).ok())
                .and_then(|value| match value {
                    JsonValue::Array(items) => Some(items),
                    _ => None,
                })
                .unwrap_or_default();
            (original, ModelErrorReport::new(vec![text], detail))
        }
    }
}

#[derive(Default)]
struct ModelErrorReport {
    errors: Vec<String>,
    detail: Vec<JsonValue>,
}

impl ModelErrorReport {
    fn new(errors: Vec<String>, detail: Vec<JsonValue>) -> Self {
        Self { errors, detail }
    }

    fn from_error(error: String) -> Self {
        Self {
            errors: vec![error.clone()],
            detail: vec![serde_json::json!({
                "loc": ["body"],
                "msg": error,
                "type": "value_error",
            })],
        }
    }

    fn is_empty(&self) -> bool {
        self.errors.is_empty() && self.detail.is_empty()
    }
}

fn append_model_errors(report: String, model_errors: ModelErrorReport) -> String {
    if model_errors.is_empty() {
        return report;
    }

    let Ok(mut report_value) = serde_json::from_str::<JsonValue>(&report) else {
        return report;
    };
    let JsonValue::Object(report_object) = &mut report_value else {
        return report;
    };

    if report_object
        .get("matched")
        .and_then(JsonValue::as_bool)
        .unwrap_or(true)
    {
        report_object.insert(
            "status_code".to_string(),
            JsonValue::Number(Number::from(422)),
        );
    }

    let errors = report_object
        .entry("errors".to_string())
        .or_insert_with(|| JsonValue::Array(Vec::new()));
    match errors {
        JsonValue::Array(items) => {
            items.extend(model_errors.errors.into_iter().map(JsonValue::String));
        }
        other => {
            *other = JsonValue::Array(
                model_errors
                    .errors
                    .into_iter()
                    .map(JsonValue::String)
                    .collect(),
            );
        }
    }

    if !model_errors.detail.is_empty() {
        let detail = report_object
            .entry("detail".to_string())
            .or_insert_with(|| JsonValue::Array(Vec::new()));
        match detail {
            JsonValue::Array(items) => {
                items.extend(model_errors.detail);
            }
            other => {
                *other = JsonValue::Array(model_errors.detail);
            }
        }
    }

    serde_json::to_string(&report_value).unwrap_or(report)
}

fn response_error_json(model_errors: ModelErrorReport) -> String {
    let mut object = Map::new();
    object.insert(
        "errors".to_string(),
        JsonValue::Array(
            model_errors
                .errors
                .into_iter()
                .map(JsonValue::String)
                .collect(),
        ),
    );
    if !model_errors.detail.is_empty() {
        object.insert("detail".to_string(), JsonValue::Array(model_errors.detail));
    }
    serde_json::to_string(&JsonValue::Object(object)).unwrap_or_else(|_| "{}".to_string())
}

fn normalized_response_body(
    app_value: MbValue,
    method: &str,
    path: &str,
    result: MbValue,
) -> Result<String, ModelErrorReport> {
    let Some(model) = response_model_handle(app_value, method, path) else {
        return serde_json::to_string(&mb_to_json_value(result))
            .map_err(|err| ModelErrorReport::from_error(format!("SerializationError: {err}")));
    };

    let args = [model, result];
    let dumped = unsafe { mb_schema_model_dump_json(args.as_ptr(), args.len()) };
    let Some(text) = (ops().str_read)(dumped) else {
        return Err(ModelErrorReport::from_error(
            "ValidationError: model_dump_json returned non-string result".to_string(),
        ));
    };
    if serde_json::from_str::<JsonValue>(&text).is_ok() {
        return Ok(text);
    }

    let detail = model_validation_detail_json(model, result, "response")
        .and_then(|json| serde_json::from_str::<JsonValue>(&json).ok())
        .and_then(|value| match value {
            JsonValue::Array(items) => Some(items),
            _ => None,
        })
        .unwrap_or_default();
    Err(ModelErrorReport::new(vec![text], detail))
}

pub fn app_dispatch_handler_json(
    app_value: MbValue,
    method: &str,
    path: &str,
    status_code: u16,
) -> Option<(u16, String)> {
    let handler = route_handler(app_value, method, path)?;
    let result = (ops().call0)(handler)?;
    match normalized_response_body(app_value, method, path, result) {
        Ok(body) => Some((status_code, body)),
        Err(errors) => Some((500, response_error_json(errors))),
    }
}

fn matching_dependency_keys(app: &App, method: &str, path: &str) -> Vec<String> {
    let method = normalize_method(method);
    let path = normalize_path(path);
    app.endpoints()
        .iter()
        .find(|endpoint| endpoint.method == method && endpoint.path == path)
        .map(|endpoint| endpoint.dependency_keys.clone())
        .unwrap_or_default()
}

fn route_model_key(receiver: MbValue, method: &str, path: &str) -> (u64, String, String) {
    (
        receiver.to_bits(),
        normalize_method(method),
        normalize_path(path),
    )
}

fn register_request_model_handle(receiver: MbValue, method: &str, path: &str, model: MbValue) {
    let key = route_model_key(receiver, method, path);
    REQUEST_MODEL_HANDLES.with(|models| {
        models.borrow_mut().insert(key, model);
    });
}

fn request_model_handle(receiver: MbValue, method: &str, path: &str) -> Option<MbValue> {
    let key = route_model_key(receiver, method, path);
    REQUEST_MODEL_HANDLES.with(|models| models.borrow().get(&key).copied())
}

fn register_response_model_handle(receiver: MbValue, method: &str, path: &str, model: MbValue) {
    let key = route_model_key(receiver, method, path);
    RESPONSE_MODEL_HANDLES.with(|models| {
        models.borrow_mut().insert(key, model);
    });
}

fn response_model_handle(receiver: MbValue, method: &str, path: &str) -> Option<MbValue> {
    let key = route_model_key(receiver, method, path);
    RESPONSE_MODEL_HANDLES.with(|models| models.borrow().get(&key).copied())
}

fn register_route_handler(receiver: MbValue, method: &str, path: &str, handler: MbValue) {
    if handler.is_none() {
        return;
    }
    let key = route_model_key(receiver, method, path);
    ROUTE_HANDLERS.with(|handlers| {
        handlers.borrow_mut().insert(key, handler);
    });
}

fn route_handler(receiver: MbValue, method: &str, path: &str) -> Option<MbValue> {
    let key = route_model_key(receiver, method, path);
    ROUTE_HANDLERS.with(|handlers| handlers.borrow().get(&key).copied())
}

fn clear_route_bindings(receiver: MbValue) {
    let bits = receiver.to_bits();
    REQUEST_MODEL_HANDLES.with(|models| {
        models
            .borrow_mut()
            .retain(|(receiver_bits, _, _), _| *receiver_bits != bits);
    });
    RESPONSE_MODEL_HANDLES.with(|models| {
        models
            .borrow_mut()
            .retain(|(receiver_bits, _, _), _| *receiver_bits != bits);
    });
    ROUTE_HANDLERS.with(|handlers| {
        handlers
            .borrow_mut()
            .retain(|(receiver_bits, _, _), _| *receiver_bits != bits);
    });
}

fn normalize_method(method: &str) -> String {
    let trimmed = method.trim();
    if trimmed.is_empty() {
        "GET".to_string()
    } else {
        trimmed.to_ascii_uppercase()
    }
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        "/".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

unsafe fn read_app_mut(value: MbValue) -> Option<&'static mut App> {
    unsafe { mb_unwrap_native_mut::<App>(value) }
}

unsafe fn read_router_mut(value: MbValue) -> Option<&'static mut Router> {
    unsafe { mb_unwrap_native_mut::<Router>(value) }
}

unsafe fn read_background_tasks_mut(value: MbValue) -> Option<&'static mut BackgroundTasks> {
    unsafe { mb_unwrap_native_mut::<BackgroundTasks>(value) }
}

unsafe fn read_background_tasks(value: MbValue) -> Option<&'static BackgroundTasks> {
    unsafe { mb_unwrap_native_ref::<BackgroundTasks>(value) }
}

unsafe fn read_endpoint(value: MbValue) -> Option<&'static Endpoint> {
    unsafe { mb_unwrap_native_ref::<Endpoint>(value) }
}

unsafe fn read_app(value: MbValue) -> Option<&'static App> {
    unsafe { mb_unwrap_native_ref::<App>(value) }
}

unsafe fn read_router(value: MbValue) -> Option<&'static Router> {
    unsafe { mb_unwrap_native_ref::<Router>(value) }
}

fn native_func(func: NativeFn) -> MbValue {
    MbValue::from_func(func as usize)
}

fn bind_route_target(receiver: MbValue, method: &'static str) {
    ACTIVE_ROUTE_TARGET.with(|slot| {
        *slot.borrow_mut() = Some(BoundRouteTarget { receiver, method });
    });
}

fn take_bound_route_target() -> Option<BoundRouteTarget> {
    ACTIVE_ROUTE_TARGET.with(|slot| slot.borrow_mut().take())
}

fn set_pending_route(route: Option<PendingRoute>) {
    PENDING_ROUTE.with(|slot| {
        *slot.borrow_mut() = route;
    });
}

fn take_pending_route() -> Option<PendingRoute> {
    PENDING_ROUTE.with(|slot| slot.borrow_mut().take())
}

unsafe fn route_factory_for_method(
    args: *const MbValue,
    nargs: usize,
    method: &'static str,
) -> MbValue {
    let receiver = unsafe { read(args, nargs, 0) };
    bind_route_target(receiver, method);
    native_func(httpkit_route_factory)
}

fn register_pending_endpoint(pending: PendingRoute, handler: MbValue) {
    let route_path = normalize_path(&pending.path);
    let request_model_handle = pending
        .request_model
        .as_ref()
        .and_then(|model| model.native_handle);
    let response_model_handle = pending
        .response_model
        .as_ref()
        .and_then(|model| model.native_handle);
    let mut endpoint = Endpoint::new(pending.method, pending.path)
        .with_dependency_keys(pending.dependency_keys)
        .with_parameters(pending.parameters)
        .with_status_code(pending.status_code);
    if let Some(request_model) = pending.request_model {
        endpoint = endpoint.with_request_model(request_model.name);
        if let Some(schema_json) = request_model.schema_json {
            endpoint = endpoint.with_request_schema_json(schema_json);
        }
    }
    if let Some(response_model) = pending.response_model {
        endpoint = endpoint.with_response_model(response_model.name);
        if let Some(schema_json) = response_model.schema_json {
            endpoint = endpoint.with_response_schema_json(schema_json);
        }
    }
    match native_type_name(pending.receiver) {
        Some("App") => {
            if let Some(app) = unsafe { read_app_mut(pending.receiver) } {
                app.add_endpoint(endpoint);
            }
        }
        Some("Router") => {
            if let Some(router) = unsafe { read_router_mut(pending.receiver) } {
                router.add_endpoint(endpoint);
            }
        }
        _ => {}
    }
    if let Some(model) = request_model_handle {
        register_request_model_handle(pending.receiver, pending.method, &route_path, model);
    }
    if let Some(model) = response_model_handle {
        register_response_model_handle(pending.receiver, pending.method, &route_path, model);
    }
    register_route_handler(pending.receiver, pending.method, &route_path, handler);
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#app
#[no_mangle]
pub unsafe extern "C" fn app_new(args: *const MbValue, nargs: usize) -> MbValue {
    let metadata = read_string_map(args, nargs, 0);
    let app = mb_wrap_native_typed("App", App::new(metadata));
    clear_route_bindings(app);
    app
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#app
#[no_mangle]
pub unsafe extern "C" fn fastapi_new(args: *const MbValue, nargs: usize) -> MbValue {
    app_new(args, nargs)
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#router
#[no_mangle]
pub unsafe extern "C" fn router_new(args: *const MbValue, nargs: usize) -> MbValue {
    let prefix = read_string(args, nargs, 0).unwrap_or_default();
    let tags = read_string_list(args, nargs, 1);
    let router = mb_wrap_native_typed("Router", Router::new(prefix, tags));
    clear_route_bindings(router);
    router
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#endpoint
#[no_mangle]
pub unsafe extern "C" fn endpoint_new(args: *const MbValue, nargs: usize) -> MbValue {
    let method = read_string(args, nargs, 0).unwrap_or_else(|| "GET".to_string());
    let path = read_string(args, nargs, 1).unwrap_or_else(|| "/".to_string());
    let handler_name = read_string(args, nargs, 2);
    let dependency_keys = read_string_list(args, nargs, 3);
    let request_model = read_string(args, nargs, 4);
    let response_model = read_string(args, nargs, 5);
    let status_code = read_status_code(args, nargs, 6);

    let mut endpoint = Endpoint::new(method, path)
        .with_dependency_keys(dependency_keys)
        .with_status_code(status_code);
    if let Some(handler_name) = handler_name {
        endpoint = endpoint.with_handler_name(handler_name);
    }
    if let Some(request_model) = request_model {
        endpoint = endpoint.with_request_model(request_model);
    }
    if let Some(response_model) = response_model {
        endpoint = endpoint.with_response_model(response_model);
    }

    mb_wrap_native_typed("Endpoint", endpoint)
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#endpoint
#[no_mangle]
pub unsafe extern "C" fn app_add_endpoint(args: *const MbValue, nargs: usize) -> MbValue {
    let app_value = read(args, nargs, 0);
    let endpoint_value = read(args, nargs, 1);
    let Some(app) = (unsafe { read_app_mut(app_value) }) else {
        return MbValue::none();
    };
    let Some(endpoint) = (unsafe { read_endpoint(endpoint_value) }) else {
        return MbValue::none();
    };

    app.add_endpoint(endpoint.clone());
    app_value
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#endpoint
#[no_mangle]
pub unsafe extern "C" fn router_add_endpoint(args: *const MbValue, nargs: usize) -> MbValue {
    let router_value = read(args, nargs, 0);
    let endpoint_value = read(args, nargs, 1);
    let Some(router) = (unsafe { read_router_mut(router_value) }) else {
        return MbValue::none();
    };
    let Some(endpoint) = (unsafe { read_endpoint(endpoint_value) }) else {
        return MbValue::none();
    };

    router.add_endpoint(endpoint.clone());
    router_value
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#middleware
#[no_mangle]
pub unsafe extern "C" fn cors_middleware_new(args: *const MbValue, nargs: usize) -> MbValue {
    let allow_origins = read_string_list(args, nargs, 0);
    let allow_methods = read_string_list(args, nargs, 1);
    let allow_headers = read_string_list(args, nargs, 2);
    let allow_credentials = bool::from_mb_value(read(args, nargs, 3)).unwrap_or(false);
    mb_wrap_native_typed(
        "CORSMiddleware",
        CORSMiddleware {
            allow_origins,
            allow_methods,
            allow_headers,
            allow_credentials,
        },
    )
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#static-files
#[no_mangle]
pub unsafe extern "C" fn static_files_new(args: *const MbValue, nargs: usize) -> MbValue {
    let directory = read_string(args, nargs, 0);
    let html = bool::from_mb_value(read(args, nargs, 1)).unwrap_or(false);
    mb_wrap_native_typed("StaticFiles", StaticFiles { directory, html })
}

unsafe fn param_new(args: *const MbValue, nargs: usize, kind: ParamKind) -> MbValue {
    let (positional_nargs, kwargs) = unsafe { trailing_kwargs(args, nargs) };
    let kw_default = kwargs_get(kwargs, "default");
    let single_name_arg = if matches!(kind, ParamKind::Query | ParamKind::Header)
        && positional_nargs == 1
        && kwargs.is_none()
    {
        read_string(args, positional_nargs, 0)
    } else {
        None
    };
    let default_provided =
        single_name_arg.is_none() && (positional_nargs > 0 || kw_default.is_some());
    let default = if single_name_arg.is_some() {
        MbValue::none()
    } else {
        kw_default.unwrap_or_else(|| read(args, positional_nargs, 0))
    };
    let alias = kwargs_get(kwargs, "alias")
        .and_then(read_string_value)
        .or_else(|| read_string(args, positional_nargs, 1));
    let name = kwargs_get(kwargs, "name")
        .and_then(read_string_value)
        .or_else(|| alias.clone())
        .or_else(|| single_name_arg.clone());
    let description = kwargs_get(kwargs, "description").and_then(read_string_value);
    let required = kwargs_get(kwargs, "required")
        .and_then(|value| value.as_bool())
        .unwrap_or(!default_provided);
    let dependency_key = if kind == ParamKind::Depends {
        read_string(args, positional_nargs, 0).and_then(|key| ProviderKey::new(key).ok())
    } else {
        None
    };
    mb_wrap_native_typed(
        match kind {
            ParamKind::Depends => "Depends",
            ParamKind::Query => "Query",
            ParamKind::Body => "Body",
            ParamKind::Header => "Header",
        },
        Param {
            kind,
            default,
            default_provided,
            name,
            alias,
            description,
            required,
            dependency_key,
        },
    )
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#param
#[no_mangle]
pub unsafe extern "C" fn depends_new(args: *const MbValue, nargs: usize) -> MbValue {
    param_new(args, nargs, ParamKind::Depends)
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#param
#[no_mangle]
pub unsafe extern "C" fn query_new(args: *const MbValue, nargs: usize) -> MbValue {
    param_new(args, nargs, ParamKind::Query)
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#param
#[no_mangle]
pub unsafe extern "C" fn body_new(args: *const MbValue, nargs: usize) -> MbValue {
    param_new(args, nargs, ParamKind::Body)
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#param
#[no_mangle]
pub unsafe extern "C" fn header_new(args: *const MbValue, nargs: usize) -> MbValue {
    param_new(args, nargs, ParamKind::Header)
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#background-tasks
#[no_mangle]
pub unsafe extern "C" fn background_tasks_new(_args: *const MbValue, _nargs: usize) -> MbValue {
    mb_wrap_native_typed("BackgroundTasks", BackgroundTasks::new())
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#background-tasks
#[no_mangle]
pub unsafe extern "C" fn background_tasks_add_task(args: *const MbValue, nargs: usize) -> MbValue {
    let tasks_value = unsafe { read(args, nargs, 0) };
    let name = unsafe { read_string(args, nargs, 1) }.unwrap_or_default();
    let payload = unsafe { read(args, nargs, 2) };
    let queue = unsafe { read_string(args, nargs, 3) };
    let Some(tasks) = (unsafe { read_background_tasks_mut(tasks_value) }) else {
        return MbValue::none();
    };
    let payload_json = if payload.is_none() {
        None
    } else {
        serde_json::to_string(&mb_to_json_value(payload)).ok()
    };
    if tasks.add_named_task(name, payload_json, queue) {
        tasks_value
    } else {
        MbValue::none()
    }
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#background-tasks
#[no_mangle]
pub unsafe extern "C" fn background_tasks_len(args: *const MbValue, nargs: usize) -> MbValue {
    let tasks_value = unsafe { read(args, nargs, 0) };
    let Some(tasks) = (unsafe { read_background_tasks(tasks_value) }) else {
        return MbValue::none();
    };
    MbValue::from_int(tasks.len() as i64)
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#background-tasks
#[no_mangle]
pub unsafe extern "C" fn background_tasks_json(args: *const MbValue, nargs: usize) -> MbValue {
    let tasks_value = unsafe { read(args, nargs, 0) };
    let Some(tasks) = (unsafe { read_background_tasks(tasks_value) }) else {
        return MbValue::none();
    };
    (ops().str_new)(&tasks.tasks_json())
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#request-context
#[no_mangle]
pub unsafe extern "C" fn request_context_new(_args: *const MbValue, _nargs: usize) -> MbValue {
    mb_wrap_native_typed("RequestContext", RequestContext)
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#streaming-response
#[no_mangle]
pub unsafe extern "C" fn streaming_response_new(args: *const MbValue, nargs: usize) -> MbValue {
    let status_code = read_status_code(args, nargs, 1);
    let media_type = read_string(args, nargs, 2);
    let headers = read_string_map(args, nargs, 3);
    mb_wrap_native_typed(
        "StreamingResponse",
        StreamingResponse {
            status_code,
            media_type,
            headers,
        },
    )
}

/// Route decorators preserve the handler and, when produced from a typed
/// `App`/`Router` route getter, record native endpoint metadata.
/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#route-decorator
#[no_mangle]
pub unsafe extern "C" fn httpkit_route_passthrough(args: *const MbValue, nargs: usize) -> MbValue {
    let handler = unsafe { read(args, nargs, 0) };
    if let Some(pending) = take_pending_route() {
        register_pending_endpoint(pending, handler);
    }
    handler
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#route-decorator
#[no_mangle]
pub unsafe extern "C" fn httpkit_route_factory(args: *const MbValue, nargs: usize) -> MbValue {
    let (positional_nargs, kwargs) = unsafe { trailing_kwargs(args, nargs) };
    let path = unsafe { read_string(args, positional_nargs, 0) }.unwrap_or_else(|| "/".to_string());
    let status_code = kwargs_get(kwargs, "status_code")
        .and_then(read_status_code_value)
        .unwrap_or_else(|| unsafe { read_status_code(args, positional_nargs, 1) });
    let dependency_keys = kwargs_get(kwargs, "dependencies")
        .map(dependency_keys_from_value)
        .unwrap_or_else(|| {
            if positional_nargs > 2 {
                dependency_keys_from_value(unsafe { read(args, positional_nargs, 2) })
            } else {
                Vec::new()
            }
        });
    let parameters = kwargs_get(kwargs, "parameters")
        .map(route_parameters_from_value)
        .unwrap_or_default();
    let request_model = kwargs_get(kwargs, "request_model")
        .and_then(model_metadata_from_value)
        .or_else(|| {
            if positional_nargs > 3 {
                model_metadata_from_value(unsafe { read(args, positional_nargs, 3) })
            } else {
                None
            }
        });
    let response_model = kwargs_get(kwargs, "response_model")
        .and_then(model_metadata_from_value)
        .or_else(|| {
            if positional_nargs > 4 {
                model_metadata_from_value(unsafe { read(args, positional_nargs, 4) })
            } else {
                None
            }
        });
    let pending = take_bound_route_target().map(|bound| PendingRoute {
        receiver: bound.receiver,
        method: bound.method,
        path,
        status_code,
        dependency_keys,
        parameters,
        request_model,
        response_model,
    });
    set_pending_route(pending);
    native_func(httpkit_route_passthrough)
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#app-methods
#[no_mangle]
pub unsafe extern "C" fn httpkit_noop(_args: *const MbValue, _nargs: usize) -> MbValue {
    MbValue::none()
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#endpoint
#[no_mangle]
pub unsafe extern "C" fn app_endpoint_count(args: *const MbValue, nargs: usize) -> MbValue {
    let app_value = unsafe { read(args, nargs, 0) };
    let Some(app) = (unsafe { read_app(app_value) }) else {
        return MbValue::none();
    };
    MbValue::from_int(app.endpoint_count() as i64)
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#openapi
#[no_mangle]
pub unsafe extern "C" fn app_openapi_json(args: *const MbValue, nargs: usize) -> MbValue {
    let app_value = unsafe { read(args, nargs, 0) };
    let Some(app) = (unsafe { read_app(app_value) }) else {
        return MbValue::none();
    };
    (ops().str_new)(&app.openapi_json())
}

#[no_mangle]
pub unsafe extern "C" fn app_openapi_json_bound(_args: *const MbValue, _nargs: usize) -> MbValue {
    let Some(bound) = take_bound_route_target() else {
        return MbValue::none();
    };
    let Some(app) = (unsafe { read_app(bound.receiver) }) else {
        return MbValue::none();
    };
    (ops().str_new)(&app.openapi_json())
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#preflight
#[no_mangle]
pub unsafe extern "C" fn app_preflight_json(args: *const MbValue, nargs: usize) -> MbValue {
    let app_value = unsafe { read(args, nargs, 0) };
    let method = unsafe { read_string(args, nargs, 1) }.unwrap_or_else(|| "GET".to_string());
    let path = unsafe { read_string(args, nargs, 2) }.unwrap_or_else(|| "/".to_string());
    let body_value = unsafe { read(args, nargs, 3) };
    let provider_value = unsafe { read(args, nargs, 4) };
    let context_value = unsafe { read(args, nargs, 5) };
    let Some(app) = (unsafe { read_app(app_value) }) else {
        return MbValue::none();
    };
    let dependency_keys = matching_dependency_keys(app, &method, &path);
    let dependencies = resolve_dependency_values(provider_value, &dependency_keys);
    let (body, model_errors) = normalized_preflight_body(app_value, &method, &path, body_value);
    let report = app.preflight_request_json_with_context(
        &method,
        &path,
        body,
        dependencies,
        read_request_context_map(context_value, "query"),
        read_request_context_map(context_value, "headers"),
    );
    (ops().str_new)(&append_model_errors(report, model_errors))
}

#[no_mangle]
pub unsafe extern "C" fn app_preflight_json_bound(args: *const MbValue, nargs: usize) -> MbValue {
    let Some(bound) = take_bound_route_target() else {
        return MbValue::none();
    };
    let method = unsafe { read(args, nargs, 0) };
    let path = unsafe { read(args, nargs, 1) };
    let body = unsafe { read(args, nargs, 2) };
    let provider = unsafe { read(args, nargs, 3) };
    let context = unsafe { read(args, nargs, 4) };
    let forwarded = [bound.receiver, method, path, body, provider, context];
    unsafe { app_preflight_json(forwarded.as_ptr(), forwarded.len()) }
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#endpoint
#[no_mangle]
pub unsafe extern "C" fn router_endpoint_count(args: *const MbValue, nargs: usize) -> MbValue {
    let router_value = unsafe { read(args, nargs, 0) };
    let Some(router) = (unsafe { read_router(router_value) }) else {
        return MbValue::none();
    };
    MbValue::from_int(router.endpoint_count() as i64)
}

/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#request-context
#[no_mangle]
pub unsafe extern "C" fn request_json(_args: *const MbValue, _nargs: usize) -> MbValue {
    (ops().dict_new)()
}

pub unsafe extern "C" fn get_route_factory_get(args: *const MbValue, nargs: usize) -> MbValue {
    unsafe { route_factory_for_method(args, nargs, "GET") }
}

pub unsafe extern "C" fn get_route_factory_post(args: *const MbValue, nargs: usize) -> MbValue {
    unsafe { route_factory_for_method(args, nargs, "POST") }
}

pub unsafe extern "C" fn get_route_factory_put(args: *const MbValue, nargs: usize) -> MbValue {
    unsafe { route_factory_for_method(args, nargs, "PUT") }
}

pub unsafe extern "C" fn get_route_factory_patch(args: *const MbValue, nargs: usize) -> MbValue {
    unsafe { route_factory_for_method(args, nargs, "PATCH") }
}

pub unsafe extern "C" fn get_route_factory_delete(args: *const MbValue, nargs: usize) -> MbValue {
    unsafe { route_factory_for_method(args, nargs, "DELETE") }
}

unsafe extern "C" fn get_noop(_args: *const MbValue, _nargs: usize) -> MbValue {
    native_func(httpkit_noop)
}

unsafe extern "C" fn get_app_endpoint_count(args: *const MbValue, nargs: usize) -> MbValue {
    unsafe { app_endpoint_count(args, nargs) }
}

unsafe extern "C" fn get_app_openapi(args: *const MbValue, nargs: usize) -> MbValue {
    let receiver = unsafe { read(args, nargs, 0) };
    bind_route_target(receiver, "OPENAPI");
    native_func(app_openapi_json_bound)
}

unsafe extern "C" fn get_app_preflight(args: *const MbValue, nargs: usize) -> MbValue {
    let receiver = unsafe { read(args, nargs, 0) };
    bind_route_target(receiver, "PREFLIGHT");
    native_func(app_preflight_json_bound)
}

unsafe extern "C" fn get_router_endpoint_count(args: *const MbValue, nargs: usize) -> MbValue {
    unsafe { router_endpoint_count(args, nargs) }
}

unsafe extern "C" fn get_request_json(_args: *const MbValue, _nargs: usize) -> MbValue {
    native_func(request_json)
}

unsafe extern "C" fn get_background_tasks_add_task(args: *const MbValue, nargs: usize) -> MbValue {
    let receiver = unsafe { read(args, nargs, 0) };
    bind_route_target(receiver, "BACKGROUND_TASKS");
    native_func(background_tasks_add_task_bound)
}

#[no_mangle]
pub unsafe extern "C" fn background_tasks_add_task_bound(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let Some(bound) = take_bound_route_target() else {
        return MbValue::none();
    };
    let name = unsafe { read(args, nargs, 0) };
    let payload = unsafe { read(args, nargs, 1) };
    let queue = unsafe { read(args, nargs, 2) };
    let forwarded = [bound.receiver, name, payload, queue];
    unsafe { background_tasks_add_task(forwarded.as_ptr(), forwarded.len()) }
}

fn http_status_value() -> MbValue {
    let ops = ops();
    let status = (ops.dict_new)();
    for (name, code) in [
        ("OK", 200),
        ("CREATED", 201),
        ("ACCEPTED", 202),
        ("NO_CONTENT", 204),
        ("BAD_REQUEST", 400),
        ("UNAUTHORIZED", 401),
        ("NOT_FOUND", 404),
        ("CONFLICT", 409),
        ("UNPROCESSABLE_ENTITY", 422),
        ("INTERNAL_SERVER_ERROR", 500),
    ] {
        (ops.dict_insert_str)(status, name, MbValue::from_int(code));
    }
    status
}

fn register_getter(type_name: &str, attr: &str, getter: NativeFn) {
    if let Some(o) = mamba_registry::ops::OBJECT_OPS.get() {
        (o.register_getter)(type_name, attr, getter);
    }
}

/// Register the hand-written Mamba interface that sits around generated core
/// request/response symbols and the native httpkit app model.
/// @spec .score/tech_design/projects/httpkit/runtime-surface.md#register
pub fn register(r: &mut ModuleRegistrar) {
    r.add_symbol(rt_sym!("App", app_new, "App(**metadata) -> App"));
    r.add_symbol(rt_sym!(
        "FastAPI",
        fastapi_new,
        "FastAPI(**metadata) -> App (compat alias; not ASGI)"
    ));
    r.add_symbol(rt_sym!(
        "Router",
        router_new,
        "Router(prefix: str = '', tags: list[str] | None = None) -> Router"
    ));
    r.add_symbol(rt_sym!(
        "Endpoint",
        endpoint_new,
        "Endpoint(method: str, path: str, handler_name: str | None = None, dependency_keys: list[str] | None = None, request_model: str | None = None, response_model: str | None = None, status_code: int = 200) -> Endpoint"
    ));
    r.add_symbol(rt_sym!(
        "_httpkit_app_add_endpoint",
        app_add_endpoint,
        "_httpkit_app_add_endpoint(app: App, endpoint: Endpoint) -> App"
    ));
    r.add_symbol(rt_sym!(
        "_httpkit_router_add_endpoint",
        router_add_endpoint,
        "_httpkit_router_add_endpoint(router: Router, endpoint: Endpoint) -> Router"
    ));
    r.add_symbol(rt_sym!(
        "_httpkit_app_endpoint_count",
        app_endpoint_count,
        "_httpkit_app_endpoint_count(app: App) -> int"
    ));
    r.add_symbol(rt_sym!(
        "_httpkit_app_openapi",
        app_openapi_json,
        "_httpkit_app_openapi(app: App) -> str"
    ));
    r.add_symbol(rt_sym!(
        "_httpkit_app_preflight",
        app_preflight_json,
        "_httpkit_app_preflight(app: App, method: str, path: str, body: dict, provider: Container | RequestScope | dict, context: dict | None = None) -> str"
    ));
    r.add_symbol(rt_sym!(
        "_httpkit_router_endpoint_count",
        router_endpoint_count,
        "_httpkit_router_endpoint_count(router: Router) -> int"
    ));
    r.add_symbol(rt_sym!(
        "CORSMiddleware",
        cors_middleware_new,
        "CORSMiddleware(...) -> CORSMiddleware"
    ));
    r.add_symbol(rt_sym!(
        "StaticFiles",
        static_files_new,
        "StaticFiles(directory: str | None = None, html: bool = False) -> StaticFiles"
    ));
    r.add_symbol(rt_sym!(
        "Depends",
        depends_new,
        "Depends(dependency: object | None = None, **metadata) -> Depends"
    ));
    r.add_symbol(rt_sym!(
        "Query",
        query_new,
        "Query(default: object | None = required, alias: str | None = None, **metadata) -> Query"
    ));
    r.add_symbol(rt_sym!(
        "Body",
        body_new,
        "Body(default: object | None = None, **metadata) -> Body"
    ));
    r.add_symbol(rt_sym!(
        "Header",
        header_new,
        "Header(default: object | None = required, alias: str | None = None, **metadata) -> Header"
    ));
    r.add_symbol(rt_sym!(
        "BackgroundTasks",
        background_tasks_new,
        "BackgroundTasks() -> BackgroundTasks"
    ));
    r.add_symbol(rt_sym!(
        "_httpkit_background_tasks_add_task",
        background_tasks_add_task,
        "_httpkit_background_tasks_add_task(tasks: BackgroundTasks, name: str, payload: object | None = None, queue: str | None = None) -> BackgroundTasks"
    ));
    r.add_symbol(rt_sym!(
        "_httpkit_background_tasks_len",
        background_tasks_len,
        "_httpkit_background_tasks_len(tasks: BackgroundTasks) -> int"
    ));
    r.add_symbol(rt_sym!(
        "_httpkit_background_tasks_json",
        background_tasks_json,
        "_httpkit_background_tasks_json(tasks: BackgroundTasks) -> str"
    ));
    r.add_symbol(rt_sym!(
        "RequestContext",
        request_context_new,
        "RequestContext() -> RequestContext"
    ));
    r.add_symbol(rt_sym!("StreamingResponse", streaming_response_new, "StreamingResponse(content, status_code: int = 200, media_type: str | None = None, headers: dict | None = None) -> StreamingResponse"));
    r.add_symbol(rt_sym!(
        "_httpkit_route_factory",
        httpkit_route_factory,
        "_httpkit_route_factory(path: str, **options) -> decorator"
    ));
    r.add_symbol(rt_sym!(
        "_httpkit_route_passthrough",
        httpkit_route_passthrough,
        "_httpkit_route_passthrough(handler) -> handler"
    ));
    r.add_symbol(rt_sym!(
        "_httpkit_noop",
        httpkit_noop,
        "_httpkit_noop(*args) -> None"
    ));
    r.add_symbol(rt_sym!(
        "_request_json",
        request_json,
        "_request_json() -> dict"
    ));
    r.add_value(RuntimeValue::new("HTTPStatus", http_status_value));

    for type_name in ["App", "Router"] {
        register_getter(type_name, "get", get_route_factory_get);
        register_getter(type_name, "post", get_route_factory_post);
        register_getter(type_name, "put", get_route_factory_put);
        register_getter(type_name, "patch", get_route_factory_patch);
        register_getter(type_name, "delete", get_route_factory_delete);
        register_getter(type_name, "on_event", get_noop);
        for attr in ["include_router", "add_middleware", "mount", "run"] {
            register_getter(type_name, attr, get_noop);
        }
    }
    register_getter("App", "endpoint_count", get_app_endpoint_count);
    register_getter("App", "openapi", get_app_openapi);
    register_getter("App", "preflight", get_app_preflight);
    register_getter("Router", "endpoint_count", get_router_endpoint_count);
    register_getter("BackgroundTasks", "add_task", get_background_tasks_add_task);
    register_getter("Request", "json", get_request_json);
    register_getter("RequestContext", "json", get_request_json);
}
// HANDWRITE-END
