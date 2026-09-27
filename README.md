# axiom

This repository is archived. The monorepo was split into one repository per
project under the [faberline](https://github.com/faberline) organization; each
carries its own history, filtered out of this one, and its open branches.

| Was | Now |
|-----|-----|
| `libs/`, `crates/cli-registry`, `vendor/jieba-rs` | [faberline/core](https://github.com/faberline/core) |
| `apps/arena` | [faberline/arena](https://github.com/faberline/arena) |
| `apps/aw` | [faberline/aw](https://github.com/faberline/aw) |
| `apps/beam`, `projects/beam` | [faberline/beam](https://github.com/faberline/beam) |
| `apps/cap` | [faberline/cap](https://github.com/faberline/cap) |
| `apps/cgdb` | [faberline/cgdb](https://github.com/faberline/cgdb) |
| `apps/courier` | [faberline/courier](https://github.com/faberline/courier) |
| `apps/cube` | [faberline/cube](https://github.com/faberline/cube) |
| `apps/defer` | [faberline/defer](https://github.com/faberline/defer) |
| `apps/guard` | [faberline/guard](https://github.com/faberline/guard) |
| `apps/jet`, `projects/jet`, the grid, grid-wasm, grid-render-webgpu and wal crates | [faberline/jet](https://github.com/faberline/jet) |
| `apps/keep` | [faberline/keep](https://github.com/faberline/keep) |
| `apps/loom` | [faberline/loom](https://github.com/faberline/loom) |
| `apps/lumen` | [faberline/lumen](https://github.com/faberline/lumen) |
| `apps/mamba`, the mamba-only crates, `vendor/cranelift-jit` | [faberline/mamba](https://github.com/faberline/mamba) |
| `apps/mesh` | [faberline/mesh](https://github.com/faberline/mesh) |
| `apps/meter` | [faberline/meter](https://github.com/faberline/meter) |
| `apps/pgpool` | [faberline/pgpool](https://github.com/faberline/pgpool) |
| `apps/pm` | [faberline/pm](https://github.com/faberline/pm) |
| `apps/preview` | [faberline/preview](https://github.com/faberline/preview) |
| `apps/relay` | [faberline/relay](https://github.com/faberline/relay) |
| `apps/rig` | [faberline/rig](https://github.com/faberline/rig) |
| `apps/sift` | [faberline/sift](https://github.com/faberline/sift) |
| `models/` (som-core, som-code-python) | [faberline/som](https://github.com/faberline/som) |
| `apps/tape` | [faberline/tape](https://github.com/faberline/tape) |
| `apps/vat` | [faberline/vat](https://github.com/faberline/vat) |
| `apps/workbench` | [faberline/workbench](https://github.com/faberline/workbench) |
| agent skills, hooks and fleet, `scripts/`, `acceptance/`, `benchmarks/`, shared workflows | [faberline/workspace](https://github.com/faberline/workspace) |

Open issues moved to the repository of the project they belong to, with their
labels and milestones; the backlog of the planned Truce service lives in
[faberline/truce](https://github.com/faberline/truce). Old issue links redirect.

The unused `cclab-*` crates and the legacy `python/` tree were retired rather
than moved; they remain in this repository's history, as does every file the
split left behind. The two parents of this commit are the trees the project
repositories were filtered from: the last commit on main for som, the cutover
commit for every other project.

## License

MIT — see [LICENSE](LICENSE).
