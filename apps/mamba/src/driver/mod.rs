pub mod config;
pub mod module_graph;
pub mod repl;

pub use crate::pkgmanage::manifest::MambaConfig;
pub use config::{Backend, CompilerConfig, EmitMode, OptLevel};

use crate::codegen::cranelift::jit::CraneliftJitBackend;
use crate::codegen::cranelift::CraneliftBackend;
use crate::codegen::llvm::LlvmBackend;
use crate::codegen::{CodegenBackend, CodegenOutput};
use crate::diagnostic;
use crate::error::MambaError;
use crate::lower;
use crate::parser;
use crate::source::{FileId, SourceMap};
use crate::types::TypeChecker;

/// True iff `line` carries a PEP 484 `# type: ignore` comment (optionally
/// `# type: ignore[code]`, optional space after the colon).
fn line_has_type_ignore(line: &str) -> bool {
    let mut rest = line;
    while let Some(pos) = rest.find('#') {
        let comment = rest[pos + 1..].trim_start();
        if let Some(after) = comment.strip_prefix("type:") {
            let after = after.trim_start();
            if let Some(tail) = after.strip_prefix("ignore") {
                let ok = tail.is_empty()
                    || tail.starts_with('[')
                    || tail.starts_with(char::is_whitespace)
                    || tail.starts_with('#');
                if ok {
                    return true;
                }
            }
        }
        rest = &rest[pos + 1..];
    }
    false
}

/// PEP 484 `# type: ignore` — drop static type/name errors whose span touches
/// a source line carrying the comment. A bare comment-only `# type: ignore`
/// before any code suppresses the whole module (mypy file-level semantics).
/// Runtime-behavior fixtures can use this when the static checker would
/// otherwise reject the exercised path at compile time.
fn filter_type_ignored(errors: Vec<MambaError>, source: &str) -> Vec<MambaError> {
    if errors.is_empty() {
        return errors;
    }
    let mut ignore_lines: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut file_level = false;
    let mut seen_code = false;
    for (i, line) in source.lines().enumerate() {
        if line_has_type_ignore(line) {
            ignore_lines.insert(i + 1);
            if !seen_code && line.trim_start().starts_with('#') {
                file_level = true;
            }
        }
        let t = line.trim_start();
        if !t.is_empty() && !t.starts_with('#') {
            seen_code = true;
        }
    }
    if file_level {
        return Vec::new();
    }
    if ignore_lines.is_empty() {
        return errors;
    }
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(
            source
                .bytes()
                .enumerate()
                .filter(|(_, b)| *b == b'\n')
                .map(|(i, _)| i + 1),
        )
        .collect();
    let line_of = |off: usize| -> usize {
        match line_starts.binary_search(&off) {
            Ok(i) => i + 1,
            Err(i) => i,
        }
    };
    errors
        .into_iter()
        .filter(|e| {
            let Some(span) = e.span() else { return true };
            let first = line_of(span.start as usize);
            let last = line_of((span.end as usize).max(span.start as usize));
            !(first..=last).any(|ln| ignore_lines.contains(&ln))
        })
        .collect()
}

/// The main compiler session driving the pipeline.
pub struct CompilerSession {
    pub config: CompilerConfig,
    pub source_map: SourceMap,
}

impl CompilerSession {
    pub fn new(config: CompilerConfig) -> Self {
        Self {
            config,
            source_map: SourceMap::new(),
        }
    }

    /// Create a session that auto-discovers and loads `mamba.toml` from `start_dir`.
    ///
    /// If no `mamba.toml` is found the session runs in single-file mode (no
    /// external crate wiring).
    pub fn new_from_project(start_dir: &std::path::Path, base_config: CompilerConfig) -> Self {
        let project_config = MambaConfig::discover(start_dir).map(|(cfg, _path)| cfg);
        Self::new(CompilerConfig {
            project_config,
            ..base_config
        })
    }

    /// Load a source file and return its FileId.
    pub fn load_file(&mut self, path: &str) -> crate::error::Result<FileId> {
        let source = std::fs::read_to_string(path)?;
        let file_id = self.source_map.add_file(path.to_string(), source);
        Ok(file_id)
    }

    /// Type-check only (no codegen).
    pub fn check(&mut self, path: &str) -> crate::error::Result<()> {
        let file_id = self.load_file(path)?;
        let source = self.source_map.get_file(file_id).source.clone();
        let mut module = parser::parse(&source, file_id)?;
        crate::lower::pep695::desugar_module(&mut module);
        let module = module;

        if let Some(EmitMode::Ast) = self.config.emit {
            println!("{module:#?}");
        }

        // Type check
        let mut checker = TypeChecker::new();
        let errors = filter_type_ignored(checker.check_module(&module), &source);
        if !errors.is_empty() {
            for err in &errors[1..] {
                eprintln!("{}", diagnostic::render_error(err, &self.source_map));
            }
            return Err(errors.into_iter().next().unwrap());
        }

        for diag in &checker.diagnostics {
            eprintln!("{}", render_type_diagnostic(diag, &self.source_map));
        }

        Ok(())
    }

    /// Parse, typecheck, lower, and optimize Mamba source code.
    pub fn compile_source(
        &mut self,
        source: &str,
        display_name: &str,
    ) -> crate::error::Result<(crate::mir::MirModule, TypeChecker, crate::hir::HirModule)> {
        let file_id = self
            .source_map
            .add_file(display_name.to_string(), source.to_string());
        let src = self.source_map.get_file(file_id).source.clone();
        let mut module = parser::parse(&src, file_id)?;
        crate::lower::pep695::desugar_module(&mut module);

        let mut checker = TypeChecker::new();
        checker.allow_runtime_unresolved_names = true;
        let errors = filter_type_ignored(checker.check_module(&module), &src);
        if !errors.is_empty() {
            for err in &errors[1..] {
                eprintln!("{}", diagnostic::render_error(err, &self.source_map));
            }
            return Err(errors.into_iter().next().unwrap());
        }

        let hir = lower::lower_module(&module, &checker)
            .map_err(|errs| errs.into_iter().next().unwrap())?;
        let mut mir_module = lower::lower_hir_to_mir_with_symbols_src(
            &hir,
            &checker.tcx,
            &checker.symbols,
            Some((display_name, src.as_str())),
        );

        if self.config.optimize {
            let pass_opts = crate::mir::opt::PassOptions::from_env_or_config(self.config.opt_level);
            crate::mir::opt::optimize_module_with_options(&mut mir_module, &checker.tcx, &pass_opts);
        }

        Ok((mir_module, checker, hir))
    }

    /// Full compilation pipeline: parse → typecheck → lower → codegen.
    pub fn build(&mut self, path: &str, _output: Option<&str>) -> crate::error::Result<Vec<u8>> {
        // #1190 R2: Initialize module search paths for multi-file builds.
        let script_path = std::path::Path::new(path);
        if let Ok(abs_path) = std::fs::canonicalize(script_path) {
            if let Some(parent) = abs_path.parent() {
                crate::runtime::module::mb_set_script_dir(parent.to_path_buf());
                crate::runtime::module::mb_insert_search_path(0, &parent.display().to_string());
            }
        } else if let Some(parent) = script_path.parent() {
            crate::runtime::module::mb_set_script_dir(parent.to_path_buf());
            crate::runtime::module::mb_insert_search_path(0, &parent.display().to_string());
        }
        crate::runtime::module::mb_init_search_paths();

        let file_id = self.load_file(path)?;
        let source = self.source_map.get_file(file_id).source.clone();
        let mut module = parser::parse(&source, file_id)?;
        crate::lower::pep695::desugar_module(&mut module);
        let module = module;

        if let Some(EmitMode::Ast) = self.config.emit {
            println!("{module:#?}");
            return Ok(Vec::new());
        }

        // Type check — resolve and pre-check all imported dependency modules
        // first so the shared TypeChecker accumulates cross-module type info.
        let mut checker = TypeChecker::new();
        self.check_dependencies(path, &mut checker);
        let errors = filter_type_ignored(checker.check_module(&module), &source);
        if !errors.is_empty() {
            for err in &errors[1..] {
                eprintln!("{}", diagnostic::render_error(err, &self.source_map));
            }
            return Err(errors.into_iter().next().unwrap());
        }

        // Lower AST → HIR
        let hir = lower::lower_module(&module, &checker)
            .map_err(|errs| errs.into_iter().next().unwrap())?;

        if let Some(EmitMode::Hir) = self.config.emit {
            println!("{hir:#?}");
            return Ok(Vec::new());
        }

        // Lower HIR → MIR (with builtin resolution)
        let mut mir_module = lower::lower_hir_to_mir_with_symbols_src(
            &hir,
            &checker.tcx,
            &checker.symbols,
            Some((path, source.as_str())),
        );

        if self.config.optimize {
            let pass_opts = crate::mir::opt::PassOptions::from_env_or_config(self.config.opt_level);
            crate::mir::opt::optimize_module_with_options(&mut mir_module, &checker.tcx, &pass_opts);
        }

        if let Some(EmitMode::Mir) = self.config.emit {
            println!("{mir_module:#?}");
            return Ok(Vec::new());
        }

        // Codegen — select backend from config (#305 R4)
        let mut backend: Box<dyn CodegenBackend> = match self.config.backend {
            Backend::Llvm => {
                let mut llvm = LlvmBackend::new();
                llvm = match self.config.opt_level {
                    OptLevel::O0 => llvm.with_opt(crate::codegen::llvm::OptLevel::O0),
                    OptLevel::O1 => llvm.with_opt(crate::codegen::llvm::OptLevel::O1),
                    OptLevel::O2 => llvm.with_opt(crate::codegen::llvm::OptLevel::O2),
                    OptLevel::O3 => llvm.with_opt(crate::codegen::llvm::OptLevel::O3),
                };
                Box::new(llvm)
            }
            _ => {
                let cl = CraneliftBackend::new().map_err(|e| MambaError::codegen(e.to_string()))?;
                Box::new(cl)
            }
        };
        let output = backend
            .codegen(&mir_module, &checker.tcx)
            .map_err(|e| MambaError::codegen(e.to_string()))?;

        match output {
            CodegenOutput::ObjectFile(bytes) => Ok(bytes),
            CodegenOutput::LlvmIr(ir) => Ok(ir.into_bytes()),
            _ => Ok(Vec::new()),
        }
    }

    fn execute_jit_entry(entry: *const u8) -> crate::error::Result<()> {
        // #1845: serialize against any other execution touching the
        // process-global async runtime state (see ASYNC_STATE_TEST_LOCK's
        // doc comment) — this run's coroutines/tasks live from here through
        // cleanup_all_runtime_state() below.
        let _async_guard = crate::runtime::async_rt::ASYNC_STATE_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let main_fn: fn() -> i64 = unsafe { std::mem::transmute(entry) };
        let _result = main_fn();

        let pending_exception = crate::runtime::exception::mb_catch_exception();
        let pending_error = if pending_exception.is_none() {
            crate::runtime::exception::mb_take_uncaught_traceback()
        } else {
            let exc_type = crate::runtime::exception::get_exception_type_pub(pending_exception)
                .unwrap_or_else(|| "Exception".to_string());
            let message = crate::runtime::exception::get_exception_message_pub(pending_exception)
                .unwrap_or_default();
            Some(if message.is_empty() {
                exc_type
            } else {
                format!("{exc_type}: {message}")
            })
        };

        crate::runtime::stdlib::threading_mod::join_non_daemon_threads();
        crate::runtime::cleanup_all_runtime_state_for_process_exit();
        pending_error.map_or(Ok(()), |message| Err(MambaError::Other(message)))
    }

