# Jet

## Brief

Jet is a Rust-native frontend toolchain. Its Basic track replaces the usual
frontend stack around package management, builds, dev/prod serving, native
tests, product-flow e2e, browser automation, trace, and parity evidence. Its
Advanced track sinks frontend execution into Rust/WASM and renders through
canvas/WebGPU while preserving browser-observable behavior through Jet bridges.

Primary command: `jet <command>`. In integrated cclab environments,
`cclab jet <command>` may also be available.

Agent model:

| Track | Product role | Current read |
|---|---|---|
| Basic: FE-on-DOM | Replace today's Vite/Turbopack + pnpm/npm/Bun + Playwright-style toolchain. | Green for the current local Basic gate across package manager, Browser Bridge, production build, serve, workspace, test, e2e, and trace. |
| Advanced: FE-on-WASM | Run the frontend app model inside Jet's Rust/WASM runtime and render through WebGPU/canvas. | Yellow overall. Focused evidence exists, but broad DOM-vs-WASM parity is not production-ready. |

Readiness rules for agents:

- Treat Basic and Advanced as separate readiness tracks.
- Do not use Advanced WASM progress to qualify Basic production readiness.
- Nothing validates the capability structure below. The `aw` gate that read
  README shape and TD refs was deleted with the binary, and it was never
  runtime proof even while it ran.
- Basic gates compare function first and performance second. A faster run is
  still red when output, install tree, server behavior, browser action, or trace
  evidence differs from the oracle.
- Jet-owned fixture hydration stays separate from npm/pnpm/Playwright
  comparator evidence.

Common surfaces:

| Surface | Commands | Owns |
|---|---|---|
| Package management | `jet install`, `jet add`, `jet remove`, `jet update`, `jet audit`, `jet run`, `jet exec` | Dependency lifecycle, lockfile, workspace, registry/cache, bin scripts, lifecycle hooks. |
| Build | `jet build`, `jet build --wasm` | DOM artifacts, WASM artifacts, static assets, build metadata, target manifest. |
| Dev and serve | `jet dev`, `jet dev --proxy PATH=URL`, `jet serve`, `jet serve --wasm` | HMR/dev control plane, detached sessions, production static origin serving. |
| Browser Bridge | `jet bb ...`, `jet bb mcp`; legacy `jet browser ...` | Browser automation, semantic snapshots, ref-based actions, console/network observability, DOM/WASM capture. |
| Test/e2e/trace | `jet test`, `jet e2e`, `jet trace` | Native TS tests, product-flow e2e, replayable diagnostics, parity evidence. |

First commands:

```bash
jet install
jet build
jet dev -p 3000
jet serve
jet bb launch <url>
jet bb snapshot
jet test
jet e2e
jet trace
```

Primary verification:

```bash
apps/jet/scripts/verify-basic-dom-gates.sh
JET_BASIC_DOM_BUILD_SAMPLES=3 JET_BASIC_DOM_RUNTIME_SMOKE=required apps/jet/scripts/verify-basic-dom-gates.sh --all
apps/jet/scripts/verify-advanced-wasm-gates.sh
```

Source map:

| Path | Read when |
|---|---|
| `apps/jet/docs/architecture/layout.md` | You need the repo map before editing. |
| `apps/jet/src/pkg_manager/` | Package manager behavior, lockfile, registry, store, workspace, audit/publish flows. |
| `apps/jet/src/bundler/` | Dependency graph, tree shaking, CSS bundle, minification, splitting, sourcemaps. |
| `apps/jet/src/dev_server/` | Dev server, HMR, proxy, prod static serving, watcher, prebundle/import map behavior. |
| `apps/jet/src/browser/` and `apps/jet/src/browser_cli/` | Browser Bridge driver and CLI surfaces. |
| `apps/jet/src/test_runner/`, `apps/jet/src/e2e/`, `apps/jet/src/trace/` | Native test runtime, product-flow e2e, trace artifacts. |
| `apps/jet/src/wasm_build/` and `apps/jet/wasm/` | FE-on-WASM build path and runtime crate. |
| `apps/jet/parity/` | DOM/WASM parity corpus, oracle, gates, fixtures, schemas, ADRs. |
| `apps/jet/tests/` | Product and subsystem gates. |
| `.aw/tech-design/projects/jet/specs/3779.md` | Package-manager capability/spec entrypoint. |
| `.aw/tech-design/projects/jet/logic/pkg-manager.md` | Package-manager semantic/logic entrypoint. |

## Capabilities

A promise with no gate under it is not claimed.

