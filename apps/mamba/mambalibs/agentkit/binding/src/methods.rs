// MbValue is a newtype around u64; the JIT passes it by value as a 64-bit word.
#![allow(improper_ctypes_definitions)]

//! FFI functions exposed by `agent-mamba` to Mamba scripts.
//!
//! All functions follow the Mamba native-call ABI:
//! ```text
//! extern "C" fn name(args: *const MbValue, nargs: usize) -> MbValue
//! ```
//!
//! # Exposed API
//!
//! | Symbol                            | Mamba call                                    |
//! |-----------------------------------|-----------------------------------------------|
//! | `mb_agent_builder_new`            | `AgentBuilder() -> builder`                   |
//! | `mb_agent_builder_provider`       | `builder.provider(provider) -> None`          |
//! | `mb_agent_builder_system_prompt`  | `builder.system_prompt(prompt) -> None`       |
//! | `mb_agent_builder_build`          | `builder.build() -> agent`                    |
//! | `mb_agent_run`                    | `agent.run(prompt) -> str`                    |
//! | `mb_agent_team_new`               | `AgentTeam() -> team`                         |
//! | `mb_agent_team_add_role`          | `team.add_role(role_name) -> None`            |
//! | `mb_agent_team_run`               | `team.run(prompt) -> str` (JSON artifact)     |
//! | `mb_agent_claude_provider`        | `claude_provider(api_key) -> provider`        |
//! | `mb_agent_gemini_provider`        | `gemini_provider(api_key) -> provider`        |
//! | `mb_agent_openai_provider`        | `openai_provider(api_key) -> provider`        |
//! | `mb_agent_message_new`            | `Message(role, content) -> message`           |
//! | `mb_agent_message_role`           | `message.role -> str`                         |
//! | `mb_agent_message_content`        | `message.content -> str`                      |
//! | `mb_agent_tool_registry_new`      | `ToolRegistry() -> registry`                  |
//! | `mb_agent_tool_registry_register` | `registry.register(name, func) -> None`       |

use mamba_registry::convert::mb_wrap_native;
use mamba_registry::MbValue;

use crate::types::{
    MbAgentBuilder, MbAgentTeam, MbLlmAgent, MbMessage, MbProvider, MbSchema, MbSchemaBuilder,
    MbToolRegistry,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

#[inline]
unsafe fn arg(args: *const MbValue, nargs: usize, idx: usize) -> MbValue {
    if idx < nargs {
        unsafe { *args.add(idx) }
    } else {
        MbValue::none()
    }
}

fn read_str(v: MbValue) -> Option<String> {
    mamba_registry::test_ops::init();
    unsafe { mamba_registry::rc::read_obj_str(v) }
}

fn wrap_str(s: String) -> MbValue {
    mamba_registry::test_ops::init();
    mamba_registry::rc::wrap_obj_str(s)
}

// ── mb_agent_builder_new ──────────────────────────────────────────────────────

/// Create a new agent builder with empty defaults.
///
/// # ABI
/// ```text
/// (no args)
/// ```
/// Returns an opaque PTR to [`MbAgentBuilder`].
#[no_mangle]
pub unsafe extern "C" fn mb_agent_builder_new(_args: *const MbValue, _nargs: usize) -> MbValue {
    mb_wrap_native(MbAgentBuilder::new())
}

// ── mb_agent_builder_provider ─────────────────────────────────────────────────

/// Set the provider on an agent builder.
///
/// # ABI
/// ```text
/// args[0] = builder   (MbValue::Ptr → MbAgentBuilder)
/// args[1] = provider  (MbValue::Ptr → MbProvider)
/// ```
/// Returns `MbValue::none()`.
#[no_mangle]
pub unsafe extern "C" fn mb_agent_builder_provider(args: *const MbValue, nargs: usize) -> MbValue {
    let builder_val = unsafe { arg(args, nargs, 0) };
    let provider_val = unsafe { arg(args, nargs, 1) };

    let builder_addr = match builder_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => return MbValue::none(),
    };
    let builder = unsafe { &mut *(builder_addr as *mut MbAgentBuilder) };

    if let Some(provider_addr) = provider_val.as_ptr() {
        if provider_addr != 0 {
            let provider = unsafe { &*(provider_addr as *const MbProvider) };
            builder.provider_name = provider.name.clone();
            builder.api_key = provider.api_key.clone();
        }
    }

    MbValue::none()
}

