---
project:
  name: axiom
  owner: chrischeng-c4
  url: https://github.com/chrischeng-c4/axiom
  ssh: git@github.com:chrischeng-c4/axiom.git
  default_branch: main
---

# AGENTS.md - Codex bootstrap

## Review prompts

When a review prompt supplies its standard, paths, and output contract, review
only those artifacts. Do not edit them, run AW lifecycle verbs, or write Git,
tracker, release, or cloud state. Run any needed Git read through
`git -c core.fsmonitor=false`.

## Default product route

The main Terra/high controller fixes scope, dispatches work, reads short worker
summaries, and owns Git, tracker mutation, and final acceptance. It does not
investigate product source, documents, or raw logs.

| Request | Route |
|---|---|
| unclear product outcome | `product-ideate` through `<p>-pm` |
| approved outcome needing scope | `product-plan` through `<p>-pm` and `<p>-tl` |
| scoped delivery, test-only work, or read-only review | `product-deliver` |
| cross-project boundary | `cto`; use `integration-qa` only for cross-project verification |
| material product trade-off | on-demand `product-advisor` profile |
| irreversible, paid, or permission-sensitive choice | on-demand `risk-advisor` profile |
| explicitly requested legacy AW work | one renamed explicit-only legacy skill |
| bounded `apps/aw` change | `aw-dev` |

New behavior work is QA e2e evidence, Dev red unit test and implementation,
then fresh `<p>-qa` final verification. A single-project final review never
uses Integration QA. QA owns e2e cases and required registration. Dev owns
source, colocated unit tests, and test wiring. Dev cannot weaken QA tests.
PM owns authorized product documents. TL owns a proportionate technical task
draft, not source. Do not require a large up-front design, receipt, or extra
approval checkpoint.

The controller may choose e2e-only, impl-only, test-only, or read-only-review
for a partial scope. Final verification runs the owning project's declared
complete, unfiltered gate. Workers report paths, commands, exit codes, and
short failures only. Permit one writer in one worktree at a time.

The seven legacy skills are `prepare-goal`, `grill-release`, `e2e-for`,
`impl-for`, `test-for`, `review`, and `ask-user`. They are explicit-only and
are not aliases for the default route. AW phase state, commit, lifecycle, and
close actions remain controller-only even in legacy work.

The product route is active with prompt-only role boundaries. Start one scoped
role task at a time. The controller names its exact paths, keeps one writer in
one worktree, and rejects a report that crosses the assigned role or paths.

## Senior advisor policy

The main default is `gpt-5.6-terra` at high effort. It delegates detailed
work. It does not use Astra as the normal controller model.

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

Use `next-step` to give a read-only choice. It gives at most two options and
one recommendation with its purpose, completion condition, and needed
authority. It may check only needed read-only evidence. It never starts the
recommended action.

Use `follow-next-step` only after a clear request to do the latest unfinished
main recommendation. It rechecks only that target and routes real work to an
available specialist. Stop when the pending work or authority is unclear.

Use `approve-next-step` only for a clear current assent to the latest pending,
well-scoped request. It approves that one target and limit. It does not grant
future authority or bypass a gate, a read-only rule, Plan mode, or the blocked
writer policy.

## Fleet and permission boundary

The Codex 96-role fleet is active. It has PM/TL/QA/Dev for 23 apps,
with a tailored `aw-dev`, plus `cto`, `project-manager`, `tech-design`,
and `integration-qa`. Templates under
`scripts/agents/templates/` render project Markdown and `.codex` projections
through `scripts/agents/render_fleet.py`; never hand-edit generated files.

The fleet maps PM/TL and Integration QA to Terra and QA/Dev to Luna. QA/Dev
are low-effort worktree executors. They run one frozen controller assignment
from their assigned linked Git worktree. They do not edit product files
directly. The future CTO advisor profile uses Sol/xhigh. Its efforts are PM
high, TL xhigh, and QA/Dev low.

This is prompt-only scope control, not permission isolation. The CLI canary
allowed all six forbidden writes. The native read-only canary also wrote its
first `/private/tmp` sentinel. The desktop admission test ignored the hook and
launched `lumen-dev`. Do not claim sandbox isolation or hook enforcement for
Codex or Claude. Permission testing is separate work.

## Git and external work

Run Git as `git -c core.fsmonitor=false …`. Preserve unrelated work. The
controller alone commits, pushes, changes tracker state, publishes, or closes
work. A `<project>-dev` or `<project>-qa` agent may run exactly one frozen
executor assignment from its assigned linked worktree. It runs
`scripts/execute_assignment.py` with an absolute controller-state assignment
path outside the repository. The controller owns semantic verification and
acceptance. For an authorized paid GKE run, run only the named repository
script or workflow and monitor it to a terminal state. Never use manual
`gcloud`, `kubectl`, or `terraform` commands; a script result is not
acceptance.

Treat `main`, `app/*`, `lib/*`, `project-mamba`, `project-lumen`, and
`examples` as persistent refs. Never delete or force-overwrite one without
explicit human confirmation. Preserve dirty changes and use the Git skill
routes. Use `build-release` for an explicit release from an immutable verified
candidate. Candidate acceptance is not stable-release authority. Never tag,
promote, publish, or close work automatically. A worker report or partial gate
is not final acceptance. The controller accepts only after independent,
necessary verification.

`aw` is the Python Typer CLI at `apps/aw`, invoked as
`uv run --project apps/aw aw <group> …`. Its stdout and exit-code protocol is
preserved for explicit legacy use.