Nothing reads the tables below. The capability gate that validated their
shape was deleted with the `aw` binary, so the shape is convention now and
the commands named in each row are the only part that runs.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Rust-Native Frontend Toolchain Replacement | #3778 | The Basic all-in-one replacement gate is green across package manager, Browser Bridge, production build, serve, workspace, test, e2e, and trace. |
| Package Manager | #3779 | Jet owns fixture hydration and mutation gates; required isolated npm/pnpm benchmark evidence is green for the current Basic corpus. |
| Bundler And Production Build | #3782 | The expanded DOM production build corpus is green with required runtime smoke and Vite/Webpack comparisons. |
| Dev Server And HMR | #3780 | `jet dev` can replace Vite-style local development serving, HMR, browser log intake, and local API/WebSocket proxying for real projects. |
| Workspace And Task Runner | #3781 | Jet can replace npm scripts, pnpm workspaces, and common Nx/Turborepo task-runner flows through the canonical `jet.toml` configuration surface. |
| Native Test And Product-Flow E2E | #3785 | Jet native runner, reporter, product-flow e2e, and trace gates are green for the Basic production-readiness contract. |
| WASM And Multi-Target Execution | #818 | Jet can sink the frontend app model into WASM, render it through canvas/WebGPU, and preserve browser-observable semantics through bridges. |
| Browser, Trace, And Parity Infrastructure | #3786 | Jet BB is the executor for current gates, with isolated Playwright baseline evidence and trace substrate tests green. |
| Library Build And Package Publishing | #168 | `jet build --lib` (ESM+CJS, externalized deps/peerDeps, multi-entry), preserve-modules ESM/CJS output, `.d.ts` emission, and `jet publish --build` with metadata validation + private-registry (`.npmrc` scoped) e2e all shipped and tested (A1-A3 merged). `partial`: IIFE lib output, class-member `.d.ts` reduction, and some CJS re-export edge cases are TODO follow-ups. |
| Component Workbench (Stories) | #169 | Full Storybook-replacement parity from epic #1001 (#981, #987-#1000, landed via PR #1070, evidence closed by #1343): CSF3/CSF2 discovery with decorators/parameters/globals/globalTypes/loaders/autodocs (`csf.rs::tests::parses_render_path_core_fields`); a native manager with controls, toolbar, measure/outline/highlight, actions, interactions/`play()`, a11y audit, story source, search, and theming (`stories_build.rs::static_manager_keeps_dev_feature_parity_checklist`, `manager.rs::tests::manager_toolbar_renders_viewport_background_zoom_and_custom_parameters`); MDX autodocs pages compiled dev + static (`mdx.rs::tests::compiles_core_doc_blocks`, `stories_build.rs::mdx_docs_pages_render_core_blocks_in_static_export`); a static `jet stories build` export with an `index.json` manifest; and a headless-Chromium `play()` interactions runner via `jet test --stories` (`cli.rs::run_stories_smoke_tests`). CSF-compatible, no Storybook runtime. |
| Jet Project Architecture And Authoring Clarity | #1169 | Scoped layout/navigation guidance lives at `projects/jet/docs/architecture/layout.md` (path-role map + crate/package naming conventions), linked from this README's Source map; no project-root uppercase meta doc remains for Jet layout guidance. |
| Jet Agent-Facing CLI Standard Commands | #928 | `jet llm` / `jet upgrade` / `jet issue {search,view,create,comment}` are wired to the shared `cli-std` crate; `jet issue comment <n> [message...]` reopens a closed issue before posting a diagnostics-rich follow-up comment, with `--dry-run` proving the request without a network mutation. |

### Rust-Native Frontend Toolchain Replacement

Jet is gated as an all-in-one Basic frontend replacement in dependency order:
package manager, Browser Bridge, production build, serve, workspace, test, e2e,
trace, and stack-aware API client codegen. `jet codegen openapi` resolves
generated output from CLI flags, `jet.toml` `[codegen.openapi]`, and
`package.json` dependencies so the hook runtime (React Query or SWR) and
fetch/axios runtime selection match the project tech stack. See
`docs/openapi-codegen.md` for the hook/runtime matrix, axios version support,
and injecting a pre-configured `AxiosInstance`. The current
production-readiness gate is green.

- Root WI: #3778
- Surfaces: CLI: `jet install` + `jet build` + `jet dev` + `jet test` -
  Aggregate frontend toolchain entrypoints for package, build, dev, and test
  workflows.
- Gate — behavior: `apps/jet/scripts/verify-basic-dom-gates.sh --all` - Basic
  frontend replacement flow across package, build, dev, serve, workspace, test,
  e2e, and trace gates.