// ── mb_agent_builder_system_prompt ────────────────────────────────────────────

/// Set the system prompt on an agent builder.
///
/// # ABI
/// ```text
/// args[0] = builder  (MbValue::Ptr → MbAgentBuilder)
/// args[1] = prompt   (MbValue::Ptr → heap String)
/// ```
/// Returns `MbValue::none()`.
#[no_mangle]
pub unsafe extern "C" fn mb_agent_builder_system_prompt(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let builder_val = unsafe { arg(args, nargs, 0) };
    let prompt_val = unsafe { arg(args, nargs, 1) };

    let addr = match builder_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => return MbValue::none(),
    };
    let builder = unsafe { &mut *(addr as *mut MbAgentBuilder) };
    if let Some(prompt) = read_str(prompt_val) {
        builder.system_prompt = prompt;
    }
    MbValue::none()
}

// ── mb_agent_builder_build ────────────────────────────────────────────────────

/// Build an [`MbLlmAgent`] from an [`MbAgentBuilder`].
///
/// # ABI
/// ```text
/// args[0] = builder  (MbValue::Ptr → MbAgentBuilder)
/// ```
/// Returns an opaque PTR to [`MbLlmAgent`].
#[no_mangle]
pub unsafe extern "C" fn mb_agent_builder_build(args: *const MbValue, nargs: usize) -> MbValue {
    let builder_val = unsafe { arg(args, nargs, 0) };

    let addr = match builder_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => {
            return mb_wrap_native(MbLlmAgent {
                provider_name: String::new(),
                api_key: String::new(),
                system_prompt: String::new(),
            })
        }
    };
    let builder = unsafe { &*(addr as *const MbAgentBuilder) };
    mb_wrap_native(MbLlmAgent {
        provider_name: builder.provider_name.clone(),
        api_key: builder.api_key.clone(),
        system_prompt: builder.system_prompt.clone(),
    })
}

// ── mb_agent_run ──────────────────────────────────────────────────────────────

/// Run a prompt through the LLM agent and return the response text.
///
/// For the Claude provider with a valid API key, this dispatches to
/// `agent::ClaudeProvider`. All other providers (or missing API keys)
/// return a stub response so the binding compiles and tests pass without
/// live API credentials.
///
/// Full multi-provider dispatch is wired once provider constructors stabilise.
///
/// # ABI
/// ```text
/// args[0] = agent   (MbValue::Ptr → MbLlmAgent)
/// args[1] = prompt  (MbValue::Ptr → heap String)
/// ```
/// Returns `MbValue::Ptr → heap String` (response text).
#[no_mangle]
pub unsafe extern "C" fn mb_agent_run(args: *const MbValue, nargs: usize) -> MbValue {
    let agent_val = unsafe { arg(args, nargs, 0) };
    let prompt_val = unsafe { arg(args, nargs, 1) };

    let agent = match agent_val.as_ptr() {
        Some(a) if a != 0 => unsafe { &*(a as *const MbLlmAgent) },
        _ => return wrap_str("[error: invalid agent handle]".to_string()),
    };
    let prompt = read_str(prompt_val).unwrap_or_default();

    // Stub: returns a placeholder response.
    // Full LLM dispatch (agent::ClaudeProvider / GeminiProvider / OpenAIProvider)
    // is wired here once the completion API is stabilised.
    let response = format!(
        "[stub response from {} agent for: {}]",
        agent.provider_name, prompt
    );
    wrap_str(response)
}

// ── mb_agent_team_new ─────────────────────────────────────────────────────────

/// Create a new governed agent team (#1545). Empty roles list by default.
///
/// # ABI
/// ```text
/// (no args)
/// ```
/// Returns an opaque PTR to [`MbAgentTeam`].
#[no_mangle]
pub unsafe extern "C" fn mb_agent_team_new(_args: *const MbValue, _nargs: usize) -> MbValue {
    mb_wrap_native(MbAgentTeam::new())
}

// ── mb_agent_team_add_role ────────────────────────────────────────────────────