    /// JIT compile and execute Mamba source given as a string.
    ///
    /// `display_name` is used for error messages (e.g. `"<stdin>"`).
    /// Unlike [`run`], this method skips file I/O and search-path setup
    /// derived from a file path — it is intended for piped/stdin input.
    ///
    /// # REQ: R1, R2
    pub fn run_source(&mut self, source: &str, display_name: &str) -> crate::error::Result<()> {
        // Register native modules so JIT-compiled code can call into Rust bindings.
        crate::runtime::module::mb_register_native_modules();

        // Initialize default search paths (PYTHONPATH, stdlib defaults).
        crate::runtime::module::mb_init_search_paths();

        // Collect external crate symbols when in project mode.
        let ext_syms = register_external_modules(self.config.project_config.as_ref());

        let file_id = self
            .source_map
            .add_file(display_name.to_string(), source.to_string());
        let src = self.source_map.get_file(file_id).source.clone();
        let mut module = parser::parse(&src, file_id)?;
        crate::lower::pep695::desugar_module(&mut module);
        let module = module;

        // Enforce expose filtering for native-module imports when a project config is active.
        if let Some(proj) = &self.config.project_config {
            self.check_native_imports(&module, proj)?;
        }

        let mut checker = TypeChecker::new();
        checker.allow_runtime_unresolved_names = true;
        // No check_dependencies — stdin source has no associated file path.
        let errors = filter_type_ignored(checker.check_module(&module), &src);
        if !errors.is_empty() {
            for err in &errors[1..] {
                eprintln!("{}", diagnostic::render_error(err, &self.source_map));
            }
            return Err(errors.into_iter().next().unwrap());
        }

        let hir = lower::lower_module(&module, &checker)
            .map_err(|errs| errs.into_iter().next().unwrap())?;
        let mut mir_module = lower::lower_hir_to_mir_with_symbols_src(
            &hir,
            &checker.tcx,
            &checker.symbols,
            Some((display_name, src.as_str())),
        );

        if self.config.optimize {
            let pass_opts = crate::mir::opt::PassOptions::from_env_or_config(self.config.opt_level);
            crate::mir::opt::optimize_module_with_options(&mut mir_module, &checker.tcx, &pass_opts);
        }

        let mut backend = CraneliftJitBackend::new_with_externals(&ext_syms)
            .map_err(|e| MambaError::codegen(e.to_string()))?;
        let output = backend
            .codegen(&mir_module, &checker.tcx)
            .map_err(|e| MambaError::codegen(e.to_string()))?;

        match output {
            CodegenOutput::Jit { entry } => {
                // HANDWRITE-BEGIN gap="standardize:projects-mamba-src-driver-mod-rs" tracker="standardize-gap-projects-mamba-src-driver-mod-rs" reason="introspection-builtins (issue: enhancement-mamba-introspection-builtins-globals-locals-vars-dir)."
                // Populate the introspection registry so `globals()` / `locals()`
                // can name and type-tag the entries that the JIT will write into
                // GLOBAL_ID_NAMESPACE during execution.
                let func_addrs = collect_user_func_addrs(&hir, &checker, &backend);
                let (sym_info, func_info) =
                    crate::runtime::module::build_introspection_state(&checker, &hir, &func_addrs);
                crate::runtime::closure::set_module_sym_info(sym_info);
                crate::runtime::closure::set_module_func_info(func_info);
                // HANDWRITE-END
                // #945: the entry script's `__name__` is "__main__"; only
                // imported modules get their own module name (seeded
                // separately in runtime::module::compile_and_exec_module).
                // hir_to_mir no longer hardcodes "__main__" on first read of
                // `__name__`, so the entry point must pre-seed it here.
                if let Some(name_sym) = checker.symbols.lookup("__name__") {
                    crate::runtime::closure::mb_global_set_id(
                        crate::runtime::value::MbValue::from_bits(name_sym.0 as u64),
                        crate::runtime::value::MbValue::from_ptr(
                            crate::runtime::rc::MbObject::new_str("__main__".to_string()),
                        ),
                    );
                }
                Self::execute_jit_entry(entry)
            }
            _ => Err(MambaError::codegen("expected JIT output".to_string())),
        }
    }

    /// JIT compile and execute a Mamba source file.
    ///
    /// When a `project_config` is present in `self.config`, all registered
    /// `MAMBA_MODULES` are wired into the Cranelift JIT symbol table so that
    /// JIT-compiled code can call into native Rust binding crates.
    pub fn run(&mut self, path: &str) -> crate::error::Result<()> {
        // Populate the runtime module cache with native modules so that
        // `mb_import()` / `mb_module_getattr()` can resolve them (#1132 R1).
        crate::runtime::module::mb_register_native_modules();

        // #1190 R2: Initialize module search paths from the script's directory.
        // Order matters: script dir first, then PYTHONPATH, then defaults.
        let script_path = std::path::Path::new(path);
        if let Ok(abs_path) = std::fs::canonicalize(script_path) {
            if let Some(parent) = abs_path.parent() {
                crate::runtime::module::mb_set_script_dir(parent.to_path_buf());
                crate::runtime::module::mb_insert_search_path(0, &parent.display().to_string());
            }
        } else if let Some(parent) = script_path.parent() {
            // Fallback: use the path as-is if canonicalize fails.
            crate::runtime::module::mb_set_script_dir(parent.to_path_buf());
            crate::runtime::module::mb_insert_search_path(0, &parent.display().to_string());
        }
        crate::runtime::module::mb_init_search_paths();

        // If a mamba.toml project is present, add its directory to search paths.
        if let Some(_proj) = &self.config.project_config {
            if let Ok(abs_path) = std::fs::canonicalize(script_path) {
                // Walk up to find the mamba.toml directory.
                if let Some((_, toml_path)) =
                    MambaConfig::discover(abs_path.parent().unwrap_or(std::path::Path::new(".")))
                {
                    if let Some(proj_dir) = toml_path.parent() {
                        let proj_str = proj_dir.display().to_string();
                        crate::runtime::module::mb_add_search_path(
                            crate::runtime::value::MbValue::from_ptr(
                                crate::runtime::rc::MbObject::new_str(proj_str),
                            ),
                        );
                    }
                }
            }
        }

        let file_id = self.load_file(path)?;
        let source = self.source_map.get_file(file_id).source.clone();
        let mut module = parser::parse(&source, file_id)?;
        crate::lower::pep695::desugar_module(&mut module);
        let module = module;

        // Type check — resolve and pre-check all imported dependency modules
        // first so the shared TypeChecker accumulates cross-module type info.
        let mut checker = TypeChecker::new();
        checker.allow_runtime_unresolved_names = true;
        self.check_dependencies(path, &mut checker);

        // Enforce expose filtering for native-module imports when a project
        // config is active (R3).
        if let Some(proj) = &self.config.project_config {
            self.check_native_imports(&module, proj)?;
        }

        let errors = filter_type_ignored(checker.check_module(&module), &source);
        if !errors.is_empty() {
            for err in &errors[1..] {
                eprintln!("{}", diagnostic::render_error(err, &self.source_map));
            }
            return Err(errors.into_iter().next().unwrap());
        }

        // Lower
        let hir = lower::lower_module(&module, &checker)
            .map_err(|errs| errs.into_iter().next().unwrap())?;
        let mut mir_module = lower::lower_hir_to_mir_with_symbols_src(
            &hir,
            &checker.tcx,
            &checker.symbols,
            Some((path, source.as_str())),
        );

        if self.config.optimize {
            let pass_opts = crate::mir::opt::PassOptions::from_env_or_config(self.config.opt_level);
            crate::mir::opt::optimize_module_with_options(&mut mir_module, &checker.tcx, &pass_opts);
        }

        // Collect external crate symbols when in project mode (R2).
        let ext_syms = register_external_modules(self.config.project_config.as_ref());

        // JIT compile — inject external symbols so native calls resolve.
        let mut backend = CraneliftJitBackend::new_with_externals(&ext_syms)
            .map_err(|e| MambaError::codegen(e.to_string()))?;
        let output = backend
            .codegen(&mir_module, &checker.tcx)
            .map_err(|e| MambaError::codegen(e.to_string()))?;

        match output {
            CodegenOutput::Jit { entry } => {
                // HANDWRITE-BEGIN gap="standardize:projects-mamba-src-driver-mod-rs" tracker="standardize-gap-projects-mamba-src-driver-mod-rs" reason="introspection-builtins (issue: enhancement-mamba-introspection-builtins-globals-locals-vars-dir)."
                let func_addrs = collect_user_func_addrs(&hir, &checker, &backend);
                let (sym_info, func_info) =
                    crate::runtime::module::build_introspection_state(&checker, &hir, &func_addrs);
                crate::runtime::closure::set_module_sym_info(sym_info);
                crate::runtime::closure::set_module_func_info(func_info);
                // HANDWRITE-END
                // #945: the entry script's `__name__` is "__main__"; only
                // imported modules get their own module name (seeded
                // separately in runtime::module::compile_and_exec_module).
                // hir_to_mir no longer hardcodes "__main__" on first read of
                // `__name__`, so the entry point must pre-seed it here.
                if let Some(name_sym) = checker.symbols.lookup("__name__") {
                    crate::runtime::closure::mb_global_set_id(
                        crate::runtime::value::MbValue::from_bits(name_sym.0 as u64),
                        crate::runtime::value::MbValue::from_ptr(
                            crate::runtime::rc::MbObject::new_str("__main__".to_string()),
                        ),
                    );
                }
                Self::execute_jit_entry(entry)
            }
            _ => Err(MambaError::codegen("expected JIT output".to_string())),
        }
    }

    // ── Native module helpers ─────────────────────────────────────────────────

    /// Check every top-level import in `module` against the expose lists in
    /// `proj`.  Returns `Err` with an `ImportError` message if any symbol is
    /// not exposed by its crate configuration.
    // (introspection-builtins helper hangs off the impl below — see end of file.)
    fn check_native_imports(
        &self,
        module: &crate::parser::ast::Module,
        proj: &MambaConfig,
    ) -> crate::error::Result<()> {
        use crate::parser::ast::Stmt;
        use mamba_registry::find_module;

        for spanned in &module.stmts {
            if let Stmt::Import {
                module: mod_path,
                names: Some(names),
                ..
            } = &spanned.node
            {
                if mod_path.is_empty() {
                    continue;
                }
                // Convert dotted module path to the import name (underscored).
                let import_name = mod_path.join("_");
                let import_dotted = mod_path.join(".");

                // Only enforce filtering for modules that are registered as
                // native (i.e., present in MAMBA_MODULES).
                if find_module(&import_dotted).is_none() && find_module(&import_name).is_none() {
                    continue;
                }

                // Determine which crate name key to use for the expose map.
                // Convention: use the hyphenated form (e.g. "cclab-schema-mamba").
                // We also try the dotted and underscored forms as fallbacks so
                // that users can write any variant in their mamba.toml.
                let crate_key = import_dotted.replace('.', "-").replace('_', "-");

                // Find the first key variant that actually exists in the crates
                // map.  If none exists, the crate has no expose restrictions and
                // all symbols are allowed.
                let active_expose_key: Option<String> = [
                    crate_key.as_str(),
                    import_dotted.as_str(),
                    import_name.as_str(),
                ]
                .iter()
                .find(|&&k| proj.has_crate_expose(k))
                .map(|&k| k.to_string());

                for (sym, _alias) in names {
                    if sym == "*" {
                        continue; // star import: skip (handled at runtime)
                    }
                    if let Some(ref key) = active_expose_key {
                        if !proj.is_symbol_exposed(key, sym) {
                            return Err(MambaError::Other(format!(
                                "ImportError: symbol '{sym}' is not in the expose list for '{import_dotted}'"
                            )));
                        }
                    }
                    // No expose entry for this crate → all symbols allowed.
                }
            }
        }
        Ok(())
    }