- Gate: `apps/jet/scripts/verify-basic-dom-gates.sh --all`
- Gate: `cargo test -p jet --test openapi_golden`
- Source: `apps/jet/tests/fixtures/dom-production-build`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Production replacement readiness | epic | #3778 | package manager -> Browser Bridge -> build -> serve/workspace/test/e2e/trace are green in `apps/jet/scripts/verify-basic-dom-gates.sh --all` |
| Full Toolchain Dogfood Flow | epic | #3778 | `apps/jet/scripts/verify-basic-dom-gates.sh --all`<br>apps/jet/tests/fixtures/dom-production-build |
| Stack-Aware OpenAPI Codegen | change | #3778 | `cargo test -p jet --test openapi_golden` — jet codegen openapi resolves stack/http/hooks from CLI flags, jet.toml, and package.json |

### Package Manager

Jet owns fixture hydration, mutation, workspace, and frozen-lockfile checks;
isolated npm/pnpm benchmark evidence is green for the current Basic corpus.

- Root WI: #3779
- Surfaces: CLI: `jet install` + `jet add` + `jet remove` + `jet update` -
  Package lifecycle commands that own dependency and lockfile behavior.
- Gate — behavior: `cargo test -p jet --lib --test behavior_package_manager_lockfile_parity --test behavior_package_manager_registry_integrity --test behavior_package_manager_workspace_parity -- --nocapture` -
  Package lifecycle, lockfile, workspace, registry, and negative-path
  conformance.
- Gate: `cargo test -p jet --lib --test behavior_package_manager_lockfile_parity --test behavior_package_manager_registry_integrity --test behavior_package_manager_workspace_parity -- --nocapture`
- Gate:
  `node apps/jet/scripts/compare-pkg-management.mjs --baseline-tools npm,pnpm --require-baselines`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Package manager readiness | epic | #3779 | `node apps/jet/scripts/compare-pkg-management.mjs` |
| Package Manager Lockfile Parity | epic | #3779 | `cargo test -p jet --lib --test behavior_package_manager_lockfile_parity -- --nocapture` |
| Package Manager Workspace Parity | epic | #3779 | `cargo test -p jet --lib --test behavior_package_manager_workspace_parity -- --nocapture` |
| Package Manager Registry Integrity | epic | #3779 | `cargo test -p jet --lib --test behavior_package_manager_registry_integrity -- --nocapture` |

### Bundler And Production Build

Jet production build replacement is green after package manager and Browser
Bridge gates. The expanded DOM production build corpus has green static checks,
runtime smoke, and performance/size comparisons for the current fixture set.

- Root WI: #3782
- Surfaces: CLI: `jet build` + `jet build --wasm` - Production and WASM build
  command surface.
- Gate — behavior:
  `apps/jet/scripts/compare-dom-build-corpus.mjs --runtime-smoke required --build-samples 3`
  - DOM production build corpus, runtime smoke, and Vite/Webpack comparison
  behavior.
- Gate:
  `node apps/jet/scripts/compare-dom-build-corpus.mjs --runtime-smoke required --build-samples 3`
- Source: `apps/jet/tests/fixtures/dom-production-build`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Bundler production readiness | epic | #3782 | `node apps/jet/scripts/compare-dom-build-corpus.mjs --runtime-smoke required --build-samples 3` |
| Production Bundle Output Parity | epic | #3782 | `cargo test -p jet --lib --test behavior_production_bundle_output_parity -- --nocapture`<br>apps/jet/tests/fixtures/dom-production-build |
| Transform Resolver Parity | epic | #3782 | `cargo test -p jet --lib --test behavior_transform_resolver_parity -- --nocapture` |
| Asset Sourcemap Negative Paths | epic | #3782 | `cargo test -p jet --lib --test behavior_asset_sourcemap_negative_paths -- --nocapture` |
| SCSS / Sass Compilation | change | #204 | `cargo test -p jet --lib --test behavior_scss_sass_compilation` — grass-based (pure-Rust, no C deps) SCSS/Sass to CSS: nesting, variables, use/import partials, mixins; fed into the CSS pipeline before minify |
| Fix CSS Layer Statement Form Parse Error | change | #1377 | `cargo test -p jet --lib -- --nocapture` — the bare CSS Cascade Layers order statement (`@layer theme, base, components, utilities;`) emitted by Tailwind v4 is recognized and dropped by the directive pipeline before the final `lightningcss` parse step, alongside unchanged block-form `@layer name { ... }` inlining — see `projects/jet/tech-design/logic/jet-css-parser-fails-to-parse-tailwind-css-v4-layer-directives.md` |
| Fix CSS Bare-Specifier Import Resolution For Package Directories | change | #1375 | `cargo test -p jet --lib` — a bare-specifier `@import` (e.g. Tailwind v4's `@import "tailwindcss";`) whose `node_modules/<pkg>` path is a directory resolves via the package's `package.json` `exports`/`style`/`main` map instead of raising "Is a directory (os error 21)" — see `projects/jet/tech-design/logic/jet-build-fails-to-resolve-tailwind-css-import-tailwindcss.md` |