/// Append a role to a governed agent team (#1545). Role name should match
/// the serde-lowercase form of `cclab_agent::types::AgentRole`
/// (`pm | designer | dev | data | qa | release`); validation is deferred
/// to `team_run`.
///
/// # ABI
/// ```text
/// args[0] = team       (MbValue::Ptr → MbAgentTeam)
/// args[1] = role_name  (MbValue::Ptr → heap String)
/// ```
/// Returns `MbValue::none()`.
#[no_mangle]
pub unsafe extern "C" fn mb_agent_team_add_role(args: *const MbValue, nargs: usize) -> MbValue {
    let team_val = unsafe { arg(args, nargs, 0) };
    let role_val = unsafe { arg(args, nargs, 1) };

    let addr = match team_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => return MbValue::none(),
    };
    let team = unsafe { &mut *(addr as *mut MbAgentTeam) };
    if let Some(role) = read_str(role_val) {
        team.add_role(role);
    }
    MbValue::none()
}

// ── mb_agent_team_run ─────────────────────────────────────────────────────────

/// Run a prompt through the governed agent team (#1545). Returns a
/// JSON-encoded artifact placeholder matching the `agent-team-artifact.v0`
/// schema (six top-level keys). T4 stub — full multi-role dispatch lands
/// in a later sub-task.
///
/// # ABI
/// ```text
/// args[0] = team    (MbValue::Ptr → MbAgentTeam)
/// args[1] = prompt  (MbValue::Ptr → heap String)
/// ```
/// Returns `MbValue::Ptr → heap String` (JSON-encoded artifact).
#[no_mangle]
pub unsafe extern "C" fn mb_agent_team_run(args: *const MbValue, nargs: usize) -> MbValue {
    let team_val = unsafe { arg(args, nargs, 0) };
    let prompt_val = unsafe { arg(args, nargs, 1) };

    let roles_json = match team_val.as_ptr() {
        Some(a) if a != 0 => {
            let team = unsafe { &*(a as *const MbAgentTeam) };
            serde_json::to_string(&team.roles).unwrap_or_else(|_| "[]".to_string())
        }
        _ => "[]".to_string(),
    };
    let prompt = read_str(prompt_val).unwrap_or_default();

    let artifact = serde_json::json!({
        "status": "NotImplemented",
        "issue": "#1545 T4",
        "roles": serde_json::from_str::<serde_json::Value>(&roles_json)
            .unwrap_or(serde_json::Value::Array(vec![])),
        "prompt": prompt,
        "requirements_summary": "",
        "app_spec_changes": [],
        "implementation": "",
        "tests": [],
        "release_package": {},
        "review_tickets": [],
    });
    wrap_str(artifact.to_string())
}

// ── mb_agent_claude_provider ──────────────────────────────────────────────────

/// Create a Claude provider handle.
///
/// # ABI
/// ```text
/// args[0] = api_key  (MbValue::Ptr → heap String)
/// ```
/// Returns an opaque PTR to [`MbProvider`].
#[no_mangle]
pub unsafe extern "C" fn mb_agent_claude_provider(args: *const MbValue, nargs: usize) -> MbValue {
    let key_val = unsafe { arg(args, nargs, 0) };
    let api_key = read_str(key_val).unwrap_or_default();
    mb_wrap_native(MbProvider::new("claude", api_key))
}

// ── mb_agent_gemini_provider ──────────────────────────────────────────────────

/// Create a Gemini provider handle.
///
/// # ABI
/// ```text
/// args[0] = api_key  (MbValue::Ptr → heap String)
/// ```
/// Returns an opaque PTR to [`MbProvider`].
#[no_mangle]
pub unsafe extern "C" fn mb_agent_gemini_provider(args: *const MbValue, nargs: usize) -> MbValue {
    let key_val = unsafe { arg(args, nargs, 0) };
    let api_key = read_str(key_val).unwrap_or_default();
    mb_wrap_native(MbProvider::new("gemini", api_key))
}

// ── mb_agent_openai_provider ──────────────────────────────────────────────────

/// Create an OpenAI provider handle.
///
/// # ABI
/// ```text
/// args[0] = api_key  (MbValue::Ptr → heap String)
/// ```
/// Returns an opaque PTR to [`MbProvider`].
#[no_mangle]
pub unsafe extern "C" fn mb_agent_openai_provider(args: *const MbValue, nargs: usize) -> MbValue {
    let key_val = unsafe { arg(args, nargs, 0) };
    let api_key = read_str(key_val).unwrap_or_default();
    mb_wrap_native(MbProvider::new("openai", api_key))
}

// ── mb_agent_message_new ──────────────────────────────────────────────────────

