use mamba_registry::{
    convert::{mb_unwrap_native_ref, native_type_name},
    find_module, test_ops, FromMbValue, IntoMbValue, MbValue, ModuleRegistrar,
};
use mambalibs_di_binding as _;
use std::collections::{HashMap, HashSet};

type NativeFn = unsafe extern "C" fn(*const MbValue, usize) -> MbValue;

fn registered_module() -> ModuleRegistrar {
    test_ops::init();
    let module = find_module("mambalibs.di").expect("mambalibs.di must register");
    let mut registrar = ModuleRegistrar::new();
    module.register(&mut registrar);
    registrar
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

#[test]
fn mambalibs_di_exposes_core_symbols() {
    let registrar = registered_module();
    let symbols: HashSet<&str> = registrar.symbols().iter().map(|sym| sym.name).collect();

    for expected in [
        "Container",
        "RequestScope",
        "Depends",
        "container_register_value",
        "container_override_value",
        "container_clear_override",
        "container_resolve",
        "container_resolve_many",
        "scope_resolve",
        "scope_resolve_many",
    ] {
        assert!(
            symbols.contains(expected),
            "mambalibs.di missing symbol {expected}"
        );
    }
}

#[test]
fn container_register_and_resolve_round_trips_string() {
    let registrar = registered_module();
    let container = call_symbol(&registrar, "Container", &[]);
    assert_eq!(native_type_name(container), Some("Container"));

    call_symbol(
        &registrar,
        "container_register_value",
        &[
            container,
            "config".into_mb_value(),
            "prod".into_mb_value(),
            "singleton".into_mb_value(),
        ],
    );
    let value = call_symbol(
        &registrar,
        "container_resolve",
        &[container, "config".into_mb_value()],
    );

    let text = unsafe { value.as_obj_str() }.expect("resolved string");
    assert_eq!(text, "prod");
}

#[test]
fn container_resolve_many_returns_dependency_dict() {
    let registrar = registered_module();
    let container = call_symbol(&registrar, "Container", &[]);
    for (key, value) in [("settings", "prod"), ("client", "http")] {
        call_symbol(
            &registrar,
            "container_register_value",
            &[
                container,
                key.into_mb_value(),
                value.into_mb_value(),
                "singleton".into_mb_value(),
            ],
        );
    }

    let resolved = call_symbol(
        &registrar,
        "container_resolve_many",
        &[
            container,
            vec!["settings".to_string(), "client".to_string()].into_mb_value(),
        ],
    );

    let map = HashMap::<String, String>::from_mb_value(resolved).expect("resolved dict");
    assert_eq!(map.get("settings").map(String::as_str), Some("prod"));
    assert_eq!(map.get("client").map(String::as_str), Some("http"));
}

#[test]
fn request_scope_resolves_request_value() {
    let registrar = registered_module();
    let container = call_symbol(&registrar, "Container", &[]);
    call_symbol(
        &registrar,
        "container_register_value",
        &[
            container,
            "db".into_mb_value(),
            "session".into_mb_value(),
            "request".into_mb_value(),
        ],
    );

    let scope = call_symbol(&registrar, "RequestScope", &[container]);
    assert_eq!(native_type_name(scope), Some("RequestScope"));
    let value = call_symbol(&registrar, "scope_resolve", &[scope, "db".into_mb_value()]);

    let text = unsafe { value.as_obj_str() }.expect("resolved string");
    assert_eq!(text, "session");
}

#[test]
fn request_scope_resolve_many_returns_dependency_dict() {
    let registrar = registered_module();
    let container = call_symbol(&registrar, "Container", &[]);
    call_symbol(
        &registrar,
        "container_register_value",
        &[
            container,
            "db".into_mb_value(),
            "session".into_mb_value(),
            "request".into_mb_value(),
        ],
    );
    call_symbol(
        &registrar,
        "container_register_value",
        &[
            container,
            "current_user".into_mb_value(),
            "alice".into_mb_value(),
            "request".into_mb_value(),
        ],
    );

    let scope = call_symbol(&registrar, "RequestScope", &[container]);
    let resolved = call_symbol(
        &registrar,
        "scope_resolve_many",
        &[
            scope,
            vec!["db".to_string(), "current_user".to_string()].into_mb_value(),
        ],
    );

    let map = HashMap::<String, String>::from_mb_value(resolved).expect("resolved dict");
    assert_eq!(map.get("db").map(String::as_str), Some("session"));
    assert_eq!(map.get("current_user").map(String::as_str), Some("alice"));
}

#[test]
fn override_wins_for_binding_resolution() {
    let registrar = registered_module();
    let container = call_symbol(&registrar, "Container", &[]);
    call_symbol(
        &registrar,
        "container_register_value",
        &[
            container,
            "client".into_mb_value(),
            "real".into_mb_value(),
            "singleton".into_mb_value(),
        ],
    );
    call_symbol(
        &registrar,
        "container_override_value",
        &[container, "client".into_mb_value(), "fake".into_mb_value()],
    );

    let value = call_symbol(
        &registrar,
        "container_resolve",
        &[container, "client".into_mb_value()],
    );

    let text = unsafe { value.as_obj_str() }.expect("resolved string");
    assert_eq!(text, "fake");
}

#[test]
fn depends_returns_marker_with_optional_key() {
    let registrar = registered_module();
    let marker = call_symbol(&registrar, "Depends", &["current_user".into_mb_value()]);
    assert_eq!(native_type_name(marker), Some("Depends"));

    let marker: &mambalibs_di::DependencyMarker =
        unsafe { mb_unwrap_native_ref(marker).expect("typed dependency marker") };
    assert_eq!(
        marker.key().map(mambalibs_di::ProviderKey::as_str),
        Some("current_user")
    );
}