### Dev Server And HMR

`jet dev` can replace Vite-style local development serving and HMR for real
projects. Dev mode prioritizes a connected browser client, HMR, browser log
intake, and dev-only reverse proxy rules from `[dev.proxy]` in `jet.toml` or
repeatable `--proxy PATH=URL` CLI overrides. `jet serve` is a separate
Kubernetes/GKE static frontend data plane behind a load balancer, with
nginx-class static serving behavior and a hot path tuned for low memory-copy
overhead and high RPS; it does not own TLS termination, public virtual hosts,
cert management, WAF/CDN, or cross-service ingress routing. Current local proof
includes prod static serving versus nginx with first-byte p95 ratio `0.803` and
throughput ratio `1.164`.

- Root WI: #3780
- Surface: CLI: `jet dev` + `jet dev --proxy PATH=URL` + `jet serve` - Dev
  server control plane, proxy/HMR entrypoints, and production static serving
  surface.
- Surface: UI: `http://localhost:<port>` - Connected browser client for HMR,
  browser log intake, and local app inspection.
- Gate — behavior: `cargo test -p jet --lib --test behavior_dev_server_cli_contract --test behavior_dev_server_local_serving_hmr --test behavior_dev_server_proxy_contract --test behavior_dev_server_replacement_readiness -- --nocapture` - Local
  serving, HMR, proxy, browser-log intake, and production static serving
  conformance.
- Gate: `cargo test -p jet --lib --test behavior_dev_server_cli_contract --test behavior_dev_server_local_serving_hmr --test behavior_dev_server_proxy_contract --test behavior_dev_server_replacement_readiness -- --nocapture`
- Gate: `cargo test -p jet --lib -- --nocapture`
- Gate:
  `cargo test -p jet --lib -- --nocapture`
- Gate:
  `apps/jet/scripts/compare-prod-static-serve.mjs --jet-bin target/release/jet`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Dev server replacement readiness | epic | #3780 | `apps/jet/scripts/verify-basic-dom-gates.sh --phase serve` |
| Dev Server Local Serving Hmr | epic | #3780 | `cargo test -p jet --lib --test behavior_dev_server_local_serving_hmr -- --nocapture` |
| Dev Server Proxy Contract | epic | #3780 | `cargo test -p jet --lib --test behavior_dev_server_proxy_contract -- --nocapture` |
| Dev Server Cli Contract | epic | #3780 | `cargo test -p jet --lib --test behavior_dev_server_cli_contract -- --nocapture` |
| React Refresh State Preserved | epic | #3780 | `cargo test -p jet --lib --test behavior_react_refresh_state_preserved -- --nocapture` |
| Prebundle Importmap Parity | epic | #3780 | `cargo test -p jet --lib --test behavior_prebundle_importmap_parity -- --nocapture` |

### Workspace And Task Runner

Jet workspace/task-runner replacement remains part of the package-management
replacement track before build claims. The canonical project configuration file
is `jet.toml`, and the active schema artifact is `schemas/jet.schema.json`.

- Root WI: #3781
- Surfaces: CLI: `jet run` + `jet exec` - Workspace script and binary execution
  surface.
- Gate — behavior: `cargo test -p jet --lib --test behavior_task_runner_graph_cache --test behavior_workspace_task_runner_readiness -- --nocapture` -
  Workspace script execution, graph cache, package selection, and task-runner
  parity behavior.
- Gate: `cargo test -p jet --lib --test behavior_task_runner_graph_cache --test behavior_workspace_task_runner_readiness -- --nocapture`
- Gate: `cargo test -p jet --lib -- --nocapture`
- Gate: `cargo run -p jet -- config schema --check`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Workspace task runner readiness | epic | #3781 | `cargo run -p jet -- config schema --check` |
| Task Runner Graph Cache | epic | #3781 | `cargo test -p jet --lib --test behavior_task_runner_graph_cache -- --nocapture` |
| Workspace Package Selection | epic | #3781 | `cargo test -p jet --lib --test behavior_workspace_package_selection -- --nocapture` |
| Nx Graph Parity | epic | #3781 | `cargo test -p jet --lib --test behavior_nx_graph_parity -- --nocapture` |

### Native Test And Product-Flow E2E

Jet native tests, reporter artifacts, product-flow e2e, and trace evidence are
green in the Basic production-readiness gate.

- Root WI: #3785
- Surface: CLI: `jet test` + `jet e2e` - Native test runner and product-flow
  e2e surface.