    /// Resolve and type-check all modules that `path` depends on (in topo
    /// order, dependencies first) so that `checker` accumulates cross-module
    /// type information before the entry module itself is checked.
    ///
    /// Errors from dependency modules are printed to stderr but do NOT abort
    /// compilation — the entry module check will surface any cascading issues.
    fn check_dependencies(&self, path: &str, checker: &mut TypeChecker) {
        let abs_path =
            std::fs::canonicalize(path).unwrap_or_else(|_| std::path::PathBuf::from(path));
        let parent_dir = abs_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();

        let mut graph = module_graph::ModuleGraph::new(vec![parent_dir]);
        if let Err(errs) = graph.add_root(&abs_path) {
            for e in &errs {
                eprintln!("module graph warning: {e}");
            }
        }

        let order = match graph.topo_sort() {
            Ok(o) => o,
            Err(e) => {
                eprintln!("module graph cycle: {e}");
                return;
            }
        };

        for node in &order {
            // Skip the entry module — it will be checked by the caller.
            if node.path == abs_path {
                continue;
            }
            let dep_errors = checker.check_module(&node.ast);
            for err in &dep_errors {
                eprintln!("in module {}: {err}", node.name);
            }
        }
    }

    /// Render an error with source context.
    pub fn render_error(&self, err: &MambaError) -> String {
        diagnostic::render_error(err, &self.source_map)
    }

    /// Render a type diagnostic with source context.
    pub fn render_type_diagnostic(&self, diag: &crate::types::check::Diagnostic) -> String {
        render_type_diagnostic(diag, &self.source_map)
    }
}

pub fn render_type_diagnostic(
    diag: &crate::types::check::Diagnostic,
    source_map: &crate::source::SourceMap,
) -> String {
    let level_str = match diag.level {
        crate::types::check::DiagLevel::Warning => "warning",
        crate::types::check::DiagLevel::Error => "error",
    };
    if diag.span.file.0 != 0 || diag.span.start != 0 || diag.span.end != 0 {
        let file = source_map.get_file(diag.span.file);
        let (line, col) = file.line_col(diag.span.start);
        let src_line = file.line_text(line);
        format!(
            "{level_str}: {}\n  --> {}:{line}:{col}\n   |\n{line:>3} | {src_line}\n   |",
            diag.message, file.name
        )
    } else {
        format!("{level_str}: {}", diag.message)
    }
}

#[cfg(test)]
mod tests {
    // Force-link schema-mamba so its #[distributed_slice(MAMBA_MODULES)]
    // entry is included in the test binary.  This enables find_module("cclab_schema_mamba")
    // to return Some, making the expose-filtering tests exercise the real code path.
    use schema_mamba as _;
    #[cfg(feature = "native-modules")]
    use mambalibs_http_binding as _;
    #[cfg(feature = "native-modules")]
    use pgkit_binding as _;

    use super::*;
    use crate::pkgmanage::manifest::schema::{CrateConfig, CrateEntry, ProjectConfig};
    use std::collections::HashMap;

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_session_with_expose(expose: HashMap<String, Vec<String>>) -> CompilerSession {
        // Convert the flat expose map to the unified CrateEntry-based format.
        let crates: HashMap<String, CrateEntry> = expose
            .into_iter()
            .map(|(name, syms)| {
                (
                    name,
                    CrateEntry::Config(CrateConfig {
                        crate_name: None,
                        version: Some("0.1.0".to_string()),
                        path: None,
                        expose: syms,
                        module: None,
                    }),
                )
            })
            .collect();
        let proj = MambaConfig {
            project: ProjectConfig {
                name: "test".to_string(),
                version: "0.1.0".to_string(),
                entry_point: Some("main.py".to_string()),
            },
            entry_point: None,
            crates,
            expose: Default::default(),
            build: Default::default(),
            paths: Default::default(),
        };
        CompilerSession::new(CompilerConfig {
            project_config: Some(proj),
            ..Default::default()
        })
    }

    fn parse_src(session: &mut CompilerSession, src: &str) -> crate::parser::ast::Module {
        let file_id = session
            .source_map
            .add_file("test.py".to_string(), src.to_string());
        let source = session.source_map.get_file(file_id).source.clone();
        crate::parser::parse(&source, file_id).expect("parse should succeed for valid source")
    }

    #[test]
    fn type_ignore_semantics_are_independent_of_fixture_markers() {
        for marker in ["", "# mamba-strict-type: TypeError\n"] {
            let file = tempfile::NamedTempFile::with_suffix(".py").unwrap();
            std::fs::write(
                file.path(),
                format!("{marker}value: int = \"wrong\"  # type: ignore[assignment]\n"),
            )
            .unwrap();

            let mut session = CompilerSession::new(CompilerConfig::default());
            assert!(
                session.check(file.path().to_str().unwrap()).is_ok(),
                "fixture metadata must not change # type: ignore semantics"
            );

            std::fs::write(file.path(), format!("{marker}value: int = \"wrong\"\n")).unwrap();
            let mut session = CompilerSession::new(CompilerConfig::default());
            assert!(
                session.check(file.path().to_str().unwrap()).is_err(),
                "fixture metadata must not disable annotation enforcement"
            );
        }
    }

    // ── check_native_imports: expose filtering (R3) ───────────────────────────

    #[test]
    fn check_native_imports_allows_all_when_expose_absent() {
        // No expose entry for cclab_schema_mamba → all symbols allowed.
        let src = "from cclab_schema_mamba import BaseModel, Field, Validator";
        let mut session = make_session_with_expose(HashMap::new());
        let module = parse_src(&mut session, src);
        let proj = session.config.project_config.clone().unwrap();
        assert!(
            session.check_native_imports(&module, &proj).is_ok(),
            "absent expose list must allow all symbols"
        );
    }

    #[test]
    fn check_native_imports_allows_listed_symbol() {
        let src = "from cclab_schema_mamba import BaseModel";
        let mut expose = HashMap::new();
        expose.insert(
            "cclab-schema-mamba".to_string(),
            vec!["BaseModel".to_string()],
        );
        let mut session = make_session_with_expose(expose);
        let module = parse_src(&mut session, src);
        let proj = session.config.project_config.clone().unwrap();
        assert!(
            session.check_native_imports(&module, &proj).is_ok(),
            "BaseModel is in the expose list — must be allowed"
        );
    }

    #[test]
    fn check_native_imports_blocks_unlisted_symbol_with_import_error() {
        // Field is NOT in the expose list → compile-time ImportError (R3).
        let src = "from cclab_schema_mamba import BaseModel, Field";
        let mut expose = HashMap::new();
        expose.insert(
            "cclab-schema-mamba".to_string(),
            vec!["BaseModel".to_string()],
        );
        let mut session = make_session_with_expose(expose);
        let module = parse_src(&mut session, src);
        let proj = session.config.project_config.clone().unwrap();
        let err = session
            .check_native_imports(&module, &proj)
            .expect_err("Field is not exposed — must produce an ImportError");
        let msg = format!("{err}");
        assert!(
            msg.contains("ImportError"),
            "error must identify as ImportError; got: {msg}"
        );
        assert!(
            msg.contains("Field"),
            "error must name the blocked symbol; got: {msg}"
        );
    }

    #[test]
    fn check_native_imports_star_import_is_not_filtered() {
        // Star imports skip per-symbol filtering (handled at runtime).
        let src = "from cclab_schema_mamba import *";
        let mut expose = HashMap::new();
        expose.insert(
            "cclab-schema-mamba".to_string(),
            vec!["BaseModel".to_string()],
        );
        let mut session = make_session_with_expose(expose);
        let module = parse_src(&mut session, src);
        let proj = session.config.project_config.clone().unwrap();
        assert!(
            session.check_native_imports(&module, &proj).is_ok(),
            "star import must not be filtered at compile time"
        );
    }

    #[test]
    fn check_native_imports_non_native_module_always_passes() {
        // os.path is not in MAMBA_MODULES → filtering must not apply.
        let src = "from os.path import join, exists";
        let mut expose = HashMap::new();
        // Even with a restrictive expose on "os-path", non-native imports pass through.
        expose.insert("os-path".to_string(), vec!["nonexistent".to_string()]);
        let mut session = make_session_with_expose(expose);
        let module = parse_src(&mut session, src);
        let proj = session.config.project_config.clone().unwrap();
        assert!(
            session.check_native_imports(&module, &proj).is_ok(),
            "non-native imports must never be filtered by expose lists"
        );
    }

    // ── Project-mode E2E: CompilerSession::new_from_project (R1) ─────────────

