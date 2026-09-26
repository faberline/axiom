// Integration tests for agent-mamba: covers all 13 mb_agent_* functions.
// Requirements: R1, R2, R4, R5, R6
#![allow(improper_ctypes_definitions)]

use agentkit_binding::methods::{
    mb_agent_builder_build, mb_agent_builder_new, mb_agent_builder_provider,
    mb_agent_builder_system_prompt, mb_agent_claude_provider, mb_agent_gemini_provider,
    mb_agent_message_content, mb_agent_message_new, mb_agent_message_role,
    mb_agent_openai_provider, mb_agent_run, mb_agent_schema_array, mb_agent_schema_boolean,
    mb_agent_schema_build, mb_agent_schema_field, mb_agent_schema_integer, mb_agent_schema_null,
    mb_agent_schema_number, mb_agent_schema_object, mb_agent_schema_optional,
    mb_agent_schema_required, mb_agent_schema_string, mb_agent_schema_validate,
    mb_agent_tool_registry_new, mb_agent_tool_registry_register,
};
use agentkit_binding::types::{
    MbAgentBuilder, MbLlmAgent, MbMessage, MbProvider, MbSchema, MbSchemaBuilder, MbToolRegistry,
};
use mamba_registry::MbValue;

// ── Shared helpers ─────────────────────────────────────────────────────────────

fn make_str_val(s: &str) -> MbValue {
    mamba_registry::test_ops::init();
    mamba_registry::rc::wrap_obj_str(s.to_string())
}

unsafe fn read_str_val(v: MbValue) -> String {
    mamba_registry::test_ops::init();
    unsafe { mamba_registry::rc::read_obj_str(v) }.expect("expected a Str MbObject")
}

// ── mb_agent_builder_new ──────────────────────────────────────────────────────

#[test]
fn builder_new_empty() {
    let args: [MbValue; 0] = [];
    let builder_val = unsafe { mb_agent_builder_new(args.as_ptr(), 0) };
    assert!(builder_val.is_ptr(), "builder should be a ptr");
    let addr = builder_val.as_ptr().unwrap();
    let builder = unsafe { &*(addr as *const MbAgentBuilder) };
    assert!(
        builder.provider_name.is_empty(),
        "provider_name should be empty"
    );
    assert!(builder.api_key.is_empty(), "api_key should be empty");
    assert!(
        builder.system_prompt.is_empty(),
        "system_prompt should be empty"
    );
}

// ── mb_agent_claude_provider ──────────────────────────────────────────────────

#[test]
fn claude_provider_happy() {
    let args = [make_str_val("sk-ant-test-key")];
    let provider_val = unsafe { mb_agent_claude_provider(args.as_ptr(), 1) };
    assert!(provider_val.is_ptr());
    let addr = provider_val.as_ptr().unwrap();
    let provider = unsafe { &*(addr as *const MbProvider) };
    assert_eq!(provider.name, "claude");
    assert_eq!(provider.api_key, "sk-ant-test-key");
}

#[test]
fn claude_provider_empty_key() {
    let args = [make_str_val("")];
    let provider_val = unsafe { mb_agent_claude_provider(args.as_ptr(), 1) };
    assert!(provider_val.is_ptr());
    let addr = provider_val.as_ptr().unwrap();
    let provider = unsafe { &*(addr as *const MbProvider) };
    assert_eq!(provider.name, "claude");
    assert_eq!(provider.api_key, "", "empty api_key should be preserved");
}

// ── mb_agent_gemini_provider ──────────────────────────────────────────────────

#[test]
fn gemini_provider_happy() {
    let args = [make_str_val("gemini-api-key-xyz")];
    let provider_val = unsafe { mb_agent_gemini_provider(args.as_ptr(), 1) };
    assert!(provider_val.is_ptr());
    let addr = provider_val.as_ptr().unwrap();
    let provider = unsafe { &*(addr as *const MbProvider) };
    assert_eq!(provider.name, "gemini");
    assert_eq!(provider.api_key, "gemini-api-key-xyz");
}

// ── mb_agent_openai_provider ──────────────────────────────────────────────────

#[test]
fn openai_provider_happy() {
    let args = [make_str_val("sk-openai-test")];
    let provider_val = unsafe { mb_agent_openai_provider(args.as_ptr(), 1) };
    assert!(provider_val.is_ptr());
    let addr = provider_val.as_ptr().unwrap();
    let provider = unsafe { &*(addr as *const MbProvider) };
    assert_eq!(provider.name, "openai");
    assert_eq!(provider.api_key, "sk-openai-test");
}