/// Create a new chat message.
///
/// # ABI
/// ```text
/// args[0] = role     (MbValue::Ptr → heap String)
/// args[1] = content  (MbValue::Ptr → heap String)
/// ```
/// Returns an opaque PTR to [`MbMessage`].
#[no_mangle]
pub unsafe extern "C" fn mb_agent_message_new(args: *const MbValue, nargs: usize) -> MbValue {
    let role_val = unsafe { arg(args, nargs, 0) };
    let content_val = unsafe { arg(args, nargs, 1) };

    let role = read_str(role_val).unwrap_or_else(|| "user".to_string());
    let content = read_str(content_val).unwrap_or_default();
    mb_wrap_native(MbMessage::new(role, content))
}

// ── mb_agent_message_role ─────────────────────────────────────────────────────

/// Get the role of a message.
///
/// # ABI
/// ```text
/// args[0] = message  (MbValue::Ptr → MbMessage)
/// ```
/// Returns `MbValue::Ptr → heap String`.
#[no_mangle]
pub unsafe extern "C" fn mb_agent_message_role(args: *const MbValue, nargs: usize) -> MbValue {
    let msg_val = unsafe { arg(args, nargs, 0) };

    let addr = match msg_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => return wrap_str(String::new()),
    };
    let msg = unsafe { &*(addr as *const MbMessage) };
    wrap_str(msg.role.clone())
}

// ── mb_agent_message_content ──────────────────────────────────────────────────

/// Get the content of a message.
///
/// # ABI
/// ```text
/// args[0] = message  (MbValue::Ptr → MbMessage)
/// ```
/// Returns `MbValue::Ptr → heap String`.
#[no_mangle]
pub unsafe extern "C" fn mb_agent_message_content(args: *const MbValue, nargs: usize) -> MbValue {
    let msg_val = unsafe { arg(args, nargs, 0) };

    let addr = match msg_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => return wrap_str(String::new()),
    };
    let msg = unsafe { &*(addr as *const MbMessage) };
    wrap_str(msg.content.clone())
}

// ── mb_agent_tool_registry_new ────────────────────────────────────────────────

/// Create a new empty tool registry.
///
/// # ABI
/// ```text
/// (no args)
/// ```
/// Returns an opaque PTR to [`MbToolRegistry`].
#[no_mangle]
pub unsafe extern "C" fn mb_agent_tool_registry_new(
    _args: *const MbValue,
    _nargs: usize,
) -> MbValue {
    mb_wrap_native(MbToolRegistry::new())
}

// ── mb_agent_tool_registry_register ──────────────────────────────────────────

/// Register a named tool (Mamba function pointer) in the registry.
///
/// # ABI
/// ```text
/// args[0] = registry  (MbValue::Ptr → MbToolRegistry)
/// args[1] = name      (MbValue::Ptr → heap String)
/// args[2] = func      (MbValue::Func — Mamba function pointer)
/// ```
/// Returns `MbValue::none()`.
#[no_mangle]
pub unsafe extern "C" fn mb_agent_tool_registry_register(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let registry_val = unsafe { arg(args, nargs, 0) };
    let name_val = unsafe { arg(args, nargs, 1) };
    let func_val = unsafe { arg(args, nargs, 2) };

    let addr = match registry_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => return MbValue::none(),
    };
    let registry = unsafe { &mut *(addr as *mut MbToolRegistry) };
    let name = read_str(name_val).unwrap_or_default();
    let func_ptr = func_val.as_func().unwrap_or(0);
    registry.register(name, func_ptr);
    MbValue::none()
}

// ── Schema bindings (P3 / #1951) ──────────────────────────────────────────────
//
// HANDWRITE-BEGIN reason: codegen has no generator for mamba FFI thunks
// that wrap a Rust builder API onto the (*const MbValue, usize) -> MbValue
// ABI. Closes once the typed-I/O track's spec + generator land.

/// Build a fresh object-schema builder. Used by the mamba surface as the
/// entry point for `Schema.object()`. Returns an opaque PTR.
#[no_mangle]
pub unsafe extern "C" fn mb_agent_schema_object(_args: *const MbValue, _nargs: usize) -> MbValue {
    mb_wrap_native(MbSchemaBuilder::new())
}

/// `Schema.string()` → primitive schema handle.
#[no_mangle]
pub unsafe extern "C" fn mb_agent_schema_string(_args: *const MbValue, _nargs: usize) -> MbValue {
    mb_wrap_native(MbSchema(agent::Schema::String))
}