- Surface: WebAppE2E: `jet e2e` - Browser-driven product-flow verification for
  frontend behavior across app and API boundaries.
- Gate — behavior: `jet e2e` - Browser-driven product-flow verification across
  frontend behavior and app/API boundaries.
- Gate: `cargo test -p jet --lib --test behavior_native_test_runner_core --test test_runner_smoke -- --nocapture`
- Gate: `cargo test -p jet --lib --test behavior_reporter_artifacts --test html_reporter_tests -- --nocapture`
- Gate: `cargo test -p jet --lib --test behavior_product_flow_e2e_readiness --test behavior_product_flow_e2e_review --test e2e_playwright_residue --test library_publish_e2e -- --nocapture`
- Gate: `cargo test -p jet --lib --test behavior_browser_trace_parity_readiness --test behavior_trace_evidence_artifacts --test behavior_trace_replay_evidence --test trace_capture --test trace_viewer -- --nocapture`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Native test runner readiness | epic | #3785 | - |
| Product flow e2e readiness | epic | #3784 | Browser Bridge replacement gate plus `cargo test -p jet --lib --test behavior_product_flow_e2e_readiness -- --nocapture` |
| Native Test Runner Core | epic | #3785 | `cargo test -p jet --lib --test behavior_native_test_runner_core -- --nocapture` |
| Built In Ts Test Runtime | epic | #3785 | `cargo test -p jet --lib --test behavior_built_in_ts_test_runtime -- --nocapture`<br>apps/jet/tests/fixtures/jet-test-api-compat |
| Reporter Artifacts | epic | #3785 | `cargo test -p jet --lib --test behavior_reporter_artifacts -- --nocapture` |
| Product Flow E2e Review | epic | #3785 | `cargo test -p jet --lib --test behavior_product_flow_e2e_review -- --nocapture`<br>apps/jet/examples/jet-test-dogfood |
| Trace Replay Evidence | epic | #3785 | `cargo test -p jet --lib --test behavior_trace_replay_evidence -- --nocapture` |

### WASM And Multi-Target Execution

Jet can sink the frontend app model into WASM only after Basic package
management, Browser Bridge, and DOM production build contracts are stable
enough to reuse.

- Root WI: #818
- Surfaces: CLI: `jet build --wasm` - WASM build target surface.
- Gate — behavior: `apps/jet/scripts/verify-advanced-wasm-gates.sh` - WASM
  build, runtime subset, renderer target, and DOM/WASM parity behavior gates.
- Gate: `apps/jet/scripts/verify-advanced-wasm-gates.sh`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Wasm multi target readiness | epic | #818 | Basic phase 1 -> phase 2 -> phase 3 |
| Wasm Build Dev Core | epic | #818 | `cargo test -p jet --lib -- --nocapture` |
| Wasm Runtime Subset | epic | #818 | `cargo test -p jet-wasm -- --nocapture`<br>apps/jet/wasm |
| Renderer Target Output | epic | #818 | `cargo test -p jet-wasm --test renderer_integration_counter --test renderer_layout --test renderer_paint -- --nocapture`<br>apps/jet/wasm |
| WebGPU WASM Build Default | change | #818 | `cargo test -p jet --test wasm_build_end_to_end --test behavior_webgpu_wasm_build_default -- --nocapture` |
| WebGPU Large Table Smoke | change | #818 | `cargo test -p jet --test wasm_build_end_to_end --test behavior_webgpu_large_table_smoke -- --nocapture` |
| DOM Renderer Controlled Input Parity | change | #818 | `cargo test -p jet --test react_dom_oracle_conformance --test behavior_dom_renderer_controlled_input_parity -- --nocapture` |
| DOM Renderer Controlled Textarea Parity | change | #818 | `cargo test -p jet --test react_dom_oracle_conformance --test behavior_dom_renderer_controlled_textarea_parity -- --nocapture` |
| Library WASM Lowering Fixtures | change | #818 | `cargo test -p jet --test tsx_to_rust_imports -- --nocapture`<br>apps/jet/parity/data/fixtures/libraries |
| Library DOM/WASM Parity Fixtures | change | #818 | `cargo test -p jet --test react_dom_oracle_conformance --test behavior_library_dom_wasm_parity -- --nocapture`<br>apps/jet/parity/data/fixtures/libraries |
| MUI Visual Table DOM/WASM Parity | change | #818 | `cargo test -p jet --test mui_visual_regression --test behavior_mui_visual_table_dom_wasm_parity -- --nocapture`<br>Browser Bridge CLI capture/screenshot evidence<br>examples/mui-visual-demo |
| AntD Visual Table DOM/WASM Parity | change | #818 | `cargo test -p jet --test mui_visual_regression --test behavior_antd_visual_table_dom_wasm_parity -- --nocapture`<br>Browser Bridge CLI capture/screenshot evidence<br>examples/antd-visual-demo |

