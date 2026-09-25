# Codex agent fleet — projection of `.claude/agents/`

This directory holds the Codex-runtime projection of the Claude agent fleet:
one `<name>.toml` per `.claude/agents/<name>.md`, 96 in total. The Claude
markdown definition is the source of truth; each TOML carries the same
`name`, `description`, pinned reasoning effort, and the full markdown body as
`developer_instructions`. The generator is `scripts/agents/render_fleet.py`:
`--write` re-renders every projection after a `.claude/agents/` change and
removes any `*.toml` with no markdown twin, so a removed Claude agent cannot
survive here as a stale role; `--check` (run by
`.codex/hooks/test_require_spawn_agent_effort.py`) refuses a hand-edited
projection. The per-project markdown itself is rendered from
`scripts/agents/templates/<tier>/<role>.md` — edit the template, never one
project's copy.

The fleet has PM, TL, QA, and Dev roles for 23 apps. `aw-dev` is a
tailored project role. The only non-project roles are `cto`, `project-manager`,
`tech-design`, and `integration-qa`:

| Role | Effort | Owns |
|---|---|---|
| `<p>-pm` (23) | `high` | one project's `README.md`, `STATUS.md`, `ROADMAP.md`, and `docs/**` as uncommitted drafts that pass `aw metadoc check` and `aw meta check`; never commits or binds a Milestone |
| `<p>-qa` (23) | `low` | one frozen executor assignment; the assignment limits E2E paths or a measure-only final check |
| `<p>-dev` (23) | `low` | one frozen executor assignment; the assignment limits source and colocated unit-test paths |
| `cto` | `high` | one cross-project boundary decision draft (shared → `libs/`, app-owned, or a new lib) as a `type:spike` body; writes nothing |
| `project-manager` | `medium` | one release Milestone description draft, validated with `aw milestone validate --draft`; never creates it |
| `tech-design` | `xhigh` | one Milestone's GHAN issue-body drafts under `aw change bodydir`, validated with `aw change validate --body-file`; never creates them |
| `aw-dev` | `low` | one frozen executor assignment for the `apps/aw` Python CLI |

Unlike the retired single-role fleet, **effort is pinned per role**, exactly
as the Claude frontmatter pins it. Dispatch must pass the same value as
`reasoning_effort`; `.codex/hooks/require_spawn_agent_effort.py` refuses a
spawn whose effort does not match the named `agent_type`'s
`model_reasoning_effort`, mirroring `.claude/hooks/require_agent_effort.py`.
A hard case may still be raised at dispatch time — by editing nothing and
overriding `model` in the spawn call — but phase ownership does not move
with the model.

PM/TL and Integration QA use `gpt-5.6-terra`. QA/Dev use
`gpt-5.6-luna` as low-effort worktree executors. The Claude fleet pins its
model per role; this projection carries the Codex model and effort split.
Two Claude frontmatter fields have no Codex TOML equivalent and are not
projected: `tools` (Codex has no per-tool allowlist; the `## Never` sections
carry the same boundaries as prose) and `skills` (the bodies name the
`/aw-e2e-for` / `/aw-impl-for` workflows directly).

QA/Dev run exactly one frozen assignment from their assigned linked worktree.
They call `scripts/execute_assignment.py`; its backend details are private.
For a paid GKE gate, run only the named repository script or workflow and
monitor it. See `AGENTS.md` for the authority and acceptance boundaries.