/// `Schema.integer()`.
#[no_mangle]
pub unsafe extern "C" fn mb_agent_schema_integer(_args: *const MbValue, _nargs: usize) -> MbValue {
    mb_wrap_native(MbSchema(agent::Schema::Integer))
}

/// `Schema.number()`.
#[no_mangle]
pub unsafe extern "C" fn mb_agent_schema_number(_args: *const MbValue, _nargs: usize) -> MbValue {
    mb_wrap_native(MbSchema(agent::Schema::Number))
}

/// `Schema.boolean()`.
#[no_mangle]
pub unsafe extern "C" fn mb_agent_schema_boolean(_args: *const MbValue, _nargs: usize) -> MbValue {
    mb_wrap_native(MbSchema(agent::Schema::Boolean))
}

/// `Schema.null()`.
#[no_mangle]
pub unsafe extern "C" fn mb_agent_schema_null(_args: *const MbValue, _nargs: usize) -> MbValue {
    mb_wrap_native(MbSchema(agent::Schema::Null))
}

/// `Schema.array(item)` — wraps a previously-built schema as an array.
///
/// # ABI
/// ```text
/// args[0] = item  (MbValue::Ptr → MbSchema)
/// ```
#[no_mangle]
pub unsafe extern "C" fn mb_agent_schema_array(args: *const MbValue, nargs: usize) -> MbValue {
    let item_val = unsafe { arg(args, nargs, 0) };
    let addr = match item_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => {
            return mb_wrap_native(MbSchema(agent::Schema::Array(Box::new(
                agent::Schema::Null,
            ))))
        }
    };
    let item = unsafe { &*(addr as *const MbSchema) };
    mb_wrap_native(MbSchema(agent::Schema::Array(Box::new(item.0.clone()))))
}

/// `Schema.optional(inner)` — nullable wrapper.
///
/// # ABI
/// ```text
/// args[0] = inner  (MbValue::Ptr → MbSchema)
/// ```
#[no_mangle]
pub unsafe extern "C" fn mb_agent_schema_optional(args: *const MbValue, nargs: usize) -> MbValue {
    let inner_val = unsafe { arg(args, nargs, 0) };
    let addr = match inner_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => {
            return mb_wrap_native(MbSchema(agent::Schema::Optional(Box::new(
                agent::Schema::Null,
            ))))
        }
    };
    let inner = unsafe { &*(addr as *const MbSchema) };
    mb_wrap_native(MbSchema(agent::Schema::Optional(Box::new(inner.0.clone()))))
}

/// `builder.field(name, sub_schema)` — append a field to an in-progress
/// object schema. Mutates the builder in place; returns the same builder
/// PTR so callers can chain via `let b = builder.field(...)` in mamba.
///
/// # ABI
/// ```text
/// args[0] = builder    (MbValue::Ptr → MbSchemaBuilder)
/// args[1] = name       (MbValue::Ptr → heap String)
/// args[2] = sub_schema (MbValue::Ptr → MbSchema)
/// ```
#[no_mangle]
pub unsafe extern "C" fn mb_agent_schema_field(args: *const MbValue, nargs: usize) -> MbValue {
    let builder_val = unsafe { arg(args, nargs, 0) };
    let name_val = unsafe { arg(args, nargs, 1) };
    let sub_val = unsafe { arg(args, nargs, 2) };

    let builder_addr = match builder_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => return MbValue::none(),
    };
    let builder = unsafe { &mut *(builder_addr as *mut MbSchemaBuilder) };
    let name = read_str(name_val).unwrap_or_default();
    if let Some(sub_addr) = sub_val.as_ptr() {
        if sub_addr != 0 {
            let sub = unsafe { &*(sub_addr as *const MbSchema) };
            builder.field(name, sub.0.clone());
        }
    }
    builder_val
}

