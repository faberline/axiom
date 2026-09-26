//! Client-side HTTP bindings for `mambalibs.http`.
//!
//! Exposes the async HTTP client (`mambalibs_http::client`) to Mamba scripts under
//! the `mambalibs.http` namespace. Symbols:
//!
//! - `Client`, `client_get`, `client_post`, `client_put`, `client_delete`
//! - `response_status`, `response_text`, `response_json`
//! - `TestClient`, `test_client_close`, `test_client_get`, `test_client_post`,
//!   `test_client_status`, `test_client_text`, `test_client_json`
//! - TestClient methods: `get`, `post`, `close`
//! - TestResponse accessors: `status_code`, `text`, `json`

pub mod methods;
pub mod test_client;
pub mod types;

use mamba_registry::{rt_sym, ModuleRegistrar};

type NativeFn =
    unsafe extern "C" fn(*const mamba_registry::MbValue, usize) -> mamba_registry::MbValue;

fn register_getter(type_name: &str, attr: &str, getter: NativeFn) {
    if let Some(o) = mamba_registry::ops::OBJECT_OPS.get() {
        (o.register_getter)(type_name, attr, getter);
    }
}

pub fn register(r: &mut ModuleRegistrar) {
    use self::methods::{
        mb_fetch_client_delete, mb_fetch_client_get, mb_fetch_client_new, mb_fetch_client_post,
        mb_fetch_client_put, mb_fetch_response_json, mb_fetch_response_status,
        mb_fetch_response_text,
    };
    use self::test_client::{
        get_test_client_close, get_test_client_get, get_test_client_post, get_test_response_json,
        get_test_response_status, get_test_response_text, mb_fetch_test_client_close,
        mb_fetch_test_client_get, mb_fetch_test_client_json, mb_fetch_test_client_new,
        mb_fetch_test_client_post, mb_fetch_test_client_status, mb_fetch_test_client_text,
    };

    r.add_symbols([
        rt_sym!(
            "Client",
            mb_fetch_client_new,
            "Client(base_url: str, timeout: float?) -> client"
        ),
        rt_sym!(
            "client_get",
            mb_fetch_client_get,
            "client_get(client, path: str) -> response"
        ),
        rt_sym!(
            "client_post",
            mb_fetch_client_post,
            "client_post(client, path: str, body: str?) -> response"
        ),
        rt_sym!(
            "client_put",
            mb_fetch_client_put,
            "client_put(client, path: str, body: str?) -> response"
        ),
        rt_sym!(
            "client_delete",
            mb_fetch_client_delete,
            "client_delete(client, path: str) -> response"
        ),
        rt_sym!(
            "response_status",
            mb_fetch_response_status,
            "response_status(response) -> int"
        ),
        rt_sym!(
            "response_text",
            mb_fetch_response_text,
            "response_text(response) -> str"
        ),
        rt_sym!(
            "response_json",
            mb_fetch_response_json,
            "response_json(response) -> str"
        ),
        rt_sym!(
            "TestClient",
            mb_fetch_test_client_new,
            "TestClient(app, provider: Container | RequestScope | dict | None = None) -> test_client"
        ),
        rt_sym!(
            "test_client_close",
            mb_fetch_test_client_close,
            "test_client_close(client) -> None"
        ),
        rt_sym!(
            "test_client_get",
            mb_fetch_test_client_get,
            "test_client_get(client, path: str) -> response"
        ),
        rt_sym!(
            "test_client_post",
            mb_fetch_test_client_post,
            "test_client_post(client, path: str, body: str?) -> response"
        ),
        rt_sym!(
            "test_client_status",
            mb_fetch_test_client_status,
            "test_client_status(response) -> int"
        ),
        rt_sym!(
            "test_client_text",
            mb_fetch_test_client_text,
            "test_client_text(response) -> str"
        ),
        rt_sym!(
            "test_client_json",
            mb_fetch_test_client_json,
            "test_client_json(response) -> dict | str"
        ),
    ]);

    register_getter("TestClient", "get", get_test_client_get);
    register_getter("TestClient", "post", get_test_client_post);
    register_getter("TestClient", "close", get_test_client_close);
    register_getter("TestResponse", "status_code", get_test_response_status);
    register_getter("TestResponse", "text", get_test_response_text);
    register_getter("TestResponse", "json", get_test_response_json);
}