    #[test]
    fn new_from_project_sets_project_config_when_mamba_toml_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("mamba.toml"),
            "[project]\nname = \"test\"\nversion = \"0.1.0\"\nentry_point = \"src/main.py\"\n",
        )
        .unwrap();

        let session = CompilerSession::new_from_project(dir.path(), CompilerConfig::default());
        let proj = session
            .config
            .project_config
            .expect("project_config must be Some when mamba.toml is present");
        assert_eq!(proj.entry_point(), Some("src/main.py"));
    }

    #[test]
    fn new_from_project_walks_up_to_find_mamba_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("mamba.toml"),
            "[project]\nname = \"test\"\nversion = \"0.1.0\"\nentry_point = \"app.py\"\n",
        )
        .unwrap();

        // Start from a deeply-nested subdirectory -- discovery must walk up.
        let sub = dir.path().join("src").join("api").join("v2");
        std::fs::create_dir_all(&sub).unwrap();

        let session = CompilerSession::new_from_project(&sub, CompilerConfig::default());
        let proj = session
            .config
            .project_config
            .expect("should find mamba.toml by walking up parent dirs");
        assert_eq!(proj.entry_point(), Some("app.py"));
    }

    #[test]
    fn new_from_project_single_file_mode_when_no_mamba_toml() {
        // Create an isolated subtree that is guaranteed not to have a mamba.toml
        // anywhere above it (we place a sentinel dir inside the tempdir).
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("no_config_here");
        std::fs::create_dir_all(&sub).unwrap();

        // If the temp path chain happens to contain a mamba.toml (developer
        // machine edge case), we cannot assert None.  We verify no panic instead
        // and trust the discover tests above to cover the None branch.
        let session = CompilerSession::new_from_project(&sub, CompilerConfig::default());
        // Verify the session is usable regardless of mode.
        let _ = session.config.project_config;
    }

    #[test]
    fn new_from_project_preserves_base_config_fields() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("mamba.toml"),
            "[project]\nname = \"test\"\nversion = \"0.1.0\"\nentry_point = \"main.py\"\n",
        )
        .unwrap();

        let base = CompilerConfig {
            backend: Backend::CraneliftJit,
            emit: Some(EmitMode::Ast),
            opt_level: OptLevel::O2,
            project_config: None,
            optimize: true,
        };
        let session = CompilerSession::new_from_project(dir.path(), base);
        // Non-project fields from the base config must be preserved.
        assert_eq!(session.config.backend, Backend::CraneliftJit);
        assert_eq!(session.config.emit, Some(EmitMode::Ast));
        assert_eq!(session.config.opt_level, OptLevel::O2);
        // project_config must now be set from the discovered mamba.toml.
        assert!(session.config.project_config.is_some());
    }

    // ── register_external_modules (R2) ────────────────────────────────────────

    #[test]
    fn register_external_modules_returns_empty_in_single_file_mode() {
        let syms = register_external_modules(None);
        assert!(
            syms.is_empty(),
            "single-file mode must return no external symbols"
        );
    }

    #[test]
    fn register_external_modules_returns_symbols_in_project_mode() {
        let proj = MambaConfig {
            project: ProjectConfig {
                name: "test".to_string(),
                version: "0.1.0".to_string(),
                entry_point: Some("main.py".to_string()),
            },
            entry_point: None,
            crates: Default::default(),
            expose: Default::default(),
            build: Default::default(),
            paths: Default::default(),
        };
        let syms = register_external_modules(Some(&proj));
        // With schema-mamba force-linked, we expect ≥ 5 symbols.
        assert!(
            !syms.is_empty(),
            "project mode with a linked native module must expose symbols"
        );
        let names: Vec<&str> = syms.iter().map(|(n, _)| *n).collect();
        assert!(
            names.contains(&"mb_schema_base_model_new"),
            "missing mb_schema_base_model_new"
        );
        assert!(
            names.contains(&"mb_schema_validate"),
            "missing mb_schema_validate"
        );
        assert!(
            names.contains(&"mb_schema_to_json_schema"),
            "missing mb_schema_to_json_schema"
        );
    }

    #[test]
    fn pep634_builtin_self_match_classes_bind_the_subject() {
        for (class_name, subject) in [
            ("bool", "True"),
            ("bytearray", "bytearray(b\"x\")"),
            ("bytes", "b\"x\""),
            ("dict", "{\"x\": 1}"),
            ("float", "1.5"),
            ("frozenset", "frozenset([1])"),
            ("int", "1"),
            ("list", "[1]"),
            ("set", "set([1])"),
            ("str", "\"x\""),
            ("tuple", "(1,)"),
        ] {
            let src = format!(
                "subject = {subject}\nmatch subject:\n    case {class_name}(value):\n        if value == subject:\n            print(\"matched\")\n"
            );
            let previous = crate::runtime::output::begin_capture();
            let mut session = CompilerSession::new(CompilerConfig::default());
            let result = session.run_source(&src, "pep634_builtin_self_match_class.py");
            let captured = crate::runtime::output::end_capture(previous);
            crate::runtime::cleanup_all_runtime_state();

            result.unwrap_or_else(|error| panic!("{class_name} self-match failed: {error}"));
            assert_eq!(
                captured.trim(),
                "matched",
                "{class_name} must bind the whole subject"
            );
        }
    }

    #[test]
    fn class_patterns_evaluate_alias_values_instead_of_alias_names() {
        let src = r#"
BuiltinAlias = int
match 7:
    case BuiltinAlias(value):
        print(value)
class Point:
    __match_args__ = ("x",)
    def __init__(self, x):
        self.x = x
PointAlias = Point
match Point(9):
    case PointAlias(value):
        print(value)
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "class_pattern_alias_values.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("class-pattern aliases should run from source");
        assert_eq!(captured.lines().collect::<Vec<_>>(), ["7", "9"]);
    }

    #[test]
    fn repeated_function_definitions_bind_in_source_order() {
        let src = r#"
def value() -> int:
    return 1
old = value
print(value())
def value() -> str:
    return "two"
print(old())
print(value())
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "repeated_function_definitions.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("repeated function definitions should run from source");
        assert_eq!(captured.lines().collect::<Vec<_>>(), ["1", "1", "two"]);
    }

    #[test]
    fn nested_and_conditional_function_definitions_bind_their_runtime_slots() {
        let src = r#"
def outer():
    def value():
        return 1
    old = value
    def value():
        return 2
    print(old())
    print(value())
outer()
if True:
    def conditional():
        return 3
print(conditional())
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "nested_function_bindings.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("nested and conditional function definitions should run from source");
        assert_eq!(captured.lines().collect::<Vec<_>>(), ["1", "2", "3"]);
    }

    #[test]
    fn repeated_class_definitions_keep_runtime_identity_and_source_order() {
        let src = r#"
class C:
    def value(self):
        return 1
Old = C
old = C()
class C:
    def value(self):
        return "two"
new = C()
print(Old is C)
print(type(old) is Old)
print(type(old) is C)
print(old.value())
print(new.value())
print(Old.__name__)
print(C.__name__)
print(isinstance(old, Old))
print(isinstance(old, C))
Previous = C
class C(C):
    pass
print(C.__bases__[0] is Previous)
def replace(cls):
    return 42
@replace
class Decorated:
    pass
print(Decorated)
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "repeated_class_definitions.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("repeated class definitions should run from source");
        assert_eq!(
            captured.lines().collect::<Vec<_>>(),
            [
                "False", "True", "False", "1", "two", "C", "C", "True", "False", "True",
                "42"
            ]
        );
    }

    #[test]
    fn nested_and_conditional_class_definitions_bind_their_runtime_slots() {
        let src = r#"
def outer():
    class Local:
        def value(self):
            return 1
    old = Local
    class Local:
        def value(self):
            return 2
    print(old().value())
    print(Local().value())
    print(old is Local)
outer()
if True:
    class Conditional:
        def value(self):
            return 3
print(Conditional().value())
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "nested_class_bindings.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("nested and conditional class definitions should run from source");
        assert_eq!(
            captured.lines().collect::<Vec<_>>(),
            ["1", "2", "False", "3"]
        );
    }

    #[test]
    fn each_class_statement_execution_allocates_fresh_runtime_identity() {
        let src = r#"
def make():
    class C:
        def value(self):
            return 1
    return C
First = make()
Second = make()
first = First()
second = Second()
print(First is Second)
print(type(first) is First)
print(type(first) is Second)
print(type(second) is Second)
print(first)
print(second)
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "fresh_class_execution_identity.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("each class statement execution should allocate a fresh identity");
        assert_eq!(
            captured.lines().collect::<Vec<_>>(),
            ["False", "True", "False", "True", "<C instance>", "<C instance>"]
        );
    }

    #[test]
    fn repeated_local_class_execution_keeps_its_class_cell_identity() {
        let src = r#"
class BaseOne:
    def value(self):
        return 1
class BaseTwo:
    def value(self):
        return 2
def make(base):
    class Child(base):
        def value(self):
            return super().value()
        def owner(self):
            return __class__
    return Child
First = make(BaseOne)
Second = make(BaseTwo)
print(First().owner() is First)
print(First().owner() is Second)
print(Second().owner() is Second)
print(First.__bases__[0] is BaseOne)
print(Second.__bases__[0] is BaseTwo)
print(First.__bases__[0].__name__)
print(Second.__bases__[0].__name__)
print(First().value())
print(Second().value())
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "repeated_local_class_cell.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.unwrap_or_else(|error| {
            panic!(
                "each local class execution should retain its own class cell: {error}; output={captured:?}"
            )
        });
        assert_eq!(
            captured.lines().collect::<Vec<_>>(),
            ["True", "False", "True", "True", "True", "BaseOne", "BaseTwo", "1", "2"]
        );
    }

    #[test]
    fn runtime_class_base_runs_init_subclass_once_with_kwargs() {
        let src = r#"
events = []
class Base:
    def __init_subclass__(cls, **kwargs):
        events.append(kwargs["flag"])
def make(base):
    class Child(base, flag=7):
        pass
    return Child
Child = make(Base)
print(len(events))
print(events[0])
print(Child.__bases__[0] is Base)
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "runtime_base_init_subclass.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.unwrap_or_else(|error| {
            panic!("runtime class base lifecycle failed: {error}; output={captured:?}")
        });
        assert_eq!(captured.lines().collect::<Vec<_>>(), ["1", "7", "True"]);
    }

    #[test]
    fn class_cell_survives_descriptors_methods_and_class_decorators() {
        let src = r#"
from typing import Any
def identity(func: Any) -> Any:
    return func
def make():
    class C:
        @property
        def owner(self):
            return __class__
        @owner.setter
        def owner(self, value):
            self.setter_owner = __class__
        @owner.deleter
        def owner(self):
            self.deleter_owner = __class__
        @classmethod
        def class_owner(cls):
            return (__class__, cls)
        @staticmethod
        def static_owner():
            return __class__
        @identity
        def decorated_owner(self):
            return __class__
    return C
First = make()
old_method = First().decorated_owner
Second = make()
first = First()
first.owner = 1
del first.owner
print(first.owner is First)
print(first.setter_owner is First)
print(first.deleter_owner is First)
print(First.static_owner() is First)
print(Second.static_owner() is Second)
print(old_method() is First)
class Sub(First):
    pass
owners = Sub.class_owner()
print(owners[0] is First)
print(owners[1] is Sub)
class Plain:
    def owner(self):
        return __class__
print(Plain().owner() is Plain)
def replace(cls: Any) -> Any:
    return cls()
@replace
class Decorated:
    def owner(self):
        return __class__
print(getattr(Decorated, "owner")() is type(Decorated))
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "class_cell_descriptors.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.unwrap_or_else(|error| {
            panic!("class-cell descriptor lifecycle failed: {error}; output={captured:?}")
        });
        assert_eq!(
            captured.lines().collect::<Vec<_>>(),
            ["True", "True", "True", "True", "True", "True", "True", "True", "True", "True"]
        );
    }

    #[test]
    fn deferred_methods_keep_their_class_cell_identity() {
        let src = r#"
import asyncio
def make():
    class C:
        async def async_owner(self):
            return __class__
        def generator_owner(self):
            yield __class__
    return C
First = make()
Second = make()
print(asyncio.run(First().async_owner()) is First)
print(asyncio.run(Second().async_owner()) is Second)
print(next(First().generator_owner()) is First)
print(next(Second().generator_owner()) is Second)
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "deferred_method_class_cell.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.unwrap_or_else(|error| {
            panic!("deferred method class-cell lifecycle failed: {error}; output={captured:?}")
        });
        assert_eq!(
            captured.lines().collect::<Vec<_>>(),
            ["True", "True", "True", "True"]
        );
    }

    #[test]
    fn zero_arg_super_without_a_first_parameter_raises_runtime_error() {
        let src = r#"
class C:
    @staticmethod
    def broken():
        return super()
try:
    C.broken()
except RuntimeError as error:
    print(str(error))
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "zero_arg_super_no_parameter.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("zero-argument super without a first parameter should be catchable");
        assert_eq!(captured.trim(), "super(): no arguments");
    }

    #[test]
    fn user_metaclass_identity_survives_same_name_rebinding() {
        let src = r#"
class Meta(type):
    pass
OldMeta = Meta
class Meta(type):
    pass
class First(metaclass=OldMeta):
    pass
class Second(metaclass=Meta):
    pass
print(type(First) is OldMeta)
print(type(First) is Meta)
print(type(Second) is Meta)
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "same_name_metaclass_identity.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("same-named metaclasses should retain nominal identity");
        assert_eq!(
            captured.lines().collect::<Vec<_>>(),
            ["True", "False", "True"]
        );
    }

    #[test]
    fn metaclass_definition_hooks_receive_visible_class_name() {
        let src = r#"
class HookMeta(type):
    @classmethod
    def __prepare__(mcls, name, bases):
        print("prepare:" + name)
        return {}
    def __new__(mcls, name, bases, namespace):
        print("new:" + name)
        created = type.__new__(mcls, name, bases, namespace)
        print("new-done:" + name)
        return created
    def __init__(cls, name, bases, namespace):
        print("init:" + name)
        type.__init__(cls, name, bases, namespace)
class Descriptor:
    def __set_name__(self, owner, name):
        print("set:" + name)
class Base:
    def __init_subclass__(cls):
        print("base:" + str(cls().owner() is cls))
class Visible(Base, metaclass=HookMeta):
    field = Descriptor()
    def owner(self):
        return __class__
print(type(Visible) is HookMeta)
print(Visible().owner() is Visible)
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "visible_metaclass_hook_names.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("metaclass definition hooks should run from source");
        assert_eq!(
            captured.lines().collect::<Vec<_>>(),
            [
                "prepare:Visible",
                "new:Visible",
                "set:field",
                "base:True",
                "new-done:Visible",
                "init:Visible",
                "True",
                "True"
            ]
        );
        assert!(!captured.contains("__mamba_user_class__"));
    }

    #[test]
    fn metaclass_non_type_result_flows_through_decorator_and_binding() {
        let src = r#"
def decorate(value):
    print("decorate:" + str(value))
    return value + 1

class Descriptor:
    def __set_name__(self, owner, name):
        print("set:" + name)

class Base:
    def __init_subclass__(cls):
        print("base")

class ValueMeta(type):
    def __new__(mcls, name, bases, namespace):
        print("new:" + name)
        return 41
    def __init__(cls, name, bases, namespace):
        print("init:" + name)

@decorate
class Value(Base, metaclass=ValueMeta):
    field = Descriptor()

print(Value)
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "metaclass_non_type_result.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("the metaclass result must become the class statement value");
        assert_eq!(
            captured.lines().collect::<Vec<_>>(),
            ["new:Value", "decorate:41", "42"]
        );
    }

    #[test]
    fn default_type_fills_classcell_before_init_subclass() {
        let src = r#"
class Base:
    def __init_subclass__(cls):
        print(cls().owner() is cls)

class Child(Base):
    def owner(self):
        return __class__
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "default_type_classcell_timing.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("default type must fill __class__ before __init_subclass__");
        assert_eq!(captured.trim(), "True");
    }

    #[test]
    fn nondelegating_metaclass_does_not_prefill_classcell() {
        let src = r#"
class ValueMeta(type):
    def __new__(mcls, name, bases, namespace):
        return 41

class Value(metaclass=ValueMeta):
    def owner(self):
        return __class__
"#;
        let mut session = CompilerSession::new(CompilerConfig::default());
        let error = session
            .run_source(src, "nondelegating_metaclass_classcell.py")
            .expect_err("a metaclass that skips type.__new__ must not fill __class__");
        crate::runtime::cleanup_all_runtime_state();

        assert!(
            error.to_string().contains("__class__ not set defining 'Value'"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn metaclass_may_remove_classcell_after_type_new() {
        let src = r#"
class ValueMeta(type):
    def __new__(mcls, name, bases, namespace):
        created = type.__new__(mcls, name, bases, namespace)
        namespace.pop("__classcell__", None)
        return created

class Value(metaclass=ValueMeta):
    def owner(self):
        return __class__

print(Value().owner() is Value)
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "metaclass_removes_consumed_classcell.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("a consumed classcell need not remain in the namespace");
        assert_eq!(captured.trim(), "True");
    }

    #[test]
    fn metaclass_initializes_through_the_result_type() {
        let src = r#"
class Meta(type):
    def __new__(mcls, name, bases, namespace):
        if name == "Target":
            return Existing
        return type.__new__(mcls, name, bases, namespace)
    def __init__(cls, name, bases, namespace):
        if name == "Target":
            print("Meta:" + name)

class SubMeta(Meta):
    def __init__(cls, name, bases, namespace):
        if name == "Target":
            print("Sub:" + name)

class Existing(metaclass=SubMeta):
    pass

class Target(metaclass=Meta):
    pass

print(Target is Existing)
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "metaclass_result_type_init.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("metaclass __init__ must dispatch through type(result)");
        assert_eq!(captured.lines().collect::<Vec<_>>(), ["Sub:Target", "True"]);
    }

    #[test]
    fn type_new_rejects_a_non_cell_classcell() {
        let src = r#"
class ValueMeta(type):
    def __new__(mcls, name, bases, namespace):
        namespace["__classcell__"] = 42
        return type.__new__(mcls, name, bases, namespace)

class Value(metaclass=ValueMeta):
    pass
"#;
        let mut session = CompilerSession::new(CompilerConfig::default());
        let error = session
            .run_source(src, "metaclass_invalid_classcell.py")
            .expect_err("type.__new__ must reject a non-cell __classcell__");
        crate::runtime::cleanup_all_runtime_state();

        assert!(
            error.to_string().contains("__classcell__ must be a nonlocal cell"),
            "unexpected error: {error}"
        );
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_feature_links_mambalibs_http() {
        let module = mamba_registry::find_module("mambalibs.http")
            .expect("native-modules must link mambalibs.http");
        let mut registrar = mamba_registry::ModuleRegistrar::new();
        module.register(&mut registrar);
        let symbols: std::collections::HashSet<&str> =
            registrar.symbols().iter().map(|sym| sym.name).collect();
        let values: std::collections::HashSet<&str> =
            registrar.values().iter().map(|value| value.name).collect();
        assert!(symbols.contains("App"));
        assert!(symbols.contains("Router"));
        assert!(symbols.contains("Endpoint"));
        assert!(symbols.contains("_httpkit_app_add_endpoint"));
        assert!(symbols.contains("_httpkit_app_endpoint_count"));
        assert!(symbols.contains("HTTPException"));
        assert!(values.contains("HTTPStatus"));
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_http_constructor_uses_native_abi() {
        use crate::runtime::rc::MbObject;
        use crate::runtime::value::MbValue;

        crate::runtime::registry_bridge::install();
        crate::runtime::module::mb_register_native_modules();
        let ctor = crate::runtime::module::mb_module_getattr(
            MbValue::from_ptr(MbObject::new_str("mambalibs.http".to_string())),
            MbValue::from_ptr(MbObject::new_str("App".to_string())),
        );
        assert!(
            ctor.as_func().is_some(),
            "App must import as a function pointer"
        );

        let app = crate::runtime::class::mb_call0(ctor);
        let app_reg = mamba_registry::MbValue::from_bits(app.to_bits());
        assert_eq!(
            mamba_registry::convert::native_type_name(app_reg),
            Some("App")
        );
        crate::runtime::cleanup_all_runtime_state();
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_route_decorator_source_registers_endpoint() {
        let src = r#"
from mambalibs.http import App
app = App()
@app.get("/health")
def health():
    return "ok"
print(app.endpoint_count)
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "route_decorator_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("route decorator syntax should run from source");
        assert_eq!(captured.trim(), "1");
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_dotted_http_import_source_registers_endpoint() {
        let src = r#"
import mambalibs.http
app = mambalibs.http.App()
@app.get("/health")
def health():
    return "ok"
print(app.endpoint_count)
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "dotted_route_decorator_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("dotted mambalibs.http import should run from source");
        assert_eq!(captured.trim(), "1");
    }

    #[test]
    fn dotted_class_pattern_runs_from_source() {
        let src = r#"
import datetime
subject = datetime.date(2024, 1, 2)
match subject:
    case datetime.date():
        print("matched")
    case _:
        print("missed")
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "dotted_class_pattern.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("dotted class pattern should resolve from its module binding");
        assert_eq!(captured.trim(), "matched");
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_feature_links_mambalibs_di() {
        let module = mamba_registry::find_module("mambalibs.di")
            .expect("native-modules must link mambalibs.di");
        let mut registrar = mamba_registry::ModuleRegistrar::new();
        module.register(&mut registrar);
        let symbols: std::collections::HashSet<&str> =
            registrar.symbols().iter().map(|sym| sym.name).collect();
        assert!(symbols.contains("Container"));
        assert!(symbols.contains("RequestScope"));
        assert!(symbols.contains("Depends"));
        assert!(symbols.contains("container_resolve"));
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_feature_links_mambalibs_dataclasses() {
        let module = mamba_registry::find_module("mambalibs.dataclasses")
            .expect("native-modules must link mambalibs.dataclasses");
        let mut registrar = mamba_registry::ModuleRegistrar::new();
        module.register(&mut registrar);
        let symbols: std::collections::HashSet<&str> =
            registrar.symbols().iter().map(|sym| sym.name).collect();
        assert!(symbols.contains("BaseModel"));
        assert!(symbols.contains("DataClass"));
        assert!(symbols.contains("Field"));
        assert!(symbols.contains("create_model"));
        assert!(symbols.contains("add_fields"));
        assert!(symbols.contains("validate"));
        assert!(symbols.contains("model_validate"));
        assert!(symbols.contains("model_validate_json"));
        assert!(symbols.contains("parse_raw"));
        assert!(symbols.contains("model_dump"));
        assert!(symbols.contains("model_dump_json"));
        assert!(symbols.contains("model_json_schema"));
        assert!(symbols.contains("to_json_schema"));
        assert!(
            mamba_registry::find_module("cclab_schema_mamba").is_some(),
            "legacy schema module alias must remain registered"
        );
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_dataclasses_model_methods_source() {
        let src = r#"
from mambalibs.dataclasses import BaseModel, Field
User = BaseModel("User")
User.add_field(Field("name", {"min_length": 3}))
print(User.validate({"name": "alice"}))
print(User.to_json_schema())
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "dataclasses_model_methods_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("dataclasses model methods should run from source");
        let mut lines = captured.lines();
        assert_eq!(lines.next(), Some("True"));
        let schema = lines.next().unwrap_or_default();
        assert!(
            schema.contains("\"name\""),
            "schema should contain registered field: {schema}"
        );
        assert!(
            schema.contains("\"minLength\":3"),
            "schema should include Field min_length: {schema}"
        );
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_http_di_dataclasses_openapi_source() {
        let src = r#"
from mambalibs.http import FastAPI, Depends
from mambalibs.dataclasses import BaseModel, Field

app = FastAPI({"title": "Inventory", "version": "1.2.3"})
ItemCreate = BaseModel("ItemCreate")
ItemCreate.add_field(Field("name", {"min_length": 3}))
ItemRead = BaseModel("ItemRead")
ItemRead.add_field(Field("name", {"min_length": 3}))

@app.post("/items", status_code=201, dependencies=[Depends("current_user")], request_model=ItemCreate, response_model=ItemRead)
def create_item():
    return "ok"

print(app.endpoint_count)
print(app.openapi())
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "http_di_dataclasses_openapi_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("HTTP, DI, and dataclasses route metadata should run from source");
        let mut lines = captured.lines();
        assert_eq!(lines.next(), Some("1"));
        let openapi = lines.next().unwrap_or_default();
        for expected in [
            "\"/items\"",
            "\"post\"",
            "\"201\"",
            "current_user",
            "#/components/schemas/ItemCreate",
            "#/components/schemas/ItemRead",
        ] {
            assert!(
                openapi.contains(expected),
                "openapi output should contain {expected}: {openapi}"
            );
        }
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_typed_dataclasses_openapi_source() {
        let src = r#"
from mambalibs.http import FastAPI, Depends
from mambalibs.dataclasses import BaseModel, Field

app = FastAPI({"title": "Inventory", "version": "2.0.0"})
ItemCreate = BaseModel("ItemCreate")
ItemCreate.add_field(Field("name", {"type": "str", "min_length": 3}))
ItemCreate.add_field(Field("age", {"type": "int", "minimum": 1}))
ItemCreate.add_field(Field("active", {"type": "bool", "default": True}))
ItemCreate.add_field(Field("tags", {"type": "list[str]", "default": []}))

@app.post("/items", status_code=201, dependencies=[Depends("current_user")], request_model=ItemCreate, response_model=ItemCreate)
def create_item():
    return "ok"

print(ItemCreate.validate({"name": "alice", "age": 2, "active": False, "tags": ["api", "schema"]}))
print(ItemCreate.validate({"name": "al", "age": "two", "tags": [1]}))
print(ItemCreate.to_json_schema())
print(app.openapi())
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "typed_dataclasses_openapi_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("typed dataclasses should validate and feed OpenAPI from source");
        let mut lines = captured.lines();
        assert_eq!(lines.next(), Some("True"));
        let invalid = lines.next().unwrap_or_default();
        assert!(
            invalid.contains("ValidationError"),
            "invalid typed data should return ValidationError: {invalid}"
        );
        let schema = lines.next().unwrap_or_default();
        let schema_doc: serde_json::Value =
            serde_json::from_str(schema).expect("schema output should be JSON");
        assert_eq!(schema_doc["title"].as_str(), Some("ItemCreate"));
        assert_eq!(
            schema_doc["properties"]["age"]["type"].as_str(),
            Some("integer")
        );
        assert_eq!(schema_doc["properties"]["age"]["minimum"].as_i64(), Some(1));
        assert_eq!(
            schema_doc["properties"]["active"]["type"].as_str(),
            Some("boolean")
        );
        assert_eq!(
            schema_doc["properties"]["active"]["default"].as_bool(),
            Some(true)
        );
        assert_eq!(
            schema_doc["properties"]["tags"]["type"].as_str(),
            Some("array")
        );
        assert_eq!(
            schema_doc["properties"]["tags"]["items"]["type"].as_str(),
            Some("string")
        );
        assert!(schema_doc["properties"]["tags"]["default"]
            .as_array()
            .is_some_and(|items| items.is_empty()));
        assert_eq!(
            schema_doc["required"].as_array().map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
            Some(vec!["age", "name"])
        );
        let openapi = lines.next().unwrap_or_default();
        let openapi_doc: serde_json::Value =
            serde_json::from_str(openapi).expect("openapi output should be JSON");
        assert_eq!(
            openapi_doc["paths"]["/items"]["post"]["requestBody"]["content"]["application/json"]
                ["schema"]["$ref"]
                .as_str(),
            Some("#/components/schemas/ItemCreate")
        );
        assert_eq!(
            openapi_doc["components"]["schemas"]["ItemCreate"]["properties"]["tags"]["items"]
                ["type"]
                .as_str(),
            Some("string")
        );
        assert_eq!(
            openapi_doc["components"]["schemas"]["ItemCreate"]["properties"]["active"]["default"]
                .as_bool(),
            Some(true)
        );
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_dataclasses_model_parse_dump_source() {
        let src = r#"
from mambalibs.dataclasses import BaseModel, Field

ItemCreate = BaseModel("ItemCreate")
ItemCreate.add_field(Field("name", {"type": "str", "min_length": 3}))
ItemCreate.add_field(Field("age", {"type": "int", "minimum": 1}))
ItemCreate.add_field(Field("active", {"type": "bool", "default": True}))
ItemCreate.add_field(Field("tags", {"type": "list[str]", "default": []}))

print(ItemCreate.validate({"name": "alice", "age": "2"}))
print(ItemCreate.model_dump_json({"name": "alice", "age": "2"}))
print(ItemCreate.model_validate({"name": "alice", "age": "2"}, {"strict": True}))
print(ItemCreate.model_json_schema())
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "dataclasses_model_parse_dump_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("dataclasses model parse/dump helpers should run from source");
        let mut lines = captured.lines();
        let legacy_validate = lines.next().unwrap_or_default();
        assert!(
            legacy_validate.contains("ValidationError"),
            "existing validate should remain strict bool/string behavior: {legacy_validate}"
        );

        let dump_json = lines.next().unwrap_or_default();
        let dump_doc: serde_json::Value =
            serde_json::from_str(dump_json).expect("model_dump_json output should be JSON");
        assert_eq!(dump_doc["name"].as_str(), Some("alice"));
        assert_eq!(dump_doc["age"].as_i64(), Some(2));
        assert_eq!(dump_doc["active"].as_bool(), Some(true));
        assert!(dump_doc["tags"]
            .as_array()
            .is_some_and(|items| items.is_empty()));

        let strict_error = lines.next().unwrap_or_default();
        assert!(
            strict_error.contains("ValidationError"),
            "strict model_validate should reject string age: {strict_error}"
        );

        let schema = lines.next().unwrap_or_default();
        let schema_doc: serde_json::Value =
            serde_json::from_str(schema).expect("model_json_schema output should be JSON");
        assert_eq!(schema_doc["title"].as_str(), Some("ItemCreate"));
        assert_eq!(
            schema_doc["properties"]["age"]["type"].as_str(),
            Some("integer")
        );
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_dataclasses_json_validation_source() {
        let src = r#"
from mambalibs.dataclasses import BaseModel, Field

Item = BaseModel("Item")
Item.add_field(Field("name", {"type": "str", "min_length": 3}))
Item.add_field(Field("age", {"type": "int", "minimum": 1}))
Item.add_field(Field("active", {"type": "bool", "default": True}))

print(Item.model_validate_json('{"name":"alice","age":"2"}'))
print(Item.parse_raw('{"name":"bob","age":"3"}'))
print(Item.model_validate_json('{"name":"alice","age":}'))
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "dataclasses_json_validation_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("dataclasses JSON validation should run from source");
        let mut lines = captured.lines();
        let validated = lines.next().unwrap_or_default();
        assert!(
            validated.contains("'age': 2") || validated.contains("\"age\": 2"),
            "model_validate_json should coerce age: {validated}"
        );
        assert!(
            validated.contains("'active': True") || validated.contains("\"active\": true"),
            "model_validate_json should apply defaults: {validated}"
        );

        let parsed = lines.next().unwrap_or_default();
        assert!(
            parsed.contains("'age': 3") || parsed.contains("\"age\": 3"),
            "parse_raw should alias JSON validation: {parsed}"
        );

        let invalid = lines.next().unwrap_or_default();
        assert!(
            invalid.contains("ValidationError: invalid JSON"),
            "invalid JSON should report ValidationError: {invalid}"
        );
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_dataclasses_create_model_source() {
        let src = r#"
from mambalibs.dataclasses import BaseModel, Field, create_model

User = create_model("User", {
    "name": {"type": "str", "min_length": 3},
    "age": Field({"type": "int", "minimum": 1}),
    "active": "bool",
})

Profile = BaseModel("Profile")
Profile.add_fields({
    "owner": User,
    "tags": {"type": "list[str]", "default": []},
})

print(User.model_dump_json({"name": "alice", "age": "2", "active": True}))
print(Profile.model_dump_json({"owner": {"name": "bob", "age": "3", "active": False}}))
print(Profile.model_json_schema())
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "dataclasses_create_model_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("dataclasses create_model should run from source");
        let mut lines = captured.lines();
        let user_json = lines.next().unwrap_or_default();
        let user_doc: serde_json::Value =
            serde_json::from_str(user_json).expect("user dump should be JSON");
        assert_eq!(user_doc["age"].as_i64(), Some(2));
        assert_eq!(user_doc["active"].as_bool(), Some(true));

        let profile_json = lines.next().unwrap_or_default();
        let profile_doc: serde_json::Value =
            serde_json::from_str(profile_json).expect("profile dump should be JSON");
        assert_eq!(profile_doc["owner"]["name"].as_str(), Some("bob"));
        assert_eq!(profile_doc["owner"]["age"].as_i64(), Some(3));
        assert!(profile_doc["tags"]
            .as_array()
            .is_some_and(|items| items.is_empty()));

        let schema = lines.next().unwrap_or_default();
        let schema_doc: serde_json::Value =
            serde_json::from_str(schema).expect("model_json_schema output should be JSON");
        assert_eq!(
            schema_doc["properties"]["owner"]["type"].as_str(),
            Some("object")
        );
        assert_eq!(
            schema_doc["properties"]["tags"]["items"]["type"].as_str(),
            Some("string")
        );
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_dataclasses_field_aliases_constraints_source() {
        let src = r#"
from mambalibs.dataclasses import BaseModel, Field

Account = BaseModel("Account")
Account.add_field(Field("id", {"type": "int", "alias": "userId", "serialization_alias": "user_id", "gt": 0, "lt": 10}))
Account.add_field(Field("code", {"type": "str", "validation_alias": "accountCode", "regex": "^[A-Z]{2}$"}))

print(Account.model_dump_json({"userId": "2", "accountCode": "AB"}, {"by_alias": True}))
print(Account.model_dump_json({"userId": 0, "accountCode": "AB"}))
print(Account.model_json_schema())
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "dataclasses_field_aliases_constraints_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("dataclasses alias and constraint extensions should run from source");
        let mut lines = captured.lines();
        let dump_json = lines.next().unwrap_or_default();
        let dump_doc: serde_json::Value =
            serde_json::from_str(dump_json).expect("model_dump_json output should be JSON");
        assert_eq!(dump_doc["user_id"].as_i64(), Some(2));
        assert_eq!(dump_doc["code"].as_str(), Some("AB"));

        let invalid = lines.next().unwrap_or_default();
        assert!(
            invalid.contains("ValidationError"),
            "exclusive boundary should be rejected: {invalid}"
        );

        let schema = lines.next().unwrap_or_default();
        let schema_doc: serde_json::Value =
            serde_json::from_str(schema).expect("model_json_schema output should be JSON");
        assert_eq!(
            schema_doc["properties"]["user_id"]["exclusiveMinimum"].as_i64(),
            Some(0)
        );
        assert_eq!(
            schema_doc["properties"]["user_id"]["exclusiveMaximum"].as_i64(),
            Some(10)
        );
        assert_eq!(
            schema_doc["properties"]["code"]["pattern"].as_str(),
            Some("^[A-Z]{2}$")
        );
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_dataclasses_field_schema_metadata_source() {
        let src = r#"
from mambalibs.dataclasses import BaseModel, Field

CatalogItem = BaseModel("CatalogItem")
CatalogItem.add_field(Field("name", {"type": "str", "title": "Display Name", "description": "Public catalog name", "examples": ["widget", "gadget"], "deprecated": True, "read_only": True}))
CatalogItem.add_field(Field("quantity", {"type": "int", "minimum": 0, "multiple_of": 5}))
CatalogItem.add_field(Field("tags", {"type": "list[str]", "min_length": 1, "max_length": 2, "writeOnly": True}))

print(CatalogItem.model_dump_json({"name": "widget", "quantity": 10, "tags": ["new"]}))
print(CatalogItem.model_dump_json({"name": "widget", "quantity": 7, "tags": ["new"]}))
print(CatalogItem.model_dump_json({"name": "widget", "quantity": 10, "tags": []}))
print(CatalogItem.model_json_schema())
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "dataclasses_field_schema_metadata_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("dataclasses Field schema metadata should run from source");
        let mut lines = captured.lines();
        let dump_json = lines.next().unwrap_or_default();
        let dump_doc: serde_json::Value =
            serde_json::from_str(dump_json).expect("model_dump_json output should be JSON");
        assert_eq!(dump_doc["quantity"].as_i64(), Some(10));

        let invalid_multiple = lines.next().unwrap_or_default();
        assert!(
            invalid_multiple.contains("ValidationError"),
            "multiple_of should be validated: {invalid_multiple}"
        );
        let invalid_tags = lines.next().unwrap_or_default();
        assert!(
            invalid_tags.contains("ValidationError"),
            "list min_length alias should be validated: {invalid_tags}"
        );

        let schema = lines.next().unwrap_or_default();
        let schema_doc: serde_json::Value =
            serde_json::from_str(schema).expect("model_json_schema output should be JSON");
        assert_eq!(
            schema_doc["properties"]["name"]["title"].as_str(),
            Some("Display Name")
        );
        assert_eq!(
            schema_doc["properties"]["name"]["examples"],
            serde_json::json!(["widget", "gadget"])
        );
        assert_eq!(
            schema_doc["properties"]["name"]["deprecated"].as_bool(),
            Some(true)
        );
        assert_eq!(
            schema_doc["properties"]["name"]["readOnly"].as_bool(),
            Some(true)
        );
        assert_eq!(
            schema_doc["properties"]["quantity"]["multipleOf"].as_i64(),
            Some(5)
        );
        assert_eq!(
            schema_doc["properties"]["tags"]["minItems"].as_i64(),
            Some(1)
        );
        assert_eq!(
            schema_doc["properties"]["tags"]["maxItems"].as_i64(),
            Some(2)
        );
        assert_eq!(
            schema_doc["properties"]["tags"]["writeOnly"].as_bool(),
            Some(true)
        );
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_dataclasses_optional_nullable_source() {
        let src = r#"
from mambalibs.dataclasses import BaseModel, Field

Profile = BaseModel("Profile")
Profile.add_field(Field("nickname", {"type": "Optional[str]"}))
Profile.add_field(Field("score", {"type": "int", "nullable": True}))
Profile.add_field(Field("middle_name", {"type": "Optional[str]", "default": None}))

print(Profile.model_dump_json({"nickname": None, "score": None}))
print(Profile.model_json_schema())
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "dataclasses_optional_nullable_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("dataclasses optional/nullable extensions should run from source");
        let mut lines = captured.lines();
        let dump_json = lines.next().unwrap_or_default();
        let dump_doc: serde_json::Value =
            serde_json::from_str(dump_json).expect("model_dump_json output should be JSON");
        assert!(dump_doc["nickname"].is_null());
        assert!(dump_doc["score"].is_null());
        assert!(dump_doc["middle_name"].is_null());

        let schema = lines.next().unwrap_or_default();
        let schema_doc: serde_json::Value =
            serde_json::from_str(schema).expect("model_json_schema output should be JSON");
        assert_eq!(
            schema_doc["properties"]["nickname"]["type"],
            serde_json::json!(["string", "null"])
        );
        assert_eq!(
            schema_doc["properties"]["score"]["type"],
            serde_json::json!(["integer", "null"])
        );
        assert!(schema_doc["properties"]["middle_name"]["default"].is_null());
        assert_eq!(
            schema_doc["required"].as_array().map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
            Some(vec!["nickname", "score"])
        );
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_dataclasses_nested_models_source() {
        let src = r#"
from mambalibs.dataclasses import BaseModel, Field

User = BaseModel("User")
User.add_field(Field("name", {"type": "str", "min_length": 3}))
User.add_field(Field("age", {"type": "int", "default": 1}))
User.add_field(Field("active", {"type": "bool", "default": True}))

Team = BaseModel("Team")
Team.add_field(Field("owner", {"model": User}))
Team.add_field(Field("members", {"type": "list", "items_model": User}))

print(Team.model_dump_json({"owner": {"name": "alice"}, "members": [{"name": "bob", "age": "2"}]}))
print(Team.model_dump_json({"owner": {}, "members": [{"name": "bob"}]}))
print(Team.model_json_schema())
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "dataclasses_nested_models_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("dataclasses nested model extensions should run from source");
        let mut lines = captured.lines();
        let dump_json = lines.next().unwrap_or_default();
        let dump_doc: serde_json::Value =
            serde_json::from_str(dump_json).expect("model_dump_json output should be JSON");
        assert_eq!(dump_doc["owner"]["name"].as_str(), Some("alice"));
        assert_eq!(dump_doc["owner"]["age"].as_i64(), Some(1));
        assert_eq!(dump_doc["owner"]["active"].as_bool(), Some(true));
        assert_eq!(dump_doc["members"][0]["name"].as_str(), Some("bob"));
        assert_eq!(dump_doc["members"][0]["age"].as_i64(), Some(2));
        assert_eq!(dump_doc["members"][0]["active"].as_bool(), Some(true));

        let invalid = lines.next().unwrap_or_default();
        assert!(
            invalid.contains("ValidationError"),
            "nested model errors should surface as validation errors: {invalid}"
        );

        let schema = lines.next().unwrap_or_default();
        let schema_doc: serde_json::Value =
            serde_json::from_str(schema).expect("model_json_schema output should be JSON");
        assert_eq!(
            schema_doc["properties"]["owner"]["properties"]["name"]["type"].as_str(),
            Some("string")
        );
        assert_eq!(
            schema_doc["properties"]["members"]["items"]["properties"]["age"]["type"].as_str(),
            Some("integer")
        );
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_http_di_dataclasses_preflight_source() {
        let src = r#"
from mambalibs.http import FastAPI, Depends
from mambalibs.di import Container, container_register_value
from mambalibs.dataclasses import BaseModel, Field

app = FastAPI({"title": "Inventory"})
ItemCreate = BaseModel("ItemCreate")
ItemCreate.add_field(Field("name", {"type": "str", "min_length": 3}))
ItemCreate.add_field(Field("age", {"type": "int", "minimum": 1}))
ItemCreate.add_field(Field("tags", {"type": "list[str]", "default": []}))

@app.post("/items", status_code=201, dependencies=[Depends("current_user")], request_model=ItemCreate, response_model=ItemCreate)
def create_item():
    return "ok"

container = Container()
container_register_value(container, "current_user", "alice", "request")

print(app.preflight("POST", "/items", {"name": "alice", "age": "2"}, container))
print(app.preflight("POST", "/items", '{"name":"alice","age":"3"}', container))
print(app.preflight("POST", "/items", {"name": "al", "age": "two", "tags": [1]}, container))
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "http_di_dataclasses_preflight_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("HTTP preflight should compose DI and dataclasses from source");
        let mut lines = captured.lines();
        let valid = lines.next().unwrap_or_default();
        let valid_doc: serde_json::Value =
            serde_json::from_str(valid).expect("valid preflight output should be JSON");
        assert_eq!(valid_doc["matched"].as_bool(), Some(true));
        assert_eq!(valid_doc["status_code"].as_i64(), Some(201));
        assert_eq!(valid_doc["body"]["age"].as_i64(), Some(2));
        assert!(valid_doc["body"]["tags"]
            .as_array()
            .is_some_and(|items| items.is_empty()));
        assert_eq!(
            valid_doc["dependencies"]["current_user"].as_str(),
            Some("alice")
        );

        let raw = lines.next().unwrap_or_default();
        let raw_doc: serde_json::Value =
            serde_json::from_str(raw).expect("raw JSON preflight output should be JSON");
        assert_eq!(raw_doc["matched"].as_bool(), Some(true));
        assert_eq!(raw_doc["status_code"].as_i64(), Some(201));
        assert_eq!(raw_doc["body"]["age"].as_i64(), Some(3));
        assert!(raw_doc["body"]["tags"]
            .as_array()
            .is_some_and(|items| items.is_empty()));
        assert_eq!(
            raw_doc["dependencies"]["current_user"].as_str(),
            Some("alice")
        );

        let invalid = lines.next().unwrap_or_default();
        let invalid_doc: serde_json::Value =
            serde_json::from_str(invalid).expect("invalid preflight output should be JSON");
        assert_eq!(invalid_doc["status_code"].as_i64(), Some(422));
        assert!(invalid_doc["errors"]
            .as_array()
            .is_some_and(|errors| errors.iter().any(|error| error
                .as_str()
                .is_some_and(|msg| msg.contains("ValidationError")))));
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_http_preflight_validation_details_source() {
        let src = r#"
from mambalibs.http import FastAPI
from mambalibs.dataclasses import BaseModel, Field

app = FastAPI({"title": "Validation Detail"})
Item = BaseModel("Item")
Item.add_field(Field("name", {"type": "str", "min_length": 3}))
Item.add_field(Field("age", {"type": "int", "minimum": 1}))

@app.post("/items", request_model=Item, response_model=Item)
def create_item():
    return "ok"

print(app.preflight("POST", "/items", {"name": "al", "age": "two"}))
print(Item.model_dump_json({"name": "al", "age": "two"}))
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "http_preflight_validation_details_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("HTTP preflight should expose validation detail from source");
        let mut lines = captured.lines();
        let report = lines.next().unwrap_or_default();
        let report_doc: serde_json::Value =
            serde_json::from_str(report).expect("preflight output should be JSON");
        assert_eq!(report_doc["status_code"].as_i64(), Some(422));
        assert!(report_doc["errors"]
            .as_array()
            .is_some_and(|errors| errors.iter().any(|error| error
                .as_str()
                .is_some_and(|msg| msg.contains("ValidationError")))));
        assert!(report_doc["detail"]
            .as_array()
            .is_some_and(|details| details
                .iter()
                .any(|detail| detail["loc"] == serde_json::json!(["body", "age"])
                    && detail["type"].as_str() == Some("type_error"))));
        assert!(report_doc["detail"]
            .as_array()
            .is_some_and(|details| details.iter().any(|detail| detail["loc"]
                == serde_json::json!(["body", "name"])
                && detail["msg"]
                    .as_str()
                    .is_some_and(|msg| msg.contains("at least")))));

        let legacy = lines.next().unwrap_or_default();
        assert!(
            legacy.contains("ValidationError"),
            "existing model_dump_json string error behavior should remain: {legacy}"
        );
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_http_query_header_parameters_source() {
        let src = r#"
from mambalibs.http import FastAPI, Query, Header

app = FastAPI({"title": "Search"})

@app.get("/search", parameters=[Query(name="q"), Header("local", "X-Trace-ID")])
def search():
    return "ok"

print(app.openapi())
print(app.preflight("GET", "/search", {}, None, {"query": {"q": "mamba"}}))
print(app.preflight("GET", "/search", {}, None))
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "http_query_header_parameters_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("HTTP Query/Header parameters should work from source");
        let mut lines = captured.lines();
        let openapi = lines.next().unwrap_or_default();
        let openapi_doc: serde_json::Value =
            serde_json::from_str(openapi).expect("openapi output should be JSON");
        let params = openapi_doc["paths"]["/search"]["get"]["parameters"]
            .as_array()
            .expect("openapi parameters");
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

        let ok = lines.next().unwrap_or_default();
        let ok_doc: serde_json::Value =
            serde_json::from_str(ok).expect("valid preflight output should be JSON");
        assert_eq!(ok_doc["status_code"].as_i64(), Some(200));
        assert_eq!(ok_doc["parameters"]["q"].as_str(), Some("mamba"));
        assert_eq!(ok_doc["parameters"]["X-Trace-ID"].as_str(), Some("local"));

        let missing = lines.next().unwrap_or_default();
        let missing_doc: serde_json::Value =
            serde_json::from_str(missing).expect("missing preflight output should be JSON");
        assert_eq!(missing_doc["status_code"].as_i64(), Some(422));
        assert!(missing_doc["detail"]
            .as_array()
            .is_some_and(|details| details
                .iter()
                .any(|detail| detail["loc"] == serde_json::json!(["query", "q"])
                    && detail["type"].as_str() == Some("missing"))));
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_http_testclient_di_dataclasses_source() {
        let src = r#"
from mambalibs.http import FastAPI, Depends, TestClient
from mambalibs.di import Container, container_register_value
from mambalibs.dataclasses import BaseModel, Field

app = FastAPI({"title": "Inventory"})
ItemCreate = BaseModel("ItemCreate")
ItemCreate.add_field(Field("name", {"type": "str", "min_length": 3}))
ItemCreate.add_field(Field("age", {"type": "int", "minimum": 1}))
ItemCreate.add_field(Field("tags", {"type": "list[str]", "default": []}))

@app.post("/items", status_code=201, dependencies=[Depends("current_user")], request_model=ItemCreate, response_model=ItemCreate)
def create_item():
    return {"name": "created", "age": "2"}

container = Container()
container_register_value(container, "current_user", "alice", "request")
client = TestClient(app, container)

response = client.post("/items", {"name": "alice", "age": "2"})
print(response.status_code)
print(response.json()["name"])
print(response.json()["age"])

invalid = client.post("/items", {"name": "al", "age": "two"})
print(invalid.status_code)

missing = client.get("/missing")
print(missing.status_code)
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "http_testclient_di_dataclasses_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("HTTP TestClient should dispatch through DI and dataclasses from source");
        let mut lines = captured.lines();
        assert_eq!(lines.next(), Some("201"));
        assert_eq!(lines.next(), Some("created"));
        assert_eq!(lines.next(), Some("2"));
        assert_eq!(lines.next(), Some("422"));
        assert_eq!(lines.next(), Some("404"));
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_http_testclient_di_create_model_source() {
        let src = r#"
from mambalibs.http import FastAPI, Depends, TestClient
from mambalibs.di import Container, container_register_value
from mambalibs.dataclasses import Field, create_model

app = FastAPI({"title": "Inventory"})
ItemCreate = create_model("ItemCreate", {
    "name": {"type": "str", "min_length": 3},
    "age": Field({"type": "int", "minimum": 1}),
    "tags": {"type": "list[str]", "default": []},
})

@app.post("/items", status_code=201, dependencies=[Depends("current_user")], request_model=ItemCreate, response_model=ItemCreate)
def create_item():
    return {"name": "created", "age": "2"}

container = Container()
container_register_value(container, "current_user", "alice", "request")
client = TestClient(app, container)

response = client.post("/items", {"name": "alice", "age": "2"})
print(response.status_code)
print(response.json()["name"])
print(response.json()["age"])

invalid = client.post("/items", {"name": "al", "age": "two"})
print(invalid.status_code)
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "http_testclient_di_create_model_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("HTTP TestClient should use create_model schemas with DI");
        let mut lines = captured.lines();
        assert_eq!(lines.next(), Some("201"));
        assert_eq!(lines.next(), Some("created"));
        assert_eq!(lines.next(), Some("2"));
        assert_eq!(lines.next(), Some("422"));
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_http_run_surface_source() {
        let src = r#"
from mambalibs.http import FastAPI, run

app = FastAPI({"title": "Run Surface"})

@app.get("/health")
def health():
    return "ok"

server = app.run("127.0.0.1", 0)
print(server.url)
print(server.host)
print(server.port)
print(server.running)
print(server.endpoint_count)
server.stop()
print(server.running)

module_server = run(app, "0.0.0.0", 9000)
print(module_server.url)
"#;
        crate::runtime::registry_bridge::install();
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "http_run_surface_source.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("HTTP run surface should work from source");
        let mut lines = captured.lines();
        assert_eq!(lines.next(), Some("http://127.0.0.1:0"));
        assert_eq!(lines.next(), Some("127.0.0.1"));
        assert_eq!(lines.next(), Some("0"));
        assert_eq!(lines.next(), Some("True"));
        assert_eq!(lines.next(), Some("1"));
        assert_eq!(lines.next(), Some("False"));
        assert_eq!(lines.next(), Some("http://0.0.0.0:9000"));
    }

    #[cfg(feature = "native-modules")]
    #[test]
    fn native_modules_feature_links_pgkit() {
        let module = mamba_registry::find_module("mambalibs.pg")
            .expect("native-modules must link pgkit as mambalibs.pg");
        let mut registrar = mamba_registry::ModuleRegistrar::new();
        module.register(&mut registrar);
        let symbols: std::collections::HashSet<&str> =
            registrar.symbols().iter().map(|sym| sym.name).collect();
        assert!(symbols.contains("connect"));
        assert!(symbols.contains("execute"));
        assert!(symbols.contains("transaction_begin"));
        assert!(symbols.contains("Session"));

        let migrate = mamba_registry::find_module("mambalibs.pg.migrate")
            .expect("native-modules must link pgkit migrate as mambalibs.pg.migrate");
        let mut migrate_registrar = mamba_registry::ModuleRegistrar::new();
        migrate.register(&mut migrate_registrar);
        let migrate_symbols: std::collections::HashSet<&str> = migrate_registrar
            .symbols()
            .iter()
            .map(|sym| sym.name)
            .collect();
        assert!(migrate_symbols.contains("MigrationRunner"));
        assert!(migrate_symbols.contains("runner_status"));
    }

    // ── #1492: runtime-base slot inheritance ──────────────────────────────────

    #[test]
    fn runtime_base_slots_include_inherited_fields_before_instance_init() {
        let src = r#"
class Base:
    __slots__ = ("x",)

    def __init__(self, x):
        self.x = x


class Child(Base):
    __slots__ = ("y",)

    def class_cell(self):
        return super()

    def __init__(self, x, y):
        Base.__init__(self, x)
        self.y = y


child = Child(1, 2)
print(child.x)
print(child.y)
print(Child.__slots__)
"#;
        let previous = crate::runtime::output::begin_capture();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.run_source(src, "runtime_base_slots.py");
        let captured = crate::runtime::output::end_capture(previous);
        crate::runtime::cleanup_all_runtime_state();

        result.expect("runtime-base slot inheritance should run from source");
        // #1523: `Child.__slots__` reports the declared-own tuple ('y',), not
        // the merged instance layout ('y', 'x') — instance field access
        // (x=1, y=2, both inherited and own) must still work correctly.
        assert_eq!(captured.lines().collect::<Vec<_>>(), ["1", "2", "('y',)"]);
    }

    // ── CompilerSession::new ──────────────────────────────────────────────────

    #[test]
    fn compiler_session_new_creates_session() {
        let session = CompilerSession::new(CompilerConfig::default());
        // session created — just verify no panic
        drop(session);
    }

    // ── CompilerSession::load_file ────────────────────────────────────────────

    #[test]
    fn load_file_valid_path_ok() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), "x = 1\n").unwrap();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.load_file(file.path().to_str().unwrap());
        assert!(result.is_ok(), "valid path should succeed");
    }

    #[test]
    fn load_file_invalid_path_err() {
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.load_file("/no/such/file.py");
        assert!(result.is_err(), "invalid path should return Err");
    }

    // ── CompilerSession::check ────────────────────────────────────────────────

    #[test]
    fn check_valid_source_ok() {
        let file = tempfile::NamedTempFile::with_suffix(".py").unwrap();
        std::fs::write(file.path(), "x: int = 1\n").unwrap();
        let mut session = CompilerSession::new(CompilerConfig::default());
        let result = session.check(file.path().to_str().unwrap());
        assert!(result.is_ok(), "valid source should type-check ok");
    }

    #[test]
    fn check_emit_ast_returns_ok() {
        let file = tempfile::NamedTempFile::with_suffix(".py").unwrap();
        std::fs::write(file.path(), "x = 1\n").unwrap();
        let mut session = CompilerSession::new(CompilerConfig {
            emit: Some(EmitMode::Ast),
            ..Default::default()
        });
        let result = session.check(file.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    // ── CompilerSession::build ────────────────────────────────────────────────

    #[test]
    fn build_emit_hir_returns_empty_bytes() {
        let file = tempfile::NamedTempFile::with_suffix(".py").unwrap();
        std::fs::write(file.path(), "x = 1\n").unwrap();
        let mut session = CompilerSession::new(CompilerConfig {
            emit: Some(EmitMode::Hir),
            ..Default::default()
        });
        let result = session.build(file.path().to_str().unwrap(), None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn build_emit_mir_returns_empty_bytes() {
        let file = tempfile::NamedTempFile::with_suffix(".py").unwrap();
        std::fs::write(file.path(), "x = 1\n").unwrap();
        let mut session = CompilerSession::new(CompilerConfig {
            emit: Some(EmitMode::Mir),
            ..Default::default()
        });
        let result = session.build(file.path().to_str().unwrap(), None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn build_emit_ast_returns_empty_bytes() {
        let file = tempfile::NamedTempFile::with_suffix(".py").unwrap();
        std::fs::write(file.path(), "x = 1\n").unwrap();
        let mut session = CompilerSession::new(CompilerConfig {
            emit: Some(EmitMode::Ast),
            ..Default::default()
        });
        let result = session.build(file.path().to_str().unwrap(), None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ── check_dependencies (no imports) ──────────────────────────────────────

    #[test]
    fn check_dependencies_no_imports_no_panic() {
        let file = tempfile::NamedTempFile::with_suffix(".py").unwrap();
        std::fs::write(file.path(), "x = 1\n").unwrap();
        let session = CompilerSession::new(CompilerConfig::default());
        let mut checker = crate::types::TypeChecker::new();
        // Should complete without panic
        session.check_dependencies(file.path().to_str().unwrap(), &mut checker);
    }

    // ── #1190 R2: Driver script directory initialization ─────────────────────

    #[test]
    fn build_sets_script_dir() {
        // After build(), SCRIPT_DIR should be set to the script's parent.
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("build_test.py");
        std::fs::write(&file_path, "x = 1\n").unwrap();

        crate::runtime::module::cleanup_all_modules();
        let mut session = CompilerSession::new(CompilerConfig {
            emit: Some(EmitMode::Hir), // emit HIR to avoid full codegen
            ..Default::default()
        });
        let _ = session.build(file_path.to_str().unwrap(), None);

        crate::runtime::module::SCRIPT_DIR.with(|sd| {
            let dir = sd.borrow();
            assert!(dir.is_some(), "SCRIPT_DIR should be set after build()");
        });
        crate::runtime::module::cleanup_all_modules();
    }

    #[test]
    fn build_adds_script_dir_to_search_paths() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("search_test.py");
        std::fs::write(&file_path, "x = 1\n").unwrap();

        crate::runtime::module::cleanup_all_modules();
        let mut session = CompilerSession::new(CompilerConfig {
            emit: Some(EmitMode::Hir),
            ..Default::default()
        });
        let _ = session.build(file_path.to_str().unwrap(), None);

        crate::runtime::module::SEARCH_PATHS.with(|sp| {
            let paths = sp.borrow();
            // The script's parent directory should be at position 0.
            assert!(!paths.is_empty(), "SEARCH_PATHS should not be empty");
            let canon = std::fs::canonicalize(dir.path()).unwrap_or(dir.path().to_path_buf());
            let first_path = &paths[0];
            assert_eq!(
                first_path.display().to_string(),
                canon.display().to_string(),
                "SEARCH_PATHS[0] should be the script's parent dir"
            );
        });
        crate::runtime::module::cleanup_all_modules();
    }

    #[test]
    fn build_calls_init_search_paths() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("pythonpath_test.py");
        std::fs::write(&file_path, "x = 1\n").unwrap();

        crate::runtime::module::cleanup_all_modules();
        std::env::set_var("PYTHONPATH", "/test/driver/pythonpath");

        let mut session = CompilerSession::new(CompilerConfig {
            emit: Some(EmitMode::Hir),
            ..Default::default()
        });
        let _ = session.build(file_path.to_str().unwrap(), None);

        crate::runtime::module::SEARCH_PATHS.with(|sp| {
            let paths = sp.borrow();
            let path_strs: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
            assert!(
                path_strs.contains(&"/test/driver/pythonpath".to_string()),
                "SEARCH_PATHS should contain PYTHONPATH entry after build(), got: {:?}",
                path_strs
            );
        });

        std::env::remove_var("PYTHONPATH");
        crate::runtime::module::cleanup_all_modules();
    }
}

// ── register_external_modules ─────────────────────────────────────────────────

/// Collect all symbols from every crate registered via `MAMBA_MODULES`.
///
/// Returns a `Vec` of `(name, raw_ptr)` pairs ready to be injected into a
/// Cranelift `JITBuilder` via [`CraneliftJitBackend::new_with_externals`].
///
/// When `project_config` is `None` (single-file mode) an empty vec is
/// returned — no external wiring happens.
pub fn register_external_modules(
    project_config: Option<&MambaConfig>,
) -> Vec<(&'static str, *const u8)> {
    if project_config.is_none() {
        return Vec::new();
    }
    use mamba_registry::{all_modules, ModuleRegistrar};

    let mut out: Vec<(&'static str, *const u8)> = Vec::new();
    for module in all_modules() {
        let mut registrar = ModuleRegistrar::new();
        module.register(&mut registrar);
        for sym in registrar.into_symbols() {
            out.push((sym.ffi_name, sym.func_ptr as *const u8));
        }
    }
    out
}

// HANDWRITE-BEGIN gap="standardize:projects-mamba-src-driver-mod-rs" tracker="standardize-gap-projects-mamba-src-driver-mod-rs" reason="introspection-builtins (issue: enhancement-mamba-introspection-builtins-globals-locals-vars-dir)."
/// Collect compiled function pointers for user-defined functions so they can
/// surface in `globals()`. Mirrors the same loop in `module.rs::execute_module`.
/// @spec .aw/tech-design/cclab-mamba/logic/introspection-builtins.md#globals_impl
fn collect_user_func_addrs(
    hir: &crate::hir::HirModule,
    checker: &crate::types::TypeChecker,
    backend: &crate::codegen::cranelift::jit::CraneliftJitBackend,
) -> Vec<(u32, String, *const u8)> {
    let mut sym_names: std::collections::HashMap<crate::resolve::SymbolId, String> =
        std::collections::HashMap::new();
    for sym_info in checker.symbols.all_symbols() {
        sym_names.insert(sym_info.id, sym_info.name.clone());
    }
    for (id, name) in &hir.sym_names {
        sym_names.insert(*id, name.clone());
    }

    let mut out: Vec<(u32, String, *const u8)> = Vec::new();
    for f in &hir.functions {
        if let Some(name) = sym_names.get(&f.name) {
            if let Some(ptr) = backend.get_func_ptr(f.name.0) {
                out.push((f.name.0, name.clone(), ptr));
            }
        }
    }
    out
}
// HANDWRITE-END

#[cfg(test)]
#[path = "tests"]
mod pipeline_tests {
    mod behavioral_builtins;
    mod behavioral_lang;
    mod behavioral_stdlib;
    mod codegen;
    mod gen_thread_pool;
    mod generator_conformance;
    mod iterator_conformance;
    mod jit;
    mod jit_refcount;
    mod no_arg_constructor;
    mod p0_conformance;
    mod perf_benchmark;
    mod perf_comparison;
    mod pipeline;
    mod runtime_bugs_conformance;
    mod strict_type_dynamic_ingress;
    mod xfail_zero_conformance;
}