// ── mb_agent_builder_provider ─────────────────────────────────────────────────

#[test]
fn builder_set_provider() {
    let builder_val = unsafe { mb_agent_builder_new([].as_ptr(), 0) };

    let claude_val = unsafe { mb_agent_claude_provider([make_str_val("my-key")].as_ptr(), 1) };
    let set_args = [builder_val, claude_val];
    let result = unsafe { mb_agent_builder_provider(set_args.as_ptr(), 2) };
    assert!(result.is_none());

    let addr = builder_val.as_ptr().unwrap();
    let builder = unsafe { &*(addr as *const MbAgentBuilder) };
    assert_eq!(builder.provider_name, "claude");
    assert_eq!(builder.api_key, "my-key");
}

#[test]
fn builder_set_provider_null() {
    let args = [MbValue::none(), make_str_val("some-val")];
    let result = unsafe { mb_agent_builder_provider(args.as_ptr(), 2) };
    assert!(
        result.is_none(),
        "builder_provider with null builder should return none()"
    );
}

// ── mb_agent_builder_system_prompt ────────────────────────────────────────────

#[test]
fn builder_system_prompt() {
    let builder_val = unsafe { mb_agent_builder_new([].as_ptr(), 0) };
    let args = [builder_val, make_str_val("You are a helpful assistant.")];
    let result = unsafe { mb_agent_builder_system_prompt(args.as_ptr(), 2) };
    assert!(result.is_none());

    let addr = builder_val.as_ptr().unwrap();
    let builder = unsafe { &*(addr as *const MbAgentBuilder) };
    assert_eq!(builder.system_prompt, "You are a helpful assistant.");
}

#[test]
fn builder_system_prompt_null() {
    let args = [MbValue::none(), make_str_val("prompt")];
    let result = unsafe { mb_agent_builder_system_prompt(args.as_ptr(), 2) };
    assert!(
        result.is_none(),
        "system_prompt with null builder should return none()"
    );
}

// ── mb_agent_builder_build ────────────────────────────────────────────────────

#[test]
fn builder_build_configured() {
    let builder_val = unsafe { mb_agent_builder_new([].as_ptr(), 0) };

    let provider_val =
        unsafe { mb_agent_claude_provider([make_str_val("api-key-123")].as_ptr(), 1) };
    unsafe { mb_agent_builder_provider([builder_val, provider_val].as_ptr(), 2) };
    unsafe {
        mb_agent_builder_system_prompt([builder_val, make_str_val("Be helpful.")].as_ptr(), 2)
    };

    let agent_val = unsafe { mb_agent_builder_build([builder_val].as_ptr(), 1) };
    assert!(agent_val.is_ptr());
    let addr = agent_val.as_ptr().unwrap();
    let agent = unsafe { &*(addr as *const MbLlmAgent) };
    assert_eq!(agent.provider_name, "claude");
    assert_eq!(agent.api_key, "api-key-123");
    assert_eq!(agent.system_prompt, "Be helpful.");
}

#[test]
fn builder_build_null() {
    // Null builder → returns agent with empty fields (no crash)
    let agent_val = unsafe { mb_agent_builder_build([MbValue::none()].as_ptr(), 1) };
    assert!(
        agent_val.is_ptr(),
        "build with null should still return a ptr"
    );
    let addr = agent_val.as_ptr().unwrap();
    let agent = unsafe { &*(addr as *const MbLlmAgent) };
    assert!(
        agent.provider_name.is_empty(),
        "provider_name should be empty for null build"
    );
}

// ── mb_agent_run ──────────────────────────────────────────────────────────────

#[test]
fn agent_run_stub() {
    // Build a gemini agent and run it — should return a stub response
    let builder_val = unsafe { mb_agent_builder_new([].as_ptr(), 0) };
    let provider_val =
        unsafe { mb_agent_gemini_provider([make_str_val("gemini-key")].as_ptr(), 1) };
    unsafe { mb_agent_builder_provider([builder_val, provider_val].as_ptr(), 2) };
    let agent_val = unsafe { mb_agent_builder_build([builder_val].as_ptr(), 1) };

    let run_args = [agent_val, make_str_val("What is 2+2?")];
    let resp_val = unsafe { mb_agent_run(run_args.as_ptr(), 2) };
    assert!(resp_val.is_ptr());
    let resp = unsafe { read_str_val(resp_val) };
    assert!(
        resp.contains("stub"),
        "gemini agent response should contain 'stub': {resp}"
    );
}

