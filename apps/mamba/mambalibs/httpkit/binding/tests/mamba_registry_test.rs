use mamba_registry::{
    convert::{mb_unwrap_native_ref, native_type_name},
    find_module, ops, test_ops, FromMbValue, IntoMbValue, MbValue, ModuleRegistrar,
};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

type NativeFn = unsafe extern "C" fn(*const MbValue, usize) -> MbValue;

static TEST_HANDLER_CALLS: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn response_model_handler(_args: *const MbValue, _nargs: usize) -> MbValue {
    TEST_HANDLER_CALLS.fetch_add(1, Ordering::SeqCst);
    dict(&[
        ("name", "created".to_string().into_mb_value()),
        ("age", "4".to_string().into_mb_value()),
    ])
}

fn registered_module_by_name(name: &str) -> ModuleRegistrar {
    test_ops::init();
    let module = find_module(name).unwrap_or_else(|| panic!("httpkit must register {name}"));
    let mut registrar = ModuleRegistrar::new();
    module.register(&mut registrar);
    registrar
}

fn registered_module() -> ModuleRegistrar {
    registered_module_by_name("mambalibs.http")
}

fn call_symbol(registrar: &ModuleRegistrar, name: &str, args: &[MbValue]) -> MbValue {
    let sym = registrar
        .symbols()
        .iter()
        .find(|sym| sym.name == name)
        .unwrap_or_else(|| panic!("missing symbol {name}"));
    let func: NativeFn = unsafe { std::mem::transmute(sym.func_ptr) };
    unsafe { func(args.as_ptr(), args.len()) }
}

fn native_fn(value: MbValue, context: &str) -> NativeFn {
    unsafe { std::mem::transmute(value.as_func().unwrap_or_else(|| panic!("{context}"))) }
}

fn dict(items: &[(&str, MbValue)]) -> MbValue {
    let dict = (ops().dict_new)();
    for (key, value) in items {
        (ops().dict_insert_str)(dict, key, *value);
    }
    dict
}

fn detail_has_loc_type(details: MbValue, field: &str, error_type: &str) -> bool {
    let Some(len) = (ops().list_len)(details) else {
        return false;
    };
    (0..len).any(|idx| {
        let Some(detail) = (ops().list_get)(details, idx) else {
            return false;
        };
        let Some(loc) = (ops().dict_get_str)(detail, "loc") else {
            return false;
        };
        let loc_matches = (ops().list_len)(loc) == Some(2)
            && (ops().list_get)(loc, 0)
                .and_then(|value| (ops().str_read)(value))
                .as_deref()
                == Some("body")
            && (ops().list_get)(loc, 1)
                .and_then(|value| (ops().str_read)(value))
                .as_deref()
                == Some(field);
        let type_matches = (ops().dict_get_str)(detail, "type")
            .and_then(|value| (ops().str_read)(value))
            .as_deref()
            == Some(error_type);
        loc_matches && type_matches
    })
}

#[test]
fn mambalibs_http_exposes_core_and_runtime_symbols() {
    let registrar = registered_module();
    let symbols: HashSet<&str> = registrar.symbols().iter().map(|sym| sym.name).collect();
    let values: HashSet<&str> = registrar.values().iter().map(|value| value.name).collect();

    for expected in [
        "HealthCheck",
        "HealthManager",
        "HTTPException",
        "Cookie",
        "Request",
        "Response",
        "App",
        "FastAPI",
        "Router",
        "Endpoint",
        "Server",
        "run",
        "_httpkit_app_add_endpoint",
        "_httpkit_router_add_endpoint",
        "_httpkit_app_endpoint_count",
        "_httpkit_app_openapi",
        "_httpkit_app_preflight",
        "_httpkit_router_endpoint_count",
        "CORSMiddleware",
        "StaticFiles",
        "Depends",
        "Query",
        "Body",
        "Header",
        "BackgroundTasks",
        "_httpkit_background_tasks_add_task",
        "_httpkit_background_tasks_len",
        "_httpkit_background_tasks_json",
        "RequestContext",
        "StreamingResponse",
    ] {
        assert!(
            symbols.contains(expected),
            "mambalibs.http missing runtime symbol {expected}"
        );
    }
    assert!(
        values.contains("HTTPStatus"),
        "HTTPStatus must be a module value"
    );
}

#[test]
fn background_tasks_binding_records_queue_handoff_metadata() {
    let registrar = registered_module();
    let tasks = call_symbol(&registrar, "BackgroundTasks", &[]);
    assert_eq!(native_type_name(tasks), Some("BackgroundTasks"));

    let payload = dict(&[
        ("user_id", 42i64.into_mb_value()),
        ("email", "a@example.test".to_string().into_mb_value()),
    ]);
    let added = call_symbol(
        &registrar,
        "_httpkit_background_tasks_add_task",
        &[
            tasks,
            "send_email".to_string().into_mb_value(),
            payload,
            "email".to_string().into_mb_value(),
        ],
    );
    assert_eq!(native_type_name(added), Some("BackgroundTasks"));

    let count = call_symbol(&registrar, "_httpkit_background_tasks_len", &[tasks]);
    assert_eq!(count.as_int(), Some(1));

    let serialized = call_symbol(&registrar, "_httpkit_background_tasks_json", &[tasks]);
    let doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(serialized).unwrap()).expect("tasks json");
    assert_eq!(doc[0]["name"].as_str(), Some("send_email"));
    assert_eq!(doc[0]["payload"]["user_id"].as_i64(), Some(42));
    assert_eq!(doc[0]["payload"]["email"].as_str(), Some("a@example.test"));
    assert_eq!(doc[0]["queue"].as_str(), Some("email"));
}

#[test]
fn http_status_is_native_module_value() {
    let registrar = registered_module();
    let value = registrar
        .values()
        .iter()
        .find(|value| value.name == "HTTPStatus")
        .expect("HTTPStatus value must be registered")
        .value();

    let not_found = (ops().dict_get_str)(value, "NOT_FOUND").expect("NOT_FOUND status");
    let created = (ops().dict_get_str)(value, "CREATED").expect("CREATED status");
    assert_eq!(not_found.as_int(), Some(404));
    assert_eq!(created.as_int(), Some(201));
}