### Browser, Trace, And Parity Infrastructure

Jet Browser Bridge, trace, and parity diagnostics are the second Basic
replacement gate and the evidence substrate for later DOM/WASM parity.

- Root WI: #3786
- Surfaces: CLI: `jet bb` + `jet trace` - Browser Bridge and trace diagnostic
  surface.
- Gate — behavior: `apps/jet/scripts/verify-browser-bridge-replacement.mjs` -
  Browser Bridge automation, trace evidence, and DOM/WASM parity corpus
  behavior.
- Gate:
  `node apps/jet/scripts/verify-browser-bridge-replacement.mjs --jet-bin target/release/jet`
- Source: `apps/jet/parity/**`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Browser trace parity readiness | epic | #3786 | `node apps/jet/scripts/verify-browser-bridge-replacement.mjs` |
| Trace Evidence Artifacts | epic | #3786 | `cargo test -p jet --lib --test behavior_trace_evidence_artifacts -- --nocapture` |
| Browser Automation Diagnostics | epic | #3786 | `cargo test -p jet --lib --test behavior_browser_trace_parity_readiness --test browser_cli_smoke --test browser_context --test browser_install -- --nocapture` |
| Parity Corpus Gates | epic | #3786 | `apps/jet/scripts/verify-parity-oracle-gate.sh`<br>apps/jet/parity/** |
| WebGPU WASM Build Default | change | #818 | `cargo test -p jet --test wasm_build_end_to_end --test behavior_webgpu_wasm_build_default -- --nocapture` |
| WebGPU Large Table Smoke | change | #818 | `cargo test -p jet --test wasm_build_end_to_end --test behavior_webgpu_large_table_smoke -- --nocapture` |
| DOM Renderer Controlled Input Parity | change | #818 | `cargo test -p jet --test react_dom_oracle_conformance --test behavior_dom_renderer_controlled_input_parity -- --nocapture` |
| DOM Renderer Controlled Textarea Parity | change | #818 | `cargo test -p jet --test react_dom_oracle_conformance --test behavior_dom_renderer_controlled_textarea_parity -- --nocapture` |
| Library WASM Lowering Fixtures | change | #818 | `cargo test -p jet --test tsx_to_rust_imports -- --nocapture`<br>apps/jet/parity/data/fixtures/libraries |
| Library DOM/WASM Parity Fixtures | change | #818 | `cargo test -p jet --test react_dom_oracle_conformance --test behavior_library_dom_wasm_parity -- --nocapture`<br>apps/jet/parity/data/fixtures/libraries |
| MUI Visual Table DOM/WASM Parity | change | #818 | `cargo test -p jet --test mui_visual_regression --test behavior_mui_visual_table_dom_wasm_parity -- --nocapture`<br>Browser Bridge CLI capture/screenshot evidence<br>examples/mui-visual-demo |
| AntD Visual Table DOM/WASM Parity | change | #818 | `cargo test -p jet --test mui_visual_regression --test behavior_antd_visual_table_dom_wasm_parity -- --nocapture`<br>Browser Bridge CLI capture/screenshot evidence<br>examples/antd-visual-demo |

### Library Build And Package Publishing

jet builds publishable npm packages in library mode (ESM + optional CJS,
externalized dependencies/peerDependencies, multi-entry from package.json
`exports`), emits `.d.ts` type declarations, and `jet publish --build` builds +
validates package metadata (`exports`/`main`/`module`/`types`) before
publishing to public or private (GitLab/Verdaccio/Nexus) registries via
`.npmrc` scoped-registry auth. App-mode `jet build` is unchanged.

- Root WI: #168
- Surfaces: CLI: `jet build --lib` + `jet publish --build` - Library package
  build, metadata validation, and registry publishing surface.
- Gate — behavior: `cargo test -p jet --test library_publish_e2e` - Library
  build, declaration output, package metadata validation, and
  publish/private-registry conformance.
- Gate: `cargo test -p jet --test library_build`
- Gate: `cargo test -p jet --test library_dts`
- Gate: `cargo test -p jet --test library_publish_e2e`
- Gate: `cargo test -p jet --lib`
- Gate: `cargo test -p jet --lib`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Library publishing readiness | epic | #168 | `cargo test -p jet --test library_publish_e2e` — A1 library build, A2 declaration emission, and A3 publish/private-registry hardening are merged |
| Library Build Mode | change | #170 | `cargo test -p jet --test library_build` — ESM+CJS, externalized deps/peerDeps, multi-entry, preserve-modules ESM/CJS (IIFE TODO) |
| Type Declaration Emission | change | #171 | `cargo test -p jet --test library_dts` — declaration files per entry plus package types field (isolatedDeclarations) |
| Publish And Private Registry | change | #172 | `cargo test -p jet --test library_publish_e2e` — build + metadata validate; in-process mock-registry publish/install round-trip |
| Library CSS Cascade-Merge | change | #205 | `cargo test -p jet --lib` — cascade-ordered CSS merge across entries plus raw asset copy in library builds |