#[test]
fn agent_run_null_agent() {
    let args = [MbValue::none(), make_str_val("hello")];
    let resp_val = unsafe { mb_agent_run(args.as_ptr(), 2) };
    assert!(resp_val.is_ptr());
    let resp = unsafe { read_str_val(resp_val) };
    assert!(
        resp.contains("error"),
        "null agent response should contain 'error': {resp}"
    );
}

// ── mb_agent_message_new ──────────────────────────────────────────────────────

#[test]
fn message_new_happy() {
    let args = [make_str_val("user"), make_str_val("Hello, world!")];
    let msg_val = unsafe { mb_agent_message_new(args.as_ptr(), 2) };
    assert!(msg_val.is_ptr());
    let addr = msg_val.as_ptr().unwrap();
    let msg = unsafe { &*(addr as *const MbMessage) };
    assert_eq!(msg.role, "user");
    assert_eq!(msg.content, "Hello, world!");
}

#[test]
fn message_new_default_role() {
    // nargs=0 → role defaults to "user"
    let args: [MbValue; 0] = [];
    let msg_val = unsafe { mb_agent_message_new(args.as_ptr(), 0) };
    assert!(msg_val.is_ptr());
    let addr = msg_val.as_ptr().unwrap();
    let msg = unsafe { &*(addr as *const MbMessage) };
    assert_eq!(msg.role, "user", "default role should be 'user'");
}

// ── mb_agent_message_role ─────────────────────────────────────────────────────

#[test]
fn message_role_happy() {
    let msg_val = unsafe {
        mb_agent_message_new([make_str_val("assistant"), make_str_val("ok")].as_ptr(), 2)
    };
    let role_val = unsafe { mb_agent_message_role([msg_val].as_ptr(), 1) };
    let role = unsafe { read_str_val(role_val) };
    assert_eq!(role, "assistant");
}

#[test]
fn message_role_null() {
    let role_val = unsafe { mb_agent_message_role([MbValue::none()].as_ptr(), 1) };
    // Null ptr → returns empty string (not a panic)
    assert!(
        role_val.is_ptr(),
        "message_role with null ptr should return a ptr (empty string)"
    );
    let role = unsafe { read_str_val(role_val) };
    assert!(
        role.is_empty(),
        "role for null message should be empty string"
    );
}

// ── mb_agent_message_content ──────────────────────────────────────────────────

#[test]
fn message_content_happy() {
    let content_text = "The answer is 42.";
    let msg_val = unsafe {
        mb_agent_message_new(
            [make_str_val("assistant"), make_str_val(content_text)].as_ptr(),
            2,
        )
    };
    let content_val = unsafe { mb_agent_message_content([msg_val].as_ptr(), 1) };
    let content = unsafe { read_str_val(content_val) };
    assert_eq!(content, content_text);
}

#[test]
fn message_content_null() {
    let content_val = unsafe { mb_agent_message_content([MbValue::none()].as_ptr(), 1) };
    assert!(
        content_val.is_ptr(),
        "message_content with null ptr should return a ptr"
    );
    let content = unsafe { read_str_val(content_val) };
    assert!(
        content.is_empty(),
        "content for null message should be empty string"
    );
}

// ── mb_agent_tool_registry_new ────────────────────────────────────────────────

#[test]
fn tool_registry_new() {
    let args: [MbValue; 0] = [];
    let reg_val = unsafe { mb_agent_tool_registry_new(args.as_ptr(), 0) };
    assert!(reg_val.is_ptr());
    let addr = reg_val.as_ptr().unwrap();
    let registry = unsafe { &*(addr as *const MbToolRegistry) };
    assert_eq!(registry.tools.len(), 0, "new registry should have no tools");
}

// ── mb_agent_tool_registry_register ──────────────────────────────────────────

#[test]
fn tool_registry_register_happy() {
    let reg_val = unsafe { mb_agent_tool_registry_new([].as_ptr(), 0) };
    let fn_ptr = MbValue::from_func(0xDEAD);
    let args = [reg_val, make_str_val("my_tool"), fn_ptr];
    let result = unsafe { mb_agent_tool_registry_register(args.as_ptr(), 3) };
    assert!(result.is_none());

    let addr = reg_val.as_ptr().unwrap();
    let registry = unsafe { &*(addr as *const MbToolRegistry) };
    assert_eq!(registry.tools.len(), 1);
    assert_eq!(registry.tools[0].0, "my_tool");
    assert_eq!(registry.tools[0].1, 0xDEAD);
}