#[test]
fn generated_core_constructors_return_typed_native_handles() {
    let registrar = registered_module();
    let args = [
        404i64.into_mb_value(),
        "missing".to_string().into_mb_value(),
        std::collections::HashMap::<String, String>::new().into_mb_value(),
    ];
    let exc_value = call_symbol(&registrar, "HTTPException", &args);

    assert_eq!(native_type_name(exc_value), Some("HTTPException"));
    let exc: &mambalibs_http::http_exception::HTTPException =
        unsafe { mb_unwrap_native_ref(exc_value).expect("typed HTTPException handle") };
    assert_eq!(exc.status_code, 404);
    assert_eq!(exc.detail, "missing");
}

#[test]
fn app_constructor_returns_typed_native_handle() {
    let registrar = registered_module();
    let metadata = std::collections::HashMap::<String, String>::from([
        ("title".to_string(), "Cue".to_string()),
        ("version".to_string(), "0.1.0".to_string()),
    ]);
    let app_value = call_symbol(&registrar, "App", &[metadata.into_mb_value()]);

    assert_eq!(native_type_name(app_value), Some("App"));
    let app: &mambalibs_http_binding::app::App =
        unsafe { mb_unwrap_native_ref(app_value).expect("typed App handle") };
    assert_eq!(app.metadata.get("title").map(String::as_str), Some("Cue"));
}

#[test]
fn depends_records_shared_di_provider_key() {
    let registrar = registered_module();
    let depends_value = call_symbol(
        &registrar,
        "Depends",
        &["current_user".to_string().into_mb_value()],
    );

    assert_eq!(native_type_name(depends_value), Some("Depends"));
    let param: &mambalibs_http_binding::app::Param =
        unsafe { mb_unwrap_native_ref(depends_value).expect("typed Depends handle") };
    assert_eq!(param.kind, mambalibs_http_binding::app::ParamKind::Depends);
    assert_eq!(
        param
            .dependency_key
            .as_ref()
            .map(mambalibs_di::ProviderKey::as_str),
        Some("current_user")
    );
}

#[test]
fn endpoint_contract_combines_http_di_and_dataclasses() {
    use schema_mamba::methods::mb_schema_base_model_new;
    use schema_mamba::types::MbBaseModel;

    let registrar = registered_module();

    let depends_value = call_symbol(
        &registrar,
        "Depends",
        &["current_user".to_string().into_mb_value()],
    );
    let param: &mambalibs_http_binding::app::Param =
        unsafe { mb_unwrap_native_ref(depends_value).expect("typed Depends handle") };
    let dependency_key = param
        .dependency_key
        .as_ref()
        .map(mambalibs_di::ProviderKey::as_str)
        .expect("Depends should carry a DI provider key")
        .to_string();

    let request_model_value =
        unsafe { mb_schema_base_model_new(["ItemCreate".to_string().into_mb_value()].as_ptr(), 1) };
    let response_model_value =
        unsafe { mb_schema_base_model_new(["ItemRead".to_string().into_mb_value()].as_ptr(), 1) };
    assert_eq!(native_type_name(request_model_value), Some("BaseModel"));
    assert_eq!(native_type_name(response_model_value), Some("BaseModel"));
    let request_model: &MbBaseModel =
        unsafe { mb_unwrap_native_ref(request_model_value).expect("typed request model") };
    let response_model: &MbBaseModel =
        unsafe { mb_unwrap_native_ref(response_model_value).expect("typed response model") };

    let endpoint_value = call_symbol(
        &registrar,
        "Endpoint",
        &[
            "POST".to_string().into_mb_value(),
            "/items".to_string().into_mb_value(),
            "create_item".to_string().into_mb_value(),
            vec![dependency_key].into_mb_value(),
            request_model.name.clone().into_mb_value(),
            response_model.name.clone().into_mb_value(),
            201i64.into_mb_value(),
        ],
    );
    assert_eq!(native_type_name(endpoint_value), Some("Endpoint"));

    let endpoint: &mambalibs_http_binding::app::Endpoint =
        unsafe { mb_unwrap_native_ref(endpoint_value).expect("typed Endpoint handle") };
    assert_eq!(endpoint.method, "POST");
    assert_eq!(endpoint.path, "/items");
    assert_eq!(endpoint.handler_name.as_deref(), Some("create_item"));
    assert_eq!(endpoint.dependency_keys, vec!["current_user".to_string()]);
    assert_eq!(endpoint.request_model.as_deref(), Some("ItemCreate"));
    assert_eq!(endpoint.response_model.as_deref(), Some("ItemRead"));
    assert_eq!(endpoint.status_code, 201);

    let app_value = call_symbol(
        &registrar,
        "App",
        &[std::collections::HashMap::<String, String>::new().into_mb_value()],
    );
    let result = call_symbol(
        &registrar,
        "_httpkit_app_add_endpoint",
        &[app_value, endpoint_value],
    );
    assert_eq!(result, app_value);

    let app: &mambalibs_http_binding::app::App =
        unsafe { mb_unwrap_native_ref(app_value).expect("typed App handle") };
    assert_eq!(app.route_count(), 1);
    assert_eq!(app.endpoint_count(), 1);
    assert_eq!(
        app.endpoints()[0].dependency_keys,
        vec!["current_user".to_string()]
    );
}

#[test]
fn route_decorator_factory_preserves_handler() {
    let registrar = registered_module();
    let decorator = call_symbol(
        &registrar,
        "_httpkit_route_factory",
        &["/health".to_string().into_mb_value()],
    );
    let decorator_func = native_fn(decorator, "decorator factory returns function");

    let handler = MbValue::from_func(0x1234);
    let result = unsafe { decorator_func([handler].as_ptr(), 1) };
    assert_eq!(result, handler);
}

