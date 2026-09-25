---
project:
  name: axiom
  owner: chrischeng-c4
  url: https://github.com/chrischeng-c4/axiom
  ssh: git@github.com:chrischeng-c4/axiom.git
  default_branch: main
---

# CLAUDE.md - Claude Code bootstrap

## Default product route

The main Terra/high controller owns scope, dispatch, Git, tracker changes, and final
acceptance. It reads worker summaries, not product source, documents, or raw
logs.

| Request | Route |
|---|---|
| unclear outcome | `product-ideate` through `<p>-pm` |
| approved outcome needing a scoped task | `product-plan` through `<p>-pm` and `<p>-tl` |
| delivery, test-only, or read-only review | `product-deliver` |
| cross-project boundary | `cto`, then `integration-qa` only when verification crosses projects |
| material product trade-off | on-demand `product-advisor` profile |
| irreversible, paid, or permission-sensitive choice | on-demand `risk-advisor` profile |
| explicit legacy AW work | one renamed explicit-only legacy skill |
| bounded AW Python CLI work | `aw-dev` |

For normal behavior delivery, QA supplies a red e2e case and its registration,
Dev supplies a red colocated unit test and implementation, then a fresh
`<p>-qa` runs the full declared project gate. Integration QA is never the
single-project final verifier. PM owns authorized product documents. TL owns
the short technical task draft. QA and Dev do not commit, push, mutate tracker
state, or accept work. Dev cannot weaken QA tests.

`product-deliver` may run e2e-only, impl-only, test-only, or read-only-review
for an authorized partial scope. Do not require a receipt, a new role, a large
up-front design, or arbitrary approval. Permit one writer in one worktree at
a time.

The legacy skills are `prepare-goal`, `grill-release`, `e2e-for`, `impl-for`,
`test-for`, `review`, and `ask-user`. Use them only by explicit human request.
The controller alone performs AW phase state, commit, lifecycle, and close
actions. Preserve the AW CLI protocol for this legacy route.

The new product route is PENDING ACTIVATION. Do not start a product writer
task or pilot as part of this migration.

## Senior advisor policy

The main default is `gpt-5.6-terra` at high effort. It delegates detailed
work. Astra is not the normal controller model.

Ask one senior advisor only when the evidence leaves a material decision open.
Give the advisor one exact question and the needed evidence. The advisor has
no Git, tracker, release, cloud, or acceptance authority.

| Profile | Model and effort | Use when |
|---|---|---|
| `cto` | Sol / xhigh | a shared interface, project boundary, or architecture decision crosses projects |
| `product-advisor` | Sol / high | product value, scope, or roadmap choices conflict |
| `risk-advisor` | Sol / xhigh | a release, paid run, deletion, permission, or other irreversible choice needs a second judgment |

Do not ask an advisor for routine routing or a clear, repeatable task. Do not
ask all advisors for one decision. Their profiles are future fleet policy, not
active role configuration. The isolation admission work remains incomplete.

## Conversation shortcuts

Use `next-step` for a read-only choice. It gives at most two options and one
recommendation with its purpose, completion condition, and needed authority.
It may check only needed read-only evidence. It never starts the recommended
action.

Use `follow-next-step` only after a clear request to do the latest unfinished
main recommendation. It rechecks only that target and routes real work to an
available specialist. Stop when the pending work or authority is unclear.

Use `approve-next-step` only for a clear current assent to the latest pending,
well-scoped request. It approves that one target and limit. It does not grant
future authority or bypass a gate, a read-only rule, Plan mode, or the blocked
writer policy.

## Fleet and safe dispatch

The 98-role fleet is a generated candidate target, not the active fleet. It
has 23 apps with PM/TL/QA/Dev, custom `aw-dev`, and six singleton
roles. Edit templates under
`scripts/agents/templates/`, not generated fleet copies, then use
`scripts/agents/render_fleet.py` only from the repository root.

The candidate maps PM/TL and Integration QA to Terra and QA/Dev to Luna. The
future CTO advisor profile uses Sol/xhigh. Its efforts are PM high, TL xhigh,
QA max, and Dev medium. Only one declared model result was measured:
`lumen-qa` used Luna at max effort.

The CLI canary allowed all six forbidden writes. The native read-only canary
also wrote its first `/private/tmp` sentinel. The desktop admission test then
ignored the installed hook and launched `lumen-dev`; it did no tool or write
action. The global configuration was restored. These results do not prove
permission isolation or hook enforcement. Do not claim either for Codex or
Claude.

## Controller rules

The controller stays thin: it does not inspect product code, docs, or raw test
logs. It reads concise worker evidence and owns final acceptance. It preserves
unrelated work and runs Git through `git -c core.fsmonitor=false …`.

Use a fresh linked-worktree executor task only for a user-authorized frozen
external payload. It receives an absolute controller assignment and runs the
generic executor entrypoint. For an authorized paid GKE run, use only the
controller-named repository script or workflow and monitor it. The controller
owns semantic verification, Git, tracker, publication, and cleanup choices.

Treat `main`, `app/*`, `lib/*`, `project-mamba`, `project-lumen`, and
`examples` as persistent refs. Never delete or force-overwrite one without
explicit human confirmation. Preserve dirty changes and use the Git skill
routes. Use `build-release` for an explicit release from an immutable verified
candidate. Candidate acceptance is not stable-release authority. Never tag,
promote, publish, or close work automatically. A worker report or partial gate
is not final acceptance. The controller accepts only after independent,
necessary verification.

`aw` is the Python CLI at `apps/aw`, invoked as
`uv run --project apps/aw aw <group> …`. It remains available for the explicit
legacy route only.
