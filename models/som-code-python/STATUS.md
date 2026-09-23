# SOM code corpus for Python status

## Scope

This document describes the support contract in the current repository source.
It does not claim that every listed gate ran in this working session.

The rows are the corpus surfaces: the oracle families and their harness, the
curated rationale and its check, the snippet ISA and its verifier, the project's own test suite, its packaging, and
the legacy lineage carried over from the earlier ranker research. Use the
[README](README.md) for the workflow and capabilities. Use the
[roadmap](ROADMAP.md) for future outcomes and explicit non-goals.

## State definitions

| State | Meaning |
|---|---|
| Supported | The current source has a public contract, an implementation, and a named executable gate for the stated scope. |
| Limited | The current source supports the stated scope, but the Limits cell names a material boundary. |
| Not supported | The behavior is not part of the current product contract. The Evidence cell points to a future outcome or a non-goal. |

## Support matrix

| Surface | ID | State | Supported scope | Limits | Evidence |
|---|---|---|---|---|---|
| Executable oracle corpus | `oracle-corpus` | Limited | The harness discovers every family under `data/python-v2/materials`, runs each candidate against its fixture, and exits 0 only when every gold exits 0, every near miss fails on exactly the fixture tests its `caught_by` declares, and every declared `oracle` is the fixture the harness ran; the current run evaluates all 101 families with every gold and every near miss passing that contract. | Families `08-asyncio-concurrency-limiter`, `09-pydantic-field-cross-validation`, and `10-security-timing-constant-auth` have no fixture under `data/python-v2/fixtures`; the harness prints that no test file matched, skips them, and still exits 0, so their candidates are unproven. See [Oracle fixtures complete](ROADMAP.md#oracle-fixtures-complete). | `models/som-code-python/.venv/bin/python models/som-code-python/data/python-v2/fixtures/verify_harness.py` |
| Curated rationale | `curated-rationale` | Limited | Every `family.json` carries `rationale.teaches`, `rationale.why`, and `oracle`, every near miss carries `why_wrong` and `caught_by`, and every family carries a `caption` of 80 to 400 words and a `decompiled` block measured from the gold AST; the check refuses a caption that omits a raised exception, declared status code, or imported library, a `decompiled` block that drifted from the source, a missing or trivial field, a `why_wrong` that repeats its failure mode, a `caught_by` naming a test the fixture does not define, a label word in candidate source, and a comment or docstring that differs across the six candidates; the current run reports 0 defects over 101 families. | Families `08-asyncio-concurrency-limiter`, `09-pydantic-field-cross-validation`, and `10-security-timing-constant-auth` have no fixture, so their `oracle` and every `caught_by` under them is `null` and their `why_wrong` sentences are unmeasured prose. See [Oracle fixtures complete](ROADMAP.md#oracle-fixtures-complete). | `models/som-code-python/.venv/bin/python models/som-code-python/scripts/verify_curation.py` |
| Snippet ISA v1 | `snippet-isa` | Supported | The eight snippets across `fastapi`, `pydantic`, and `sqlalchemy` exist with consistent Mustache parameters, assemble in dependency order into one module, and that module parses and byte-compiles. | The ISA has one consumer, the verifier; no assembler executes it yet, and the library set is fixed at three. | `models/som-code-python/.venv/bin/python models/som-code-python/scripts/verify_snippets.py` |
| Project test suite | `python-test-suite` | Limited | `tests/` runs under the project virtualenv and the suite passes for the oracle, candidate quality, corpus materializer, seed inventory, training runtime, and snippet adversarial cases. | The following tests fail because the legacy corpora and runtimes they read are not tracked here: `tests/test_modern.py::test_modern_splits_counts_and_public_boundary`; `tests/test_next.py::test_next_holdout_is_unused_and_oracle_labeled`; in `tests/test_ranker.py` `test_v4_boundaries_and_count_balance`, `test_new_public_holdout_excludes_prior_test_and_development`, `test_mbpp_normalized_code_has_no_public_overlap`, `test_frontend_new_families_have_real_failing_candidates`, `test_browser_diagnostic_excludes_all_development_families`, and `test_feature_transfer_keeps_only_identical_rows_and_charges_time`; in `tests/test_som_python_corpus.py` `test_smoke_builder_omits_one_deterministic_near_miss_from_present_rows`, `test_smoke_builder_uses_its_own_directory_by_default`, `test_smoke_builder_accepts_an_existing_absolute_staging_directory`, `test_smoke_builder_rejects_staging_root_that_overlaps_source_fixtures`, `test_smoke_builder_stream_copy_preserves_source_bytes`, `test_smoke_builder_copy_never_executes_fixture_content`, `test_smoke_builder_copy_rejects_source_path_escape`, `test_smoke_builder_copy_timeout_names_the_fixed_fixture_path`, `test_smoke_builder_stream_copy_rejects_symlink`, `test_smoke_hydration_covers_exactly_the_selected_fixed_candidate_files`, `test_smoke_hydration_timeout_names_the_fixed_fixture_path`, `test_smoke_hydration_uses_a_small_bounded_worker_pool`, and `test_smoke_hydration_rejects_paths_outside_fixed_source_root`; in `tests/test_som_python_material_inventory.py` `test_inventory_is_staged_and_does_not_make_formal_rows`, `test_failing_quality_pack_is_refused`, and `test_existing_packs_can_be_staged`. See [Legacy suite retired](ROADMAP.md#legacy-suite-retired). | `models/som-code-python/.venv/bin/python -m pytest models/som-code-python/tests -q` |
| uv-runnable packaging | `uv-runnable-packaging` | Not supported | None. | `uv run --project models/som-code-python` cannot resolve the `som-core` dependency, and `pyproject.toml` looks for the package under `src/` while the code lives under `som/`. | [uv-runnable packaging](ROADMAP.md#uv-runnable-packaging) |
| Algorithm, data structure, and design pattern families | `algorithm-and-pattern-families` | Not supported | None. | Every shipped family is a library-usage pitfall; no family teaches an algorithm, a data structure, or a design pattern as such. | [Algorithm, data structure, and design pattern families](ROADMAP.md#algorithm-data-structure-and-design-pattern-families) |
| Legacy ranker lineage | `legacy-ranker-lineage` | Not supported | None. | `som/specialists`, `som/developer`, the `developer-runtime` and `rust-runtime` trees, and the `som-research-*.sources.lock.json` files are carried over from the earlier ranker research and make no promise here; the v4 seed adapter that `som/specialists/som_config.py` expects under `models/som-seeds/v4` is kept locally and is not tracked. | [Legacy ranker lineage](ROADMAP.md#legacy-ranker-lineage) |

## Evidence policy

- A Supported or Limited row names the command that measures it, run from the
  repository root against the project virtualenv.
- A Limited row's Limits cell states the boundary as an observation and names
  every tolerated failure verbatim, never as a count, and links the ROADMAP
  outcome that removes it.
- A Not supported row links exactly one ROADMAP outcome or non-goal and claims
  no gate.
- The harness exiting 0 is not proof for a family it skipped; the Limits cell
  of the oracle row is where a skipped family is recorded.
