//! Mamba binding for `agentkit`.
//!
//! Exposes LLM agent capabilities to Mamba scripts via the
//! `mamba-registry` infrastructure.
//!
//! # Module name
//!
//! Import in Mamba as `mambalibs.agent`:
//! ```python
//! from mambalibs.agent import AgentBuilder, ClaudeProvider, Message, ToolRegistry
//! ```
//! `cclab.agent` remains registered as a compatibility alias.

pub mod methods;
pub mod types;

use mamba_registry::{rt_sym, MambaModule, ModuleRegistrar, MAMBA_MODULES};
use linkme::distributed_slice;

// ── Agent module registration ─────────────────────────────────────────────────

fn register_agent_surface(r: &mut ModuleRegistrar) {
    use crate::methods::{
        mb_agent_builder_build, mb_agent_builder_new, mb_agent_builder_provider,
        mb_agent_builder_system_prompt, mb_agent_claude_provider, mb_agent_gemini_provider,
        mb_agent_message_content, mb_agent_message_new, mb_agent_message_role,
        mb_agent_openai_provider, mb_agent_run, mb_agent_schema_array, mb_agent_schema_boolean,
        mb_agent_schema_build, mb_agent_schema_field, mb_agent_schema_integer,
        mb_agent_schema_null, mb_agent_schema_number, mb_agent_schema_object,
        mb_agent_schema_optional, mb_agent_schema_required, mb_agent_schema_string,
        mb_agent_schema_validate, mb_agent_team_add_role, mb_agent_team_new, mb_agent_team_run,
        mb_agent_tool_registry_new, mb_agent_tool_registry_register,
    };

    r.add_symbols([
        rt_sym!(
            "AgentBuilder",
            mb_agent_builder_new,
            "AgentBuilder() -> builder"
        ),
        rt_sym!(
            "builder_provider",
            mb_agent_builder_provider,
            "builder_provider(builder, provider) -> None"
        ),
        rt_sym!(
            "builder_system_prompt",
            mb_agent_builder_system_prompt,
            "builder_system_prompt(builder, prompt: str) -> None"
        ),
        rt_sym!(
            "builder_build",
            mb_agent_builder_build,
            "builder_build(builder) -> agent"
        ),
        rt_sym!("run", mb_agent_run, "run(agent, prompt: str) -> str"),
        rt_sym!("AgentTeam", mb_agent_team_new, "AgentTeam() -> team"),
        rt_sym!(
            "team_add_role",
            mb_agent_team_add_role,
            "team_add_role(team, role_name: str) -> None"
        ),
        rt_sym!(
            "team_run",
            mb_agent_team_run,
            "team_run(team, prompt: str) -> str (JSON artifact)"
        ),
        rt_sym!(
            "ClaudeProvider",
            mb_agent_claude_provider,
            "ClaudeProvider(api_key: str) -> provider"
        ),
        rt_sym!(
            "GeminiProvider",
            mb_agent_gemini_provider,
            "GeminiProvider(api_key: str) -> provider"
        ),
        rt_sym!(
            "OpenAIProvider",
            mb_agent_openai_provider,
            "OpenAIProvider(api_key: str) -> provider"
        ),
        rt_sym!(
            "Message",
            mb_agent_message_new,
            "Message(role: str, content: str) -> message"
        ),
        rt_sym!(
            "message_role",
            mb_agent_message_role,
            "message_role(message) -> str"
        ),
        rt_sym!(
            "message_content",
            mb_agent_message_content,
            "message_content(message) -> str"
        ),
        rt_sym!(
            "ToolRegistry",
            mb_agent_tool_registry_new,
            "ToolRegistry() -> registry"
        ),
        rt_sym!(
            "tool_registry_register",
            mb_agent_tool_registry_register,
            "tool_registry_register(registry, name: str, func) -> None"
        ),
        // ── Schema builder (P3 / #1951) ────────────────────────────
        rt_sym!(
            "schema_object",
            mb_agent_schema_object,
            "schema_object() -> builder"
        ),
        rt_sym!(
            "schema_string",
            mb_agent_schema_string,
            "schema_string() -> schema"
        ),
        rt_sym!(
            "schema_integer",
            mb_agent_schema_integer,
            "schema_integer() -> schema"
        ),
        rt_sym!(
            "schema_number",
            mb_agent_schema_number,
            "schema_number() -> schema"
        ),
        rt_sym!(
            "schema_boolean",
            mb_agent_schema_boolean,
            "schema_boolean() -> schema"
        ),
        rt_sym!(
            "schema_null",
            mb_agent_schema_null,
            "schema_null() -> schema"
        ),
        rt_sym!(
            "schema_array",
            mb_agent_schema_array,
            "schema_array(item) -> schema"
        ),
        rt_sym!(
            "schema_optional",
            mb_agent_schema_optional,
            "schema_optional(inner) -> schema"
        ),
        rt_sym!(
            "schema_field",
            mb_agent_schema_field,
            "schema_field(builder, name: str, sub_schema) -> builder"
        ),
        rt_sym!(
            "schema_required",
            mb_agent_schema_required,
            "schema_required(builder, *names: str) -> builder"
        ),
        rt_sym!(
            "schema_build",
            mb_agent_schema_build,
            "schema_build(builder) -> schema"
        ),
        rt_sym!(
            "schema_validate",
            mb_agent_schema_validate,
            "schema_validate(schema, json: str) -> str (empty if ok, else error message)"
        ),
    ]);
}

/// The primary `mambalibs.agent` native module descriptor.
pub struct AgentMambaModule;

impl MambaModule for AgentMambaModule {
    fn name(&self) -> &'static str {
        "mambalibs.agent"
    }

    fn doc(&self) -> &'static str {
        "Mamba-native agent interface"
    }

    fn register(&self, r: &mut ModuleRegistrar) {
        register_agent_surface(r);
    }
}

/// Legacy import alias kept for callers that still use `cclab.agent`.
pub struct CclabAgentCompatModule;

impl MambaModule for CclabAgentCompatModule {
    fn name(&self) -> &'static str {
        "cclab.agent"
    }

    fn doc(&self) -> &'static str {
        "Compatibility alias for mambalibs.agent"
    }

    fn register(&self, r: &mut ModuleRegistrar) {
        register_agent_surface(r);
    }
}

// ── Auto-registration ─────────────────────────────────────────────────────────

#[distributed_slice(MAMBA_MODULES)]
static AGENT_MAMBA_MODULE: &dyn MambaModule = &AgentMambaModule;

#[distributed_slice(MAMBA_MODULES)]
static CCLAB_AGENT_COMPAT_MODULE: &dyn MambaModule = &CclabAgentCompatModule;