/// `builder.required(name1, name2, ...)` — mark fields required.
///
/// Mamba's MbValue ABI has no list type, so this binding is variadic:
/// every arg after `args[0]` (the builder) is read as a string name.
///
/// # ABI
/// ```text
/// args[0]       = builder (MbValue::Ptr → MbSchemaBuilder)
/// args[1..]     = name_i  (MbValue::Ptr → heap String)
/// ```
#[no_mangle]
pub unsafe extern "C" fn mb_agent_schema_required(args: *const MbValue, nargs: usize) -> MbValue {
    let builder_val = unsafe { arg(args, nargs, 0) };
    let builder_addr = match builder_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => return MbValue::none(),
    };
    let builder = unsafe { &mut *(builder_addr as *mut MbSchemaBuilder) };
    for i in 1..nargs {
        let name_val = unsafe { arg(args, nargs, i) };
        if let Some(name) = read_str(name_val) {
            builder.required(name);
        }
    }
    builder_val
}

/// `builder.build()` → finalize an object schema builder into a
/// [`MbSchema`].
///
/// # ABI
/// ```text
/// args[0] = builder  (MbValue::Ptr → MbSchemaBuilder)
/// ```
#[no_mangle]
pub unsafe extern "C" fn mb_agent_schema_build(args: *const MbValue, nargs: usize) -> MbValue {
    let builder_val = unsafe { arg(args, nargs, 0) };
    let addr = match builder_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => {
            return mb_wrap_native(MbSchema(agent::Schema::Object {
                properties: Vec::new(),
                required: Vec::new(),
            }))
        }
    };
    let builder = unsafe { &*(addr as *const MbSchemaBuilder) };
    mb_wrap_native(builder.clone().into_object())
}

/// `schema.validate(json_str)` → empty string on success, otherwise the
/// validation error message.
///
/// # ABI
/// ```text
/// args[0] = schema    (MbValue::Ptr → MbSchema)
/// args[1] = json_str  (MbValue::Ptr → heap String)
/// ```
/// Returns `MbValue::Ptr → heap String`. Empty string ↔ valid.
#[no_mangle]
pub unsafe extern "C" fn mb_agent_schema_validate(args: *const MbValue, nargs: usize) -> MbValue {
    let schema_val = unsafe { arg(args, nargs, 0) };
    let json_val = unsafe { arg(args, nargs, 1) };

    let schema = match schema_val.as_ptr() {
        Some(a) if a != 0 => unsafe { &*(a as *const MbSchema) },
        _ => return wrap_str("[error: invalid schema handle]".to_string()),
    };
    let json_text = read_str(json_val).unwrap_or_default();
    let parsed: serde_json::Value = match serde_json::from_str(&json_text) {
        Ok(v) => v,
        Err(e) => return wrap_str(format!("invalid JSON: {e}")),
    };
    match schema.0.validate(&parsed) {
        Ok(()) => wrap_str(String::new()),
        Err(err) => wrap_str(err.to_string()),
    }
}

