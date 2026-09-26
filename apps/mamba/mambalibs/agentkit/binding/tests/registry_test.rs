use agentkit_binding as _;
use mamba_registry::{find_module, test_ops, ModuleRegistrar};
use std::collections::HashSet;

fn registered_module_by_name(name: &str) -> ModuleRegistrar {
    test_ops::init();
    let module = find_module(name).unwrap_or_else(|| panic!("agentkit must register {name}"));
    let mut registrar = ModuleRegistrar::new();
    module.register(&mut registrar);
    registrar
}

#[test]
fn mambalibs_agent_is_primary_runtime_namespace() {
    let registrar = registered_module_by_name("mambalibs.agent");
    let symbols: HashSet<&str> = registrar.symbols().iter().map(|sym| sym.name).collect();
    assert!(symbols.contains("AgentBuilder"));
    assert!(symbols.contains("ClaudeProvider"));
    assert!(symbols.contains("ToolRegistry"));
}

#[test]
fn cclab_agent_remains_compat_alias() {
    let registrar = registered_module_by_name("cclab.agent");
    let symbols: HashSet<&str> = registrar.symbols().iter().map(|sym| sym.name).collect();
    assert!(symbols.contains("AgentBuilder"));
    assert!(symbols.contains("ClaudeProvider"));
    assert!(symbols.contains("ToolRegistry"));
}