#[test]
fn tool_registry_register_null() {
    let fn_ptr = MbValue::from_func(0xBEEF);
    let args = [MbValue::none(), make_str_val("tool"), fn_ptr];
    let result = unsafe { mb_agent_tool_registry_register(args.as_ptr(), 3) };
    assert!(
        result.is_none(),
        "register with null registry should return none()"
    );
}

#[test]
fn tool_registry_multiple() {
    let reg_val = unsafe { mb_agent_tool_registry_new([].as_ptr(), 0) };
    for (name, ptr) in [("tool_a", 0x1usize), ("tool_b", 0x2), ("tool_c", 0x3)] {
        let fn_ptr = MbValue::from_func(ptr);
        let args = [reg_val, make_str_val(name), fn_ptr];
        unsafe { mb_agent_tool_registry_register(args.as_ptr(), 3) };
    }
    let addr = reg_val.as_ptr().unwrap();
    let registry = unsafe { &*(addr as *const MbToolRegistry) };
    assert_eq!(
        registry.tools.len(),
        3,
        "should have 3 tools after 3 registrations"
    );
    assert_eq!(registry.tools[0].0, "tool_a");
    assert_eq!(registry.tools[1].0, "tool_b");
    assert_eq!(registry.tools[2].0, "tool_c");
}

// ── Schema bindings (P3 / #1951) ──────────────────────────────────────────────

#[test]
fn schema_primitives_each_produce_distinct_handle() {
    let s_str = unsafe { mb_agent_schema_string([].as_ptr(), 0) };
    let s_int = unsafe { mb_agent_schema_integer([].as_ptr(), 0) };
    let s_num = unsafe { mb_agent_schema_number([].as_ptr(), 0) };
    let s_bool = unsafe { mb_agent_schema_boolean([].as_ptr(), 0) };
    let s_null = unsafe { mb_agent_schema_null([].as_ptr(), 0) };

    for v in [s_str, s_int, s_num, s_bool, s_null] {
        assert!(v.is_ptr(), "schema primitive must return a ptr");
    }

    let str_schema = unsafe { &*(s_str.as_ptr().unwrap() as *const MbSchema) };
    assert!(matches!(str_schema.0, agent::Schema::String));
    let int_schema = unsafe { &*(s_int.as_ptr().unwrap() as *const MbSchema) };
    assert!(matches!(int_schema.0, agent::Schema::Integer));
}

#[test]
fn schema_object_field_required_build_round_trip() {
    let builder_val = unsafe { mb_agent_schema_object([].as_ptr(), 0) };
    assert!(builder_val.is_ptr());

    // .field("name", Schema.string())
    let name_str = make_str_val("name");
    let str_schema = unsafe { mb_agent_schema_string([].as_ptr(), 0) };
    let returned =
        unsafe { mb_agent_schema_field([builder_val, name_str, str_schema].as_ptr(), 3) };
    assert_eq!(
        returned.to_bits(),
        builder_val.to_bits(),
        "field() must return the same builder ptr for chaining"
    );

    // .required("name")
    let name_str2 = make_str_val("name");
    let _ = unsafe { mb_agent_schema_required([builder_val, name_str2].as_ptr(), 2) };

    // .build()
    let schema_val = unsafe { mb_agent_schema_build([builder_val].as_ptr(), 1) };
    let schema = unsafe { &*(schema_val.as_ptr().unwrap() as *const MbSchema) };
    match &schema.0 {
        agent::Schema::Object {
            properties,
            required,
        } => {
            assert_eq!(properties.len(), 1);
            assert_eq!(properties[0].0, "name");
            assert!(matches!(properties[0].1, agent::Schema::String));
            assert_eq!(required, &vec!["name".to_string()]);
        }
        other => panic!("expected Object, got {other:?}"),
    }
}

#[test]
fn schema_required_is_variadic() {
    let builder_val = unsafe { mb_agent_schema_object([].as_ptr(), 0) };
    let n_a = make_str_val("a");
    let n_b = make_str_val("b");
    let n_c = make_str_val("c");
    let _ = unsafe { mb_agent_schema_required([builder_val, n_a, n_b, n_c].as_ptr(), 4) };
    let builder = unsafe { &*(builder_val.as_ptr().unwrap() as *const MbSchemaBuilder) };
    assert_eq!(builder.required, vec!["a", "b", "c"]);
}