// HANDWRITE-END

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_str_val(s: &str) -> MbValue {
        mamba_registry::test_ops::init();
        mamba_registry::rc::wrap_obj_str(s.to_string())
    }

    #[test]
    fn test_builder_new() {
        let args: [MbValue; 0] = [];
        let builder_val = unsafe { mb_agent_builder_new(args.as_ptr(), 0) };
        assert!(builder_val.is_ptr(), "builder should be a ptr");

        let addr = builder_val.as_ptr().unwrap();
        let builder = unsafe { &*(addr as *const MbAgentBuilder) };
        assert!(builder.provider_name.is_empty());
        assert!(builder.api_key.is_empty());
    }

    #[test]
    fn test_claude_provider() {
        let key_val = make_str_val("sk-ant-test-key");
        let args = [key_val];
        let provider_val = unsafe { mb_agent_claude_provider(args.as_ptr(), 1) };
        assert!(provider_val.is_ptr());

        let addr = provider_val.as_ptr().unwrap();
        let provider = unsafe { &*(addr as *const MbProvider) };
        assert_eq!(provider.name, "claude");
        assert_eq!(provider.api_key, "sk-ant-test-key");
    }

    #[test]
    fn test_gemini_provider() {
        let key_val = make_str_val("gemini-key");
        let args = [key_val];
        let provider_val = unsafe { mb_agent_gemini_provider(args.as_ptr(), 1) };
        let addr = provider_val.as_ptr().unwrap();
        let provider = unsafe { &*(addr as *const MbProvider) };
        assert_eq!(provider.name, "gemini");
    }

    #[test]
    fn test_openai_provider() {
        let key_val = make_str_val("openai-key");
        let args = [key_val];
        let provider_val = unsafe { mb_agent_openai_provider(args.as_ptr(), 1) };
        let addr = provider_val.as_ptr().unwrap();
        let provider = unsafe { &*(addr as *const MbProvider) };
        assert_eq!(provider.name, "openai");
    }

    #[test]
    fn test_message_new() {
        let role_val = make_str_val("user");
        let content_val = make_str_val("Hello, world!");
        let args = [role_val, content_val];
        let msg_val = unsafe { mb_agent_message_new(args.as_ptr(), 2) };
        assert!(msg_val.is_ptr());

        let addr = msg_val.as_ptr().unwrap();
        let msg = unsafe { &*(addr as *const MbMessage) };
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello, world!");
    }

    #[test]
    fn test_message_role_and_content() {
        let role_val = make_str_val("assistant");
        let content_val = make_str_val("I can help!");
        let args = [role_val, content_val];
        let msg_val = unsafe { mb_agent_message_new(args.as_ptr(), 2) };

        let role_result = unsafe { mb_agent_message_role([msg_val].as_ptr(), 1) };
        let content_result = unsafe { mb_agent_message_content([msg_val].as_ptr(), 1) };

        let role_str = unsafe { role_result.as_obj_str() }.unwrap();
        assert_eq!(role_str, "assistant");

        let content_str = unsafe { content_result.as_obj_str() }.unwrap();
        assert_eq!(content_str, "I can help!");
    }

    #[test]
    fn test_tool_registry() {
        let args: [MbValue; 0] = [];
        let registry_val = unsafe { mb_agent_tool_registry_new(args.as_ptr(), 0) };
        assert!(registry_val.is_ptr());

        let name_val = make_str_val("my_tool");
        let func_val = MbValue::from_func(0xBEEF);
        let reg_args = [registry_val, name_val, func_val];
        let result = unsafe { mb_agent_tool_registry_register(reg_args.as_ptr(), 3) };
        assert!(result.is_none());

        let addr = registry_val.as_ptr().unwrap();
        let registry = unsafe { &*(addr as *const MbToolRegistry) };
        assert_eq!(registry.tools.len(), 1);
        assert_eq!(registry.tools[0].0, "my_tool");
        assert_eq!(registry.tools[0].1, 0xBEEF);
    }

    #[test]
    fn test_builder_with_provider_and_build() {
        let args: [MbValue; 0] = [];
        let builder_val = unsafe { mb_agent_builder_new(args.as_ptr(), 0) };

        let key_val = make_str_val("my-api-key");
        let provider_args = [key_val];
        let provider_val = unsafe { mb_agent_claude_provider(provider_args.as_ptr(), 1) };

        let set_args = [builder_val, provider_val];
        unsafe { mb_agent_builder_provider(set_args.as_ptr(), 2) };

        let prompt_val = make_str_val("You are a helpful assistant.");
        let sys_args = [builder_val, prompt_val];
        unsafe { mb_agent_builder_system_prompt(sys_args.as_ptr(), 2) };

        let build_args = [builder_val];
        let agent_val = unsafe { mb_agent_builder_build(build_args.as_ptr(), 1) };
        assert!(agent_val.is_ptr());

        let addr = agent_val.as_ptr().unwrap();
        let agent = unsafe { &*(addr as *const MbLlmAgent) };
        assert_eq!(agent.provider_name, "claude");
        assert_eq!(agent.api_key, "my-api-key");
        assert_eq!(agent.system_prompt, "You are a helpful assistant.");
    }

    #[test]
    fn test_agent_run_stub() {
        let args: [MbValue; 0] = [];
        let builder_val = unsafe { mb_agent_builder_new(args.as_ptr(), 0) };

        let key_val = make_str_val("");
        let provider_args = [key_val];
        let provider_val = unsafe { mb_agent_gemini_provider(provider_args.as_ptr(), 1) };

        let set_args = [builder_val, provider_val];
        unsafe { mb_agent_builder_provider(set_args.as_ptr(), 2) };

        let build_args = [builder_val];
        let agent_val = unsafe { mb_agent_builder_build(build_args.as_ptr(), 1) };

        let prompt_val = make_str_val("What is 2+2?");
        let run_args = [agent_val, prompt_val];
        let response_val = unsafe { mb_agent_run(run_args.as_ptr(), 2) };
        assert!(response_val.is_ptr());

        let response = unsafe { response_val.as_obj_str() }.unwrap();
        assert!(
            response.contains("stub"),
            "should return a stub response: {response}"
        );
    }
}