### Component Workbench (Stories)

jet discovers and parses CSF3/CSF2 `*.stories.tsx` (default-export meta +
named-export stories) with decorators, `parameters`, `globals`/`globalTypes`,
`loaders`, and the `autodocs` tag; serves a jet-native manager UI (sidebar,
isolated preview, toolbar with
viewport/background/zoom/measure/outline/highlight controls and keyboard
shortcuts) with HMR; derives a live Controls panel from component prop types +
`argTypes`; records actions and interaction/`play()` logs; runs an
accessibility (a11y) audit panel; shows story source; compiles MDX autodocs
pages (dev and static, wired to `<Canvas>`/`<Story>`/`<ArgTypes>`/`<Source>`
doc blocks); emits a static, server-less `jet stories build` export with an
`index.json` manifest; and executes `play()` interactions headlessly via
`jet test --stories`. CSF/CSF2-compatible with no Storybook runtime dependency
(all shipped by epic #1001, #981/#987-#1000, evidence closed by #1343).

- Root WI: #169
- Surface: CLI: `jet stories` + `jet stories build` + `jet test --stories` -
  Component workbench dev server, static export, and headless
  play()-interactions entrypoints.
- Surface: UI: `jet stories` manager + preview - Sidebar, isolated story
  preview, toolbar, HMR, controls, actions, interactions, a11y, source, and MDX
  docs surface.
- Gate — behavior: `cargo test -p jet --test stories_build` - Static workbench
  export, story preview modules, MDX docs pages, and relative URL behavior.
- Gate — behavior: `cargo test -p jet --test manager` - Manager UI routing,
  story listing, isolated preview, and bare-import resolution behavior.
- Gate — behavior: `cargo test -p jet --test controls` - Prop-type-derived
  controls and live arg edit behavior.
- Gate — behavior: `cargo test -p jet --test stories_parity_fixture` -
  Decorators + argTypes + play() + MDX compiling and wiring together for one
  story, in-repo (no external Storybook install).
- Gate:
  `cargo test -p jet --lib`
  — decorators/parameters/globals/globalTypes/loaders/autodocs-tag CSF
  render-path parsing
- Gate:
  `cargo test -p jet --test stories_build`
  —
  toolbar/controls/actions/interactions/a11y/story-source/docs/search/theme/index.json
  static-manager parity checklist
- Gate:
  `cargo test -p jet --lib`
  — toolbar plus measure/outline/highlight, zoom, and keyboard-shortcut
  behavior
- Gate: `cargo test -p jet --lib`
  — MDX docs-page compile (`Meta`/`Canvas`/`Story`/`ArgTypes`/`Source` doc
  blocks)
- Gate:
  `cargo test -p jet --test stories_build`
  — MDX docs pages wired into the static export
- Gate: `jet test --stories` (`cli.rs::run_stories_smoke_tests`) —
  headless-Chromium `play()` interactions runner
- Gate: `cargo test -p jet --test stories_parity_fixture` — in-repo fixture
  proving decorators + `argTypes` override + `play()` + MDX docs page compile
  and wire together for one story (#1343, AC2-equivalent evidence without an
  external Storybook install)

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Component workbench readiness | epic | #169 | `cargo test -p jet --test stories_build` |
| CSF Story Discovery | change | #173 | `cargo test -p jet --test csf_discovery` — glob + CSF3 meta/named-story parse into a story index |
| Stories Dev Manager | change | #174 | `cargo test -p jet --test manager` — Stories dev command path, manager UI, and isolated per-story preview |
| Stories Preview HMR | change | #176 | `cargo test -p jet --test preview_hmr` — watcher + WS, preview re-render/reload, manager untouched |
| Stories Controls Panel | change | #175 | `cargo test -p jet --test controls` — prop-type-inferred controls plus argTypes override; live arg edits re-render the preview |
| Stories Static Export | change | #190 | `cargo test -p jet --test stories_build` — jet stories build emits a static, server-less workbench with manager, per-story previews, transformed modules, and relative URLs |
| Hook-State-Preserving Refresh | change | #196 | `cargo test -p jet --test preview_hmr` — React Refresh preserves useState/hook state across preview edits |
| Stories Bare-Import Resolution | change | #197 | `cargo test -p jet --test manager` — node_modules bare-import resolution for stories dev and static export |
| Generic / Cross-File Prop Controls | change | #198 | `cargo test -p jet --test controls` — controls inferred from generic, cross-file, and intersection prop types |
| CSF2 Template.bind + Re-Exports | change | #199 | `cargo test -p jet --test csf_discovery` — CSF2 Template.bind, re-exported stories, and spread-args discovery |
| Close Stories Parity Evidence Gaps From Epic 1001 | change | #1343 | `cargo test -p jet --test stories_parity_fixture` — README Component Workbench row/detailed block enumerate the full shipped parity surface with concrete test-path citations, `jet-stories.md` `source_units` include `mdx.rs`/`optimizer.rs`, and an in-repo fixture combining decorators + play + argTypes + MDX passes — see `projects/jet/tech-design/validate/jet-stories-close-epic-1001-ac2-ac3-readme-parity-evidence-td-so.md` |

### Jet Project Architecture And Authoring Clarity

Jet's project architecture and authoring guidance (the top-level path-role map
plus crate/package naming conventions) lives in scoped project docs rather than
a project-root uppercase meta doc. `projects/jet/docs/architecture/layout.md`
is the discoverable, README-linked home for that guidance;
`projects/jet/LAYOUT.md` no longer exists, and `README.md` plus
`CONTRIBUTING.md` remain the only Jet project-root uppercase meta docs.