#[test]
fn app_get_decorator_registers_endpoint_and_preserves_handler() {
    let registrar = registered_module();
    let app_value = call_symbol(
        &registrar,
        "App",
        &[std::collections::HashMap::<String, String>::new().into_mb_value()],
    );

    let getter_args = [app_value];
    let factory = unsafe {
        mambalibs_http_binding::app::get_route_factory_get(getter_args.as_ptr(), getter_args.len())
    };
    let factory_func = native_fn(factory, "app.get getter returns route factory");
    let factory_args = ["/health".to_string().into_mb_value()];
    let decorator = unsafe { factory_func(factory_args.as_ptr(), factory_args.len()) };
    let decorator_func = native_fn(decorator, "route factory returns decorator");

    let handler = MbValue::from_func(0x1234);
    let decorator_args = [handler];
    let result = unsafe { decorator_func(decorator_args.as_ptr(), decorator_args.len()) };

    assert_eq!(result, handler);
    let app: &mambalibs_http_binding::app::App =
        unsafe { mb_unwrap_native_ref(app_value).expect("typed App handle") };
    assert_eq!(app.endpoint_count(), 1);
    assert_eq!(app.route_count(), 1);
    let endpoint = &app.endpoints()[0];
    assert_eq!(endpoint.method, "GET");
    assert_eq!(endpoint.path, "/health");
    assert_eq!(endpoint.status_code, 200);

    let endpoint_count = call_symbol(&registrar, "_httpkit_app_endpoint_count", &[app_value]);
    assert_eq!(endpoint_count.as_int(), Some(1));
}

#[test]
fn app_post_decorator_registers_status_code() {
    let registrar = registered_module();
    let app_value = call_symbol(
        &registrar,
        "App",
        &[std::collections::HashMap::<String, String>::new().into_mb_value()],
    );

    let getter_args = [app_value];
    let factory = unsafe {
        mambalibs_http_binding::app::get_route_factory_post(getter_args.as_ptr(), getter_args.len())
    };
    let factory_func = native_fn(factory, "app.post getter returns route factory");
    let factory_args = ["/items".to_string().into_mb_value(), 201i64.into_mb_value()];
    let decorator = unsafe { factory_func(factory_args.as_ptr(), factory_args.len()) };
    let decorator_func = native_fn(decorator, "route factory returns decorator");
    let handler = MbValue::from_func(0x5678);
    let decorator_args = [handler];

    let result = unsafe { decorator_func(decorator_args.as_ptr(), decorator_args.len()) };

    assert_eq!(result, handler);
    let app: &mambalibs_http_binding::app::App =
        unsafe { mb_unwrap_native_ref(app_value).expect("typed App handle") };
    assert_eq!(app.endpoint_count(), 1);
    let endpoint = &app.endpoints()[0];
    assert_eq!(endpoint.method, "POST");
    assert_eq!(endpoint.path, "/items");
    assert_eq!(endpoint.status_code, 201);
}

#[test]
fn app_post_decorator_kwargs_register_di_models_and_openapi() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field,
    };

    let registrar = registered_module();
    let app_value = call_symbol(
        &registrar,
        "FastAPI",
        &[std::collections::HashMap::<String, String>::from([
            ("title".to_string(), "Inventory".to_string()),
            ("version".to_string(), "1.2.3".to_string()),
        ])
        .into_mb_value()],
    );
    let depends_value = call_symbol(
        &registrar,
        "Depends",
        &["current_user".to_string().into_mb_value()],
    );
    let dependencies = (ops().list_new)(vec![depends_value]);
    let request_model =
        unsafe { mb_schema_base_model_new(["ItemCreate".to_string().into_mb_value()].as_ptr(), 1) };
    let response_model =
        unsafe { mb_schema_base_model_new(["ItemRead".to_string().into_mb_value()].as_ptr(), 1) };
    let request_field = unsafe {
        mb_schema_field(
            [
                "name".to_string().into_mb_value(),
                dict(&[("min_length", 3i64.into_mb_value())]),
            ]
            .as_ptr(),
            2,
        )
    };
    let response_field = unsafe {
        mb_schema_field(
            [
                "id".to_string().into_mb_value(),
                dict(&[("type", "int".to_string().into_mb_value())]),
            ]
            .as_ptr(),
            2,
        )
    };
    let _ = unsafe { mb_schema_model_add_field([request_model, request_field].as_ptr(), 2) };
    let _ = unsafe { mb_schema_model_add_field([response_model, response_field].as_ptr(), 2) };

    let getter_args = [app_value];
    let factory = unsafe {
        mambalibs_http_binding::app::get_route_factory_post(getter_args.as_ptr(), getter_args.len())
    };
    let factory_func = native_fn(factory, "app.post getter returns route factory");
    let kwargs = dict(&[
        ("status_code", 201i64.into_mb_value()),
        ("dependencies", dependencies),
        ("request_model", request_model),
        ("response_model", response_model),
    ]);
    let factory_args = ["/items".to_string().into_mb_value(), kwargs];
    let decorator = unsafe { factory_func(factory_args.as_ptr(), factory_args.len()) };
    let decorator_func = native_fn(decorator, "route factory returns decorator");
    let handler = MbValue::from_func(0x4321);
    let result = unsafe { decorator_func([handler].as_ptr(), 1) };

    assert_eq!(result, handler);
    let app: &mambalibs_http_binding::app::App =
        unsafe { mb_unwrap_native_ref(app_value).expect("typed App handle") };
    assert_eq!(app.endpoint_count(), 1);
    let endpoint = &app.endpoints()[0];
    assert_eq!(endpoint.method, "POST");
    assert_eq!(endpoint.path, "/items");
    assert_eq!(endpoint.status_code, 201);
    assert_eq!(endpoint.dependency_keys, vec!["current_user".to_string()]);
    assert_eq!(endpoint.request_model.as_deref(), Some("ItemCreate"));
    assert!(
        endpoint
            .request_schema
            .as_deref()
            .is_some_and(|schema| schema.contains("\"minLength\":3")),
        "{endpoint:?}"
    );
    assert_eq!(endpoint.response_model.as_deref(), Some("ItemRead"));
    assert!(
        endpoint
            .response_schema
            .as_deref()
            .is_some_and(|schema| schema.contains("\"integer\"")),
        "{endpoint:?}"
    );

    let openapi = call_symbol(&registrar, "_httpkit_app_openapi", &[app_value]);
    let doc = (ops().str_read)(openapi).expect("openapi returns string");
    assert!(doc.contains("\"/items\""), "{doc}");
    assert!(doc.contains("\"post\""), "{doc}");
    assert!(doc.contains("\"201\""), "{doc}");
    assert!(doc.contains("current_user"), "{doc}");
    assert!(doc.contains("#/components/schemas/ItemCreate"), "{doc}");
    assert!(doc.contains("#/components/schemas/ItemRead"), "{doc}");
    assert!(doc.contains("\"minLength\":3"), "{doc}");
    assert!(doc.contains("\"id\":{\"type\":\"integer\"}"), "{doc}");
}