#[test]
fn schema_array_wraps_inner() {
    let inner = unsafe { mb_agent_schema_integer([].as_ptr(), 0) };
    let arr_val = unsafe { mb_agent_schema_array([inner].as_ptr(), 1) };
    let arr = unsafe { &*(arr_val.as_ptr().unwrap() as *const MbSchema) };
    match &arr.0 {
        agent::Schema::Array(item) => assert!(matches!(**item, agent::Schema::Integer)),
        other => panic!("expected Array, got {other:?}"),
    }
}

#[test]
fn schema_optional_wraps_inner() {
    let inner = unsafe { mb_agent_schema_string([].as_ptr(), 0) };
    let opt_val = unsafe { mb_agent_schema_optional([inner].as_ptr(), 1) };
    let opt = unsafe { &*(opt_val.as_ptr().unwrap() as *const MbSchema) };
    match &opt.0 {
        agent::Schema::Optional(inner) => assert!(matches!(**inner, agent::Schema::String)),
        other => panic!("expected Optional, got {other:?}"),
    }
}

#[test]
fn schema_validate_ok_returns_empty_string() {
    // Build: { name: string, required: [name] }
    let b = unsafe { mb_agent_schema_object([].as_ptr(), 0) };
    let name_k = make_str_val("name");
    let str_s = unsafe { mb_agent_schema_string([].as_ptr(), 0) };
    let _ = unsafe { mb_agent_schema_field([b, name_k, str_s].as_ptr(), 3) };
    let name_k2 = make_str_val("name");
    let _ = unsafe { mb_agent_schema_required([b, name_k2].as_ptr(), 2) };
    let s = unsafe { mb_agent_schema_build([b].as_ptr(), 1) };

    let json = make_str_val(r#"{"name": "alice"}"#);
    let result = unsafe { mb_agent_schema_validate([s, json].as_ptr(), 2) };
    let msg = unsafe { read_str_val(result) };
    assert_eq!(msg, "", "valid json must return empty string, got: {msg}");
}

#[test]
fn schema_validate_err_returns_message_with_path() {
    let b = unsafe { mb_agent_schema_object([].as_ptr(), 0) };
    let age_k = make_str_val("age");
    let int_s = unsafe { mb_agent_schema_integer([].as_ptr(), 0) };
    let _ = unsafe { mb_agent_schema_field([b, age_k, int_s].as_ptr(), 3) };
    let age_k2 = make_str_val("age");
    let _ = unsafe { mb_agent_schema_required([b, age_k2].as_ptr(), 2) };
    let s = unsafe { mb_agent_schema_build([b].as_ptr(), 1) };

    let json = make_str_val(r#"{"age": "30"}"#);
    let result = unsafe { mb_agent_schema_validate([s, json].as_ptr(), 2) };
    let msg = unsafe { read_str_val(result) };
    assert!(
        msg.contains("expected integer") && msg.contains("/age"),
        "expected error message mentioning 'expected integer' and '/age', got: {msg}"
    );
}

#[test]
fn schema_validate_missing_required() {
    let b = unsafe { mb_agent_schema_object([].as_ptr(), 0) };
    let name_k = make_str_val("name");
    let str_s = unsafe { mb_agent_schema_string([].as_ptr(), 0) };
    let _ = unsafe { mb_agent_schema_field([b, name_k, str_s].as_ptr(), 3) };
    let name_k2 = make_str_val("name");
    let _ = unsafe { mb_agent_schema_required([b, name_k2].as_ptr(), 2) };
    let s = unsafe { mb_agent_schema_build([b].as_ptr(), 1) };

    let json = make_str_val(r#"{}"#);
    let result = unsafe { mb_agent_schema_validate([s, json].as_ptr(), 2) };
    let msg = unsafe { read_str_val(result) };
    assert!(
        msg.contains("missing required field 'name'"),
        "expected missing-required error, got: {msg}"
    );
}

#[test]
fn schema_validate_invalid_json_payload() {
    let s = unsafe { mb_agent_schema_string([].as_ptr(), 0) };
    let json = make_str_val("not json {");
    let result = unsafe { mb_agent_schema_validate([s, json].as_ptr(), 2) };
    let msg = unsafe { read_str_val(result) };
    assert!(
        msg.starts_with("invalid JSON:"),
        "expected invalid-json error, got: {msg}"
    );
}