- Root WI: #1169
- Surfaces: Docs: `projects/jet/docs/architecture/layout.md` - Scoped path-role
  map and crate/package naming conventions, linked from this README's Source
  map.
- Gate — behavior:
  `test ! -e projects/jet/LAYOUT.md && ! grep -rl "projects/jet/LAYOUT.md" --include=*.md . | grep -v projects/jet/tech-design/logic/move-root-layout-meta-doc-into-scoped-architecture-documentation.md`
  - No project-root uppercase meta doc remains and no live reference still
  points at the retired path.
- Gate: `projects/jet/docs/architecture/layout.md`
- Gate:
  `test ! -e projects/jet/LAYOUT.md && ! grep -rl "projects/jet/LAYOUT.md" --include=*.md . | grep -v projects/jet/tech-design/logic/move-root-layout-meta-doc-into-scoped-architecture-documentation.md`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Move root layout meta doc into scoped architecture documentation | change | #1169 | `projects/jet/docs/architecture/layout.md` exists with the preserved path-role map and naming conventions; `projects/jet/LAYOUT.md` removed; README Source map row repointed — see `projects/jet/tech-design/logic/move-root-layout-meta-doc-into-scoped-architecture-documentation.md` |
| Rebrand jet docs site and nav-link orphaned design notes | change | #1083 | `projects/jet/docs/index.md` hero/features, `.vitepress/config.mjs` title, and `package.json` name carry jet identity (no `cclab` branding, no non-jet placeholder features); every pre-existing hand-written design-note markdown file under `projects/jet/docs/` that the doc-relocation left unlinked is reachable from the VitePress nav/sidebar — see `projects/jet/tech-design/logic/jet-docs-site-carries-cclab-era-branding-after-move-to-projects.md` |

### Jet Agent-Facing CLI Standard Commands

jet ships the shared `llm` / `upgrade` / `issue` agent-facing CLI convention
via `core/cli-std`. `jet issue` covers `search`, `view`, `create` (auto-tagged
`app:jet`), and `comment`: `jet issue comment <number> [message...]` wires to
`cli_std::issue::comment(CommentOptions)`, which ensures the target issue is
open (auto-reopening if closed) before posting a diagnostics-rich follow-up
comment, without duplicating GitHub API logic in jet's own CLI.
`jet issue comment <n> --dry-run` prints the target issue, resolved state
(open), and the assembled diagnostics comment with no network mutation.

- Root WI: #928
- Surfaces: CLI: `jet llm` + `jet upgrade` +
  `jet issue search|view|create|comment` - Agent-facing standard command
  surface shared across every ecosystem CLI.
- Gate — behavior: `cargo test -p jet --lib` - `jet issue`
  subcommand parsing/dispatch, including comment auto-reopen and dry-run
  preview.
- Gate: `cargo test -p jet --lib`
- Gate: `bash scripts/faberline-core-test.sh cli-std`
- Evidence: `jet issue --help` lists `comment`;
  `jet issue comment 123 --dry-run` prints target issue + state open +
  diagnostics comment with no network mutation — see
  `projects/jet/tech-design/interfaces/cli/jet-cli-add-issue-comment-auto-reopen-follow-up.md`