#[test]
fn app_openapi_preserves_dataclass_field_schema_metadata() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field,
    };

    let registrar = registered_module();
    let app_value = call_symbol(
        &registrar,
        "FastAPI",
        &[std::collections::HashMap::<String, String>::from([(
            "title".to_string(),
            "Catalog".to_string(),
        )])
        .into_mb_value()],
    );
    let model = unsafe {
        mb_schema_base_model_new(["CatalogItem".to_string().into_mb_value()].as_ptr(), 1)
    };
    let name_field = unsafe {
        mb_schema_field(
            [
                "name".to_string().into_mb_value(),
                dict(&[
                    ("type", "str".to_string().into_mb_value()),
                    ("title", "Display Name".to_string().into_mb_value()),
                    (
                        "description",
                        "Public catalog name".to_string().into_mb_value(),
                    ),
                    (
                        "examples",
                        (ops().list_new)(vec![
                            "widget".to_string().into_mb_value(),
                            "gadget".to_string().into_mb_value(),
                        ]),
                    ),
                    ("deprecated", true.into_mb_value()),
                    ("readOnly", true.into_mb_value()),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    let quantity_field = unsafe {
        mb_schema_field(
            [
                "quantity".to_string().into_mb_value(),
                dict(&[
                    ("type", "int".to_string().into_mb_value()),
                    ("multipleOf", 5i64.into_mb_value()),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    let _ = unsafe { mb_schema_model_add_field([model, name_field].as_ptr(), 2) };
    let _ = unsafe { mb_schema_model_add_field([model, quantity_field].as_ptr(), 2) };

    let factory =
        unsafe { mambalibs_http_binding::app::get_route_factory_post([app_value].as_ptr(), 1) };
    let factory_func = native_fn(factory, "app.post getter returns route factory");
    let kwargs = dict(&[("request_model", model), ("response_model", model)]);
    let decorator =
        unsafe { factory_func(["/items".to_string().into_mb_value(), kwargs].as_ptr(), 2) };
    let decorator_func = native_fn(decorator, "route factory returns decorator");
    let _ = unsafe { decorator_func([MbValue::from_func(0x8877)].as_ptr(), 1) };

    let openapi = call_symbol(&registrar, "_httpkit_app_openapi", &[app_value]);
    let doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(openapi).expect("openapi returns string"))
            .expect("openapi should parse");
    let item = &doc["components"]["schemas"]["CatalogItem"]["properties"];
    assert_eq!(item["name"]["title"].as_str(), Some("Display Name"));
    assert_eq!(
        item["name"]["description"].as_str(),
        Some("Public catalog name")
    );
    assert_eq!(item["name"]["deprecated"].as_bool(), Some(true));
    assert_eq!(item["name"]["readOnly"].as_bool(), Some(true));
    assert_eq!(
        item["name"]["examples"].as_array().map(|items| items
            .iter()
            .filter_map(|item| item.as_str())
            .collect::<Vec<_>>()),
        Some(vec!["widget", "gadget"])
    );
    assert_eq!(item["quantity"]["multipleOf"].as_i64(), Some(5));
}

#[test]
fn app_route_parameters_openapi_and_preflight() {
    let registrar = registered_module();
    let app_value = call_symbol(
        &registrar,
        "FastAPI",
        &[std::collections::HashMap::<String, String>::from([(
            "title".to_string(),
            "Search".to_string(),
        )])
        .into_mb_value()],
    );
    let query = call_symbol(
        &registrar,
        "Query",
        &[dict(&[("name", "q".to_string().into_mb_value())])],
    );
    let optional_query = call_symbol(
        &registrar,
        "Query",
        &[MbValue::none(), "filter".to_string().into_mb_value()],
    );
    let header = call_symbol(
        &registrar,
        "Header",
        &[
            "local".to_string().into_mb_value(),
            "X-Trace-ID".to_string().into_mb_value(),
        ],
    );
    let parameters = (ops().list_new)(vec![query, optional_query, header]);

    let factory =
        unsafe { mambalibs_http_binding::app::get_route_factory_get([app_value].as_ptr(), 1) };
    let factory_func = native_fn(factory, "app.get getter returns route factory");
    let kwargs = dict(&[("parameters", parameters)]);
    let decorator =
        unsafe { factory_func(["/search".to_string().into_mb_value(), kwargs].as_ptr(), 2) };
    let decorator_func = native_fn(decorator, "route factory returns decorator");
    let _ = unsafe { decorator_func([MbValue::from_func(0x7788)].as_ptr(), 1) };

    let openapi = call_symbol(&registrar, "_httpkit_app_openapi", &[app_value]);
    let doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(openapi).expect("openapi returns string"))
            .expect("openapi should parse");
    let params = doc["paths"]["/search"]["get"]["parameters"]
        .as_array()
        .expect("parameters");
    assert!(params.iter().any(|param| {
        param["name"].as_str() == Some("q")
            && param["in"].as_str() == Some("query")
            && param["required"].as_bool() == Some(true)
    }));
    assert!(params.iter().any(|param| {
        param["name"].as_str() == Some("X-Trace-ID")
            && param["in"].as_str() == Some("header")
            && param["required"].as_bool() == Some(false)
            && param["schema"]["default"].as_str() == Some("local")
    }));
    assert!(params.iter().any(|param| {
        param["name"].as_str() == Some("filter")
            && param["in"].as_str() == Some("query")
            && param["required"].as_bool() == Some(false)
            && param["schema"]["default"].is_null()
    }));

    let context = dict(&[("query", dict(&[("q", "mamba".to_string().into_mb_value())]))]);
    let ok = call_symbol(
        &registrar,
        "_httpkit_app_preflight",
        &[
            app_value,
            "GET".to_string().into_mb_value(),
            "/search".to_string().into_mb_value(),
            dict(&[]),
            MbValue::none(),
            context,
        ],
    );
    let ok_doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(ok).unwrap()).expect("ok report json");
    assert_eq!(ok_doc["status_code"].as_i64(), Some(200));
    assert_eq!(ok_doc["parameters"]["q"].as_str(), Some("mamba"));
    assert!(ok_doc["parameters"]["filter"].is_null());
    assert_eq!(ok_doc["parameters"]["X-Trace-ID"].as_str(), Some("local"));

    let missing = call_symbol(
        &registrar,
        "_httpkit_app_preflight",
        &[
            app_value,
            "GET".to_string().into_mb_value(),
            "/search".to_string().into_mb_value(),
            dict(&[]),
            MbValue::none(),
        ],
    );
    let missing_doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(missing).unwrap()).expect("missing report json");
    assert_eq!(missing_doc["status_code"].as_i64(), Some(422));
    assert!(missing_doc["detail"]
        .as_array()
        .is_some_and(|details| details
            .iter()
            .any(|detail| detail["loc"] == serde_json::json!(["query", "q"])
                && detail["type"].as_str() == Some("missing"))));
}

#[test]
fn app_preflight_resolves_di_and_normalizes_schema_body() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field,
    };

    let registrar = registered_module();
    let app_value = call_symbol(
        &registrar,
        "FastAPI",
        &[std::collections::HashMap::<String, String>::new().into_mb_value()],
    );
    let request_model =
        unsafe { mb_schema_base_model_new(["ItemCreate".to_string().into_mb_value()].as_ptr(), 1) };
    for (name, kwargs) in [
        (
            "name",
            dict(&[
                ("type", "str".to_string().into_mb_value()),
                ("min_length", 3i64.into_mb_value()),
            ]),
        ),
        (
            "age",
            dict(&[
                ("type", "int".to_string().into_mb_value()),
                ("minimum", 1i64.into_mb_value()),
            ]),
        ),
        (
            "tags",
            dict(&[
                ("type", "list[str]".to_string().into_mb_value()),
                ("default", (ops().list_new)(vec![])),
            ]),
        ),
    ] {
        let field =
            unsafe { mb_schema_field([name.to_string().into_mb_value(), kwargs].as_ptr(), 2) };
        let _ = unsafe { mb_schema_model_add_field([request_model, field].as_ptr(), 2) };
    }

    let depends_value = call_symbol(
        &registrar,
        "Depends",
        &["current_user".to_string().into_mb_value()],
    );
    let dependencies = (ops().list_new)(vec![depends_value]);
    let getter_args = [app_value];
    let factory = unsafe {
        mambalibs_http_binding::app::get_route_factory_post(getter_args.as_ptr(), getter_args.len())
    };
    let factory_func = native_fn(factory, "app.post getter returns route factory");
    let kwargs = dict(&[
        ("status_code", 201i64.into_mb_value()),
        ("dependencies", dependencies),
        ("request_model", request_model),
        ("response_model", request_model),
    ]);
    let decorator =
        unsafe { factory_func(["/items".to_string().into_mb_value(), kwargs].as_ptr(), 2) };
    let decorator_func = native_fn(decorator, "route factory returns decorator");
    let _ = unsafe { decorator_func([MbValue::from_func(0x7777)].as_ptr(), 1) };

    let container = unsafe { mambalibs_di_binding::di_container_new([].as_ptr(), 0) };
    let _ = unsafe {
        mambalibs_di_binding::di_container_register_value(
            [
                container,
                "current_user".to_string().into_mb_value(),
                "alice".to_string().into_mb_value(),
                "request".to_string().into_mb_value(),
            ]
            .as_ptr(),
            4,
        )
    };

    let valid_body = dict(&[
        ("name", "alice".to_string().into_mb_value()),
        ("age", "2".to_string().into_mb_value()),
    ]);
    let report = call_symbol(
        &registrar,
        "_httpkit_app_preflight",
        &[
            app_value,
            "POST".to_string().into_mb_value(),
            "/items".to_string().into_mb_value(),
            valid_body,
            container,
        ],
    );
    let doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(report).unwrap()).expect("report json");
    assert_eq!(doc["matched"].as_bool(), Some(true));
    assert_eq!(doc["status_code"].as_i64(), Some(201));
    assert_eq!(doc["body"]["age"].as_i64(), Some(2));
    assert!(doc["body"]["tags"]
        .as_array()
        .is_some_and(|items| items.is_empty()));
    assert_eq!(doc["dependencies"]["current_user"].as_str(), Some("alice"));

    let invalid_body = dict(&[
        ("name", "al".to_string().into_mb_value()),
        ("age", "two".to_string().into_mb_value()),
        ("tags", (ops().list_new)(vec![7i64.into_mb_value()])),
    ]);
    let invalid = call_symbol(
        &registrar,
        "_httpkit_app_preflight",
        &[
            app_value,
            "POST".to_string().into_mb_value(),
            "/items".to_string().into_mb_value(),
            invalid_body,
            container,
        ],
    );
    let invalid_doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(invalid).unwrap()).expect("invalid report json");
    assert_eq!(invalid_doc["status_code"].as_i64(), Some(422));
    assert!(invalid_doc["errors"]
        .as_array()
        .is_some_and(|errors| errors.iter().any(|error| error
            .as_str()
            .is_some_and(|msg| msg.contains("ValidationError")))));
    assert!(invalid_doc["detail"]
        .as_array()
        .is_some_and(|details| details
            .iter()
            .any(|detail| detail["loc"] == serde_json::json!(["body", "age"])
                && detail["type"].as_str() == Some("type_error"))));
    assert!(invalid_doc["detail"]
        .as_array()
        .is_some_and(|details| details.iter().any(|detail| detail["loc"]
            == serde_json::json!(["body", "name"])
            && detail["msg"]
                .as_str()
                .is_some_and(|msg| msg.contains("at least")))));

    let missing_dependency = call_symbol(
        &registrar,
        "_httpkit_app_preflight",
        &[
            app_value,
            "POST".to_string().into_mb_value(),
            "/items".to_string().into_mb_value(),
            valid_body,
            MbValue::none(),
        ],
    );
    let missing_doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(missing_dependency).unwrap())
            .expect("missing dependency report json");
    assert_eq!(missing_doc["status_code"].as_i64(), Some(500));
    assert!(missing_doc["dependency_errors"]
        .as_array()
        .is_some_and(|errors| errors.iter().any(|error| error
            .as_str()
            .is_some_and(|msg| msg.contains("current_user")))));
}

#[test]
fn app_preflight_reports_nested_validation_detail_locations() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field,
    };

    let registrar = registered_module();
    let app_value = call_symbol(
        &registrar,
        "FastAPI",
        &[std::collections::HashMap::<String, String>::new().into_mb_value()],
    );

    let user_model =
        unsafe { mb_schema_base_model_new(["User".to_string().into_mb_value()].as_ptr(), 1) };
    for (name, kwargs) in [
        (
            "name",
            dict(&[
                ("type", "str".to_string().into_mb_value()),
                ("min_length", 3i64.into_mb_value()),
            ]),
        ),
        (
            "age",
            dict(&[
                ("type", "int".to_string().into_mb_value()),
                ("minimum", 1i64.into_mb_value()),
            ]),
        ),
    ] {
        let field =
            unsafe { mb_schema_field([name.to_string().into_mb_value(), kwargs].as_ptr(), 2) };
        let _ = unsafe { mb_schema_model_add_field([user_model, field].as_ptr(), 2) };
    }

    let team_model =
        unsafe { mb_schema_base_model_new(["Team".to_string().into_mb_value()].as_ptr(), 1) };
    let owner_field = unsafe {
        mb_schema_field(
            [
                "owner".to_string().into_mb_value(),
                dict(&[("model", user_model)]),
            ]
            .as_ptr(),
            2,
        )
    };
    let members_field = unsafe {
        mb_schema_field(
            [
                "members".to_string().into_mb_value(),
                dict(&[
                    ("type", "list".to_string().into_mb_value()),
                    ("items_model", user_model),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    for field in [owner_field, members_field] {
        let _ = unsafe { mb_schema_model_add_field([team_model, field].as_ptr(), 2) };
    }

    let factory =
        unsafe { mambalibs_http_binding::app::get_route_factory_post([app_value].as_ptr(), 1) };
    let factory_func = native_fn(factory, "app.post getter returns route factory");
    let kwargs = dict(&[("request_model", team_model)]);
    let decorator =
        unsafe { factory_func(["/teams".to_string().into_mb_value(), kwargs].as_ptr(), 2) };
    let decorator_func = native_fn(decorator, "route factory returns decorator");
    let _ = unsafe { decorator_func([MbValue::from_func(0x9998)].as_ptr(), 1) };

    let body = dict(&[
        (
            "owner",
            dict(&[
                ("name", "al".to_string().into_mb_value()),
                ("age", "2".to_string().into_mb_value()),
            ]),
        ),
        (
            "members",
            (ops().list_new)(vec![dict(&[
                ("name", "bob".to_string().into_mb_value()),
                ("age", "two".to_string().into_mb_value()),
            ])]),
        ),
    ]);
    let report = call_symbol(
        &registrar,
        "_httpkit_app_preflight",
        &[
            app_value,
            "POST".to_string().into_mb_value(),
            "/teams".to_string().into_mb_value(),
            body,
            MbValue::none(),
        ],
    );
    let doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(report).unwrap()).expect("report json");
    assert_eq!(doc["status_code"].as_i64(), Some(422));
    assert!(doc["detail"]
        .as_array()
        .is_some_and(|details| details.iter().any(|detail| detail["loc"]
            == serde_json::json!(["body", "members", 0, "age"])
            && detail["type"].as_str() == Some("type_error"))));
    assert!(doc["detail"]
        .as_array()
        .is_some_and(|details| details.iter().any(|detail| detail["loc"]
            == serde_json::json!(["body", "owner", "name"])
            && detail["type"].as_str() == Some("value_error"))));
}

#[test]
fn test_client_dispatches_preflight_with_di_and_schema() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field,
    };

    let registrar = registered_module();
    let app_value = call_symbol(
        &registrar,
        "FastAPI",
        &[std::collections::HashMap::<String, String>::new().into_mb_value()],
    );
    let request_model =
        unsafe { mb_schema_base_model_new(["ItemCreate".to_string().into_mb_value()].as_ptr(), 1) };
    for (name, kwargs) in [
        (
            "name",
            dict(&[
                ("type", "str".to_string().into_mb_value()),
                ("min_length", 3i64.into_mb_value()),
            ]),
        ),
        (
            "age",
            dict(&[
                ("type", "int".to_string().into_mb_value()),
                ("minimum", 1i64.into_mb_value()),
            ]),
        ),
        (
            "tags",
            dict(&[
                ("type", "list[str]".to_string().into_mb_value()),
                ("default", (ops().list_new)(vec![])),
            ]),
        ),
    ] {
        let field =
            unsafe { mb_schema_field([name.to_string().into_mb_value(), kwargs].as_ptr(), 2) };
        let _ = unsafe { mb_schema_model_add_field([request_model, field].as_ptr(), 2) };
    }

    let depends_value = call_symbol(
        &registrar,
        "Depends",
        &["current_user".to_string().into_mb_value()],
    );
    let dependencies = (ops().list_new)(vec![depends_value]);
    let factory =
        unsafe { mambalibs_http_binding::app::get_route_factory_post([app_value].as_ptr(), 1) };
    let factory_func = native_fn(factory, "app.post getter returns route factory");
    let kwargs = dict(&[
        ("status_code", 201i64.into_mb_value()),
        ("dependencies", dependencies),
        ("request_model", request_model),
        ("response_model", request_model),
    ]);
    let decorator =
        unsafe { factory_func(["/items".to_string().into_mb_value(), kwargs].as_ptr(), 2) };
    let decorator_func = native_fn(decorator, "route factory returns decorator");
    let _ = unsafe { decorator_func([MbValue::from_func(0x8888)].as_ptr(), 1) };

    let container = unsafe { mambalibs_di_binding::di_container_new([].as_ptr(), 0) };
    let _ = unsafe {
        mambalibs_di_binding::di_container_register_value(
            [
                container,
                "current_user".to_string().into_mb_value(),
                "alice".to_string().into_mb_value(),
                "request".to_string().into_mb_value(),
            ]
            .as_ptr(),
            4,
        )
    };

    let client = call_symbol(&registrar, "TestClient", &[app_value, container]);
    assert_eq!(native_type_name(client), Some("TestClient"));

    let valid_body = dict(&[
        ("name", "alice".to_string().into_mb_value()),
        ("age", "2".to_string().into_mb_value()),
    ]);
    let post = unsafe {
        mambalibs_http_binding::client::test_client::get_test_client_post([client].as_ptr(), 1)
    };
    let post_func = native_fn(post, "TestClient.post getter returns bound function");
    let response = unsafe {
        post_func(
            ["/items".to_string().into_mb_value(), valid_body].as_ptr(),
            2,
        )
    };
    assert_eq!(native_type_name(response), Some("TestResponse"));
    let status = call_symbol(&registrar, "test_client_status", &[response]);
    assert_eq!(status.as_int(), Some(201));
    let report = call_symbol(&registrar, "test_client_json", &[response]);
    let body = (ops().dict_get_str)(report, "body").expect("report body");
    assert_eq!(
        (ops().dict_get_str)(body, "age").and_then(|value| value.as_int()),
        Some(2)
    );
    let tags = (ops().dict_get_str)(body, "tags").expect("defaulted tags");
    assert_eq!((ops().list_len)(tags), Some(0));
    let dependencies = (ops().dict_get_str)(report, "dependencies").expect("dependencies");
    assert_eq!(
        (ops().dict_get_str)(dependencies, "current_user")
            .and_then(|value| (ops().str_read)(value))
            .as_deref(),
        Some("alice")
    );

    let raw_json_response = unsafe {
        post_func(
            [
                "/items".to_string().into_mb_value(),
                r#"{"name":"alice","age":"3"}"#.to_string().into_mb_value(),
            ]
            .as_ptr(),
            2,
        )
    };
    let raw_status = call_symbol(&registrar, "test_client_status", &[raw_json_response]);
    assert_eq!(raw_status.as_int(), Some(201));
    let raw_report = call_symbol(&registrar, "test_client_json", &[raw_json_response]);
    let raw_body = (ops().dict_get_str)(raw_report, "body").expect("raw report body");
    assert_eq!(
        (ops().dict_get_str)(raw_body, "age").and_then(|value| value.as_int()),
        Some(3)
    );
    let raw_dependencies =
        (ops().dict_get_str)(raw_report, "dependencies").expect("raw dependencies");
    assert_eq!(
        (ops().dict_get_str)(raw_dependencies, "current_user")
            .and_then(|value| (ops().str_read)(value))
            .as_deref(),
        Some("alice")
    );

    let invalid_raw_response = unsafe {
        post_func(
            [
                "/items".to_string().into_mb_value(),
                r#"{"name":"alice","age":}"#.to_string().into_mb_value(),
            ]
            .as_ptr(),
            2,
        )
    };
    let invalid_raw_status = call_symbol(&registrar, "test_client_status", &[invalid_raw_response]);
    assert_eq!(invalid_raw_status.as_int(), Some(422));
    let invalid_raw_report = call_symbol(&registrar, "test_client_json", &[invalid_raw_response]);
    let invalid_raw_detail =
        (ops().dict_get_str)(invalid_raw_report, "detail").expect("raw detail");
    assert!((ops().list_len)(invalid_raw_detail).is_some_and(|len| len >= 1));

    let invalid_body = dict(&[
        ("name", "al".to_string().into_mb_value()),
        ("age", "two".to_string().into_mb_value()),
    ]);
    let invalid = call_symbol(
        &registrar,
        "test_client_post",
        &[client, "/items".to_string().into_mb_value(), invalid_body],
    );
    let invalid_status = call_symbol(&registrar, "test_client_status", &[invalid]);
    assert_eq!(invalid_status.as_int(), Some(422));
    let invalid_report = call_symbol(&registrar, "test_client_json", &[invalid]);
    let detail = (ops().dict_get_str)(invalid_report, "detail").expect("detail");
    assert!(detail_has_loc_type(detail, "age", "type_error"));

    let get = unsafe {
        mambalibs_http_binding::client::test_client::get_test_client_get([client].as_ptr(), 1)
    };
    let get_func = native_fn(get, "TestClient.get getter returns bound function");
    let missing = unsafe { get_func(["/missing".to_string().into_mb_value()].as_ptr(), 1) };
    let missing_status = call_symbol(&registrar, "test_client_status", &[missing]);
    assert_eq!(missing_status.as_int(), Some(404));
}

#[test]
fn test_client_dispatches_handler_and_response_model() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field,
    };

    TEST_HANDLER_CALLS.store(0, Ordering::SeqCst);
    let registrar = registered_module();
    let app_value = call_symbol(
        &registrar,
        "FastAPI",
        &[std::collections::HashMap::<String, String>::new().into_mb_value()],
    );
    let model =
        unsafe { mb_schema_base_model_new(["ItemRead".to_string().into_mb_value()].as_ptr(), 1) };
    for (name, kwargs) in [
        (
            "name",
            dict(&[
                ("type", "str".to_string().into_mb_value()),
                ("min_length", 3i64.into_mb_value()),
            ]),
        ),
        (
            "age",
            dict(&[
                ("type", "int".to_string().into_mb_value()),
                ("minimum", 1i64.into_mb_value()),
            ]),
        ),
        (
            "tags",
            dict(&[
                ("type", "list[str]".to_string().into_mb_value()),
                ("default", (ops().list_new)(vec![])),
            ]),
        ),
    ] {
        let field =
            unsafe { mb_schema_field([name.to_string().into_mb_value(), kwargs].as_ptr(), 2) };
        let _ = unsafe { mb_schema_model_add_field([model, field].as_ptr(), 2) };
    }

    let factory =
        unsafe { mambalibs_http_binding::app::get_route_factory_post([app_value].as_ptr(), 1) };
    let factory_func = native_fn(factory, "app.post getter returns route factory");
    let kwargs = dict(&[
        ("status_code", 201i64.into_mb_value()),
        ("request_model", model),
        ("response_model", model),
    ]);
    let decorator =
        unsafe { factory_func(["/items".to_string().into_mb_value(), kwargs].as_ptr(), 2) };
    let decorator_func = native_fn(decorator, "route factory returns decorator");
    let handler = test_ops::register_call0(response_model_handler);
    let _ = unsafe { decorator_func([handler].as_ptr(), 1) };

    let client = call_symbol(&registrar, "TestClient", &[app_value]);
    let valid_body = dict(&[
        ("name", "alice".to_string().into_mb_value()),
        ("age", "2".to_string().into_mb_value()),
    ]);
    let response = call_symbol(
        &registrar,
        "test_client_post",
        &[client, "/items".to_string().into_mb_value(), valid_body],
    );
    let status = call_symbol(&registrar, "test_client_status", &[response]);
    assert_eq!(status.as_int(), Some(201));
    let body = call_symbol(&registrar, "test_client_json", &[response]);
    assert_eq!(
        (ops().dict_get_str)(body, "name")
            .and_then(|value| (ops().str_read)(value))
            .as_deref(),
        Some("created")
    );
    assert_eq!(
        (ops().dict_get_str)(body, "age").and_then(|value| value.as_int()),
        Some(4)
    );
    let tags = (ops().dict_get_str)(body, "tags").expect("response default tags");
    assert_eq!((ops().list_len)(tags), Some(0));
    assert_eq!(TEST_HANDLER_CALLS.load(Ordering::SeqCst), 1);

    let invalid_body = dict(&[
        ("name", "al".to_string().into_mb_value()),
        ("age", "two".to_string().into_mb_value()),
    ]);
    let invalid = call_symbol(
        &registrar,
        "test_client_post",
        &[client, "/items".to_string().into_mb_value(), invalid_body],
    );
    let invalid_status = call_symbol(&registrar, "test_client_status", &[invalid]);
    assert_eq!(invalid_status.as_int(), Some(422));
    let invalid_report = call_symbol(&registrar, "test_client_json", &[invalid]);
    let detail = (ops().dict_get_str)(invalid_report, "detail").expect("detail");
    assert!(detail_has_loc_type(detail, "age", "type_error"));
    assert_eq!(
        TEST_HANDLER_CALLS.load(Ordering::SeqCst),
        1,
        "handler should not execute for invalid request preflight"
    );
}

#[test]
fn run_surface_returns_server_handle() {
    let registrar = registered_module();
    let app_value = call_symbol(
        &registrar,
        "FastAPI",
        &[std::collections::HashMap::<String, String>::from([(
            "title".to_string(),
            "Run Surface".to_string(),
        )])
        .into_mb_value()],
    );
    let factory =
        unsafe { mambalibs_http_binding::app::get_route_factory_get([app_value].as_ptr(), 1) };
    let factory_func = native_fn(factory, "app.get getter returns route factory");
    let decorator = unsafe { factory_func(["/health".to_string().into_mb_value()].as_ptr(), 1) };
    let decorator_func = native_fn(decorator, "route factory returns decorator");
    let _ = unsafe { decorator_func([MbValue::from_func(0x9999)].as_ptr(), 1) };

    let server = call_symbol(
        &registrar,
        "run",
        &[
            app_value,
            "127.0.0.1".to_string().into_mb_value(),
            0i64.into_mb_value(),
        ],
    );
    assert_eq!(native_type_name(server), Some("Server"));
    assert_eq!(
        (ops().str_read)(unsafe {
            mambalibs_http_binding::server::get_server_url([server].as_ptr(), 1)
        })
        .as_deref(),
        Some("http://127.0.0.1:0")
    );
    assert_eq!(
        (ops().str_read)(unsafe {
            mambalibs_http_binding::server::get_server_host([server].as_ptr(), 1)
        })
        .as_deref(),
        Some("127.0.0.1")
    );
    assert_eq!(
        unsafe { mambalibs_http_binding::server::get_server_port([server].as_ptr(), 1) }.as_int(),
        Some(0)
    );
    assert_eq!(
        unsafe { mambalibs_http_binding::server::get_server_running([server].as_ptr(), 1) }
            .as_bool(),
        Some(true)
    );
    assert_eq!(
        unsafe { mambalibs_http_binding::server::get_server_endpoint_count([server].as_ptr(), 1) }
            .as_int(),
        Some(1)
    );
    let openapi = (ops().str_read)(unsafe {
        mambalibs_http_binding::server::get_server_openapi([server].as_ptr(), 1)
    })
    .expect("server openapi string");
    assert!(openapi.contains("\"/health\""), "{openapi}");

    let stop = unsafe { mambalibs_http_binding::server::get_server_stop([server].as_ptr(), 1) };
    let stop_func = native_fn(stop, "server.stop getter returns bound function");
    let result = unsafe { stop_func([].as_ptr(), 0) };
    assert_eq!(result, server);
    assert_eq!(
        unsafe { mambalibs_http_binding::server::get_server_running([server].as_ptr(), 1) }
            .as_bool(),
        Some(false)
    );

    let app_run = unsafe { mambalibs_http_binding::server::get_app_run([app_value].as_ptr(), 1) };
    let app_run_func = native_fn(app_run, "app.run getter returns bound function");
    let app_server = unsafe {
        app_run_func(
            [
                "0.0.0.0".to_string().into_mb_value(),
                9000i64.into_mb_value(),
            ]
            .as_ptr(),
            2,
        )
    };
    assert_eq!(native_type_name(app_server), Some("Server"));
    assert_eq!(
        (ops().str_read)(unsafe {
            mambalibs_http_binding::server::get_server_url([app_server].as_ptr(), 1)
        })
        .as_deref(),
        Some("http://0.0.0.0:9000")
    );
}

#[test]
fn router_delete_decorator_registers_endpoint() {
    let registrar = registered_module();
    let router_value = call_symbol(&registrar, "Router", &[]);

    let getter_args = [router_value];
    let factory = unsafe {
        mambalibs_http_binding::app::get_route_factory_delete(
            getter_args.as_ptr(),
            getter_args.len(),
        )
    };
    let factory_func = native_fn(factory, "router.delete getter returns route factory");
    let factory_args = ["/items/{id}".to_string().into_mb_value()];
    let decorator = unsafe { factory_func(factory_args.as_ptr(), factory_args.len()) };
    let decorator_func = native_fn(decorator, "route factory returns decorator");
    let handler = MbValue::from_func(0x9012);
    let decorator_args = [handler];

    let result = unsafe { decorator_func(decorator_args.as_ptr(), decorator_args.len()) };

    assert_eq!(result, handler);
    let router: &mambalibs_http_binding::app::Router =
        unsafe { mb_unwrap_native_ref(router_value).expect("typed Router handle") };
    assert_eq!(router.endpoint_count(), 1);
    assert_eq!(router.routes.len(), 1);
    let endpoint = &router.endpoints[0];
    assert_eq!(endpoint.method, "DELETE");
    assert_eq!(endpoint.path, "/items/{id}");

    let endpoint_count = call_symbol(
        &registrar,
        "_httpkit_router_endpoint_count",
        &[router_value],
    );
    assert_eq!(endpoint_count.as_int(), Some(1));
}

#[test]
fn request_json_returns_dict_value() {
    let registrar = registered_module();
    let result = call_symbol(&registrar, "_request_json", &[]);
    let empty = std::collections::HashMap::<String, String>::from_mb_value(result)
        .expect("request json returns dict");
    assert!(empty.is_empty());
}
