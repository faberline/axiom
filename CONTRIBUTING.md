# Contributing

> An agent or human should learn *what exists* and *where to act* from `ls`,
> paths, and filenames alone — without opening files. **Every file you don't
> open is a saved tool call, less context burned, and a more precise action.**

This repository is a multi-language ecosystem (Rust runtime + libraries, TS/UI,
Python conformance tests, specs, generated code, configs, docs, handoffs). Two
conventions run across all of it, both in service of the same goal — a tree an
agent can navigate cheaply and act on correctly:

1. **Authoring** — how to shape *any* artifact tree (files, paths, names) so it
   is legible from its structure alone.
2. **Ecosystem conventions** — the shapes every long-running **service** and
   every **CLI** repeats, so there is one stack to learn instead of one per
   project.

---

## Contents

- [Authoring principle: right-sized files, semantic paths, explicit names](#authoring-principle-right-sized-files-semantic-paths-explicit-names)
  - [The three rules](#the-three-rules)
  - [Balanced splitting](#balanced-splitting)
  - [Granularity scales with tooling](#granularity-scales-with-tooling)
  - [Path grammar (a pattern, not a mandate)](#path-grammar-a-pattern-not-a-mandate)
  - [Organize by domain, not by tooling](#organize-by-domain-not-by-tooling)
  - [Where it applies (scope)](#where-it-applies-scope)
  - [Example: decomposing a monolith into a navigable tree](#example-decomposing-a-monolith-into-a-navigable-tree)
- [Authoring convention: every agent instruction is Goal / How / Acceptance / Never](#authoring-convention-every-agent-instruction-is-goal-how-acceptance-never)
  - [Why a structure at all](#why-a-structure-at-all)
  - [The layering GHAN sits in](#the-layering-ghan-sits-in)
  - [The four sections](#the-four-sections)
  - [Negative control (mandatory — a report without it is rejected)](#negative-control-mandatory-a-report-without-it-is-rejected)
  - [Report back, with measured numbers](#report-back-with-measured-numbers)
  - [Applying GHAN outside work items](#applying-ghan-outside-work-items)
- [Service archetype: durable-only, HA, HTTP/2 + OpenAPI, k8s-native](#service-archetype-durable-only-ha-http2-openapi-k8s-native)
  - [Service workload profiles — common, StatefulSet, Deployment](#service-workload-profiles-common-statefulset-deployment)
  - [Control plane and data plane responsibilities](#control-plane-and-data-plane-responsibilities)
- [Shared libraries: naming, the kit, and the shape they produce](#shared-libraries-naming-the-kit-and-the-shape-they-produce)
  - [Shared-library naming grammar](#shared-library-naming-grammar)
  - [The shared service kit — compose these libs, do not hand-roll](#the-shared-service-kit-compose-these-libs-do-not-hand-roll)
  - [Service source layout — the same kit produces the same shape](#service-source-layout-the-same-kit-produces-the-same-shape)
- [Service contracts: transport, spec, endpoints, auth, HA](#service-contracts-transport-spec-endpoints-auth-ha)
  - [Transport — h2c + OpenAPI on one port](#transport-h2c-openapi-on-one-port)
  - [OpenAPI client codegen — typed clients from the spec](#openapi-client-codegen-typed-clients-from-the-spec)
  - [Standard endpoints — one operational surface, one contract three ways](#standard-endpoints-one-operational-surface-one-contract-three-ways)
  - [Service auth — one Bearer-token contract](#service-auth-one-bearer-token-contract)
  - [HA — `raft-core`, sharded and strongly consistent](#ha-raft-core-sharded-and-strongly-consistent)
- [Deployment: artifacts, tenancy, and the control/data split](#deployment-artifacts-tenancy-and-the-controldata-split)
  - [Deploy artifacts — image, cluster API, operator, instance](#deploy-artifacts-image-cluster-api-operator-instance)
  - [Deploy tenancy — dedicated first, shared only when justified](#deploy-tenancy-dedicated-first-shared-only-when-justified)
- [Operations baseline — notice the breach, watch the watcher, answer "converged?"](#operations-baseline-notice-the-breach-watch-the-watcher-answer-converged)
- [Conformance: what proves a service actually complies](#conformance-what-proves-a-service-actually-complies)
  - [Test layout — `e2e/*.rs`, explicitly declared, no autodiscovery](#test-layout-e2ers-explicitly-declared-no-autodiscovery)
  - [Architecture/profile conformance checklist](#architectureprofile-conformance-checklist)
  - [EC gates — `vat`-driven, evidence under `external-contracts/`](#ec-gates-vat-driven-evidence-under-external-contracts)
  - [Service dogfood rules — keep the whole surface honest](#service-dogfood-rules-keep-the-whole-surface-honest)
- [CLI convention: every CLI ships `llm`, `upgrade`, `issue`](#cli-convention-every-cli-ships-llm-upgrade-issue)
- [CLI convention: stdout tells the agent the next step](#cli-convention-stdout-tells-the-agent-the-next-step)
- [DX convention: every service and CLI ships a Developer & Agent Experience capability](#dx-convention-every-service-and-cli-ships-a-developer-agent-experience-capability)
- [Meta-doc content contract](#meta-doc-content-contract)
- [Project build and release contract](#project-build-and-release-contract)
  - [Debug builds](#debug-builds)
  - [Release builds](#release-builds)

## Authoring principle: right-sized files, semantic paths, explicit names

This principle is **medium-agnostic** — it is about *navigability*, not any
language. Make the repository legible from its structure, so an agent can decide
where to act, and tooling can operate on the tree, without opening many files.

### The three rules

| Rule | What it asserts | Smell → fix |
|------|-----------------|-------------|
| **Right-sized files** | one coherent concern per file — one reason to exist, one reason to open. "Right-sized" ≠ "small": grain follows *access pattern* + *cohesion* (see *Balanced splitting*). | a file you open for several unrelated reasons → split |
| **Semantic paths** | the directory IS the taxonomy; the path conveys a file's role before you open it. Path ⇔ identity are mutually predictable. | you can't predict the path from what the file *is* → reclassify |
| **Explicit names** | the leaf name briefs the content; `ls <dir>/` reads as a table of contents. | a name that needs the body to grasp (`misc_cases`) → rename to the observable (`isleap_rule`) |

### Balanced splitting

Splitting must *earn its keep* — it pays off only when it improves navigation,
reviewability, reuse, or selective execution. Default to keeping.

```text
decide(file) →
  SPLIT  if ANY of:
    - a reader must search INSIDE it to reach one independent concern
    - the pieces are owned / reviewed separately
    - the pieces are executed / skipped / generated / compared independently
    - the resulting leaf names would form a useful table of contents
  KEEP   if ANY of:                                # cohesion outweighs file count
    - the pieces only make sense read together     # split → a cross-file puzzle
    - splitting would create trivial wrapper files
    - a shared setup dominates the content          # the setup, not the cases, is the file
    - the directory would only get noisier, not more discoverable
  DEFAULT → KEEP
```

Tie-break: a file needing internal headers between *unrelated* concerns wants
splitting; parts sharing one concept or one setup stay together (a single rule
over a few representative inputs → one file, as a table).

### Granularity scales with tooling

> Judgment call, called deliberately: the finer you split, the more files must
> stay mutually consistent — so **push granularity as fine as your tooling can
> keep consistent, no finer.**

```text
generated + linted  ⇒  go maximally atomic    # consistency is mechanical — you never hand-maintain the files
hand-authored       ⇒  stay coherent          # the consistency cost bites at scale — lean to cohesion
```

So a fully-tooled fixture tree goes maximally atomic (worked example below),
while hand-written source should not.

### Path grammar (a pattern, not a mandate)

```
<area>/<subject>/<concern>/<artifact>
```

- **area** — broad repo area: `tests`, `specs`, `configs`, `generated`, `docs`,
  `handoffs`, …
- **subject** — the module / feature / protocol / package / service / topic.
- **concern** — *the question this file answers, or its role*: behavior, errors,
  security, performance, integration, schema, api, migration, … (an open idea,
  not a fixed list — each tree names its own concerns).
- **artifact** — the specific case / scenario / generated unit / config concern /
  document.

Not every tree needs four levels — use the depth the tree earns. The same
grammar reads across media:

```
configs/auth/oauth_token_lifetime.yaml
specs/http/errors/malformed_header_rejected.md
generated/parser/ast/node_kinds.ts
handoffs/release/2026-05-rc1-risk-summary.md
tests/std-libs/calendar/behavior/isleap_rule.py     # worked below
```

### Organize by domain, not by tooling

Path grammar fixes the *axis* of a tree; this fixes its *first split*. Organize a
tree by the **capability domain** it covers, not by the runner or tool that
happens to execute it. The top level should tell an agent *what boundary each
subtree pins* before it opens anything — that is what turns an `ls` into a map
instead of a guess.

A test tree, for instance, splits by what it proves — never by test framework:

```text
tests/
├── <external-contract>/   parity / replacement contract against an oracle
├── <native-libs>/         your own library contracts (no external oracle)
├── <cli>/                 CLI / tool behavior, pinned on the built binary
└── governance/            meta-gates over manifests, profiles, CI policy, inventory shape
```

(`apps/mamba/tests/` is the reference adopter: `cpython/` pins the
CPython-replacement contract, `mambalibs/` the native-library contracts,
`pkgmgr/` the CLI, `governance/` the meta-gates. The same first-split-by-domain
shape applies to specs, configs, and generated trees.)

Three rules keep such a tree legible:

- **Entrypoints vs taxonomy.** A domain root holds only entrypoints and taxonomy
  directories; concrete cases live *below* the taxonomy
  (`<domain>/<subject>/<concern>/<artifact>`). A parse-only case belongs under
  that domain's fixtures, never dumped in `governance/`.
- **Wire deep artifacts explicitly.** Many build systems auto-discover
  entrypoints only at a fixed location (Cargo discovers top-level `tests/*.rs`),
  so artifacts nested under a domain must be reached through an explicit
  entrypoint or umbrella runner — not dropped as ad-hoc root files. Domain-local
  helper scripts stay under their domain (`<domain>/tools/regen_golden.py`, not
  `tests/regen_golden.py`).
- **Pair a manifest with its checker.** Where a gate is a manifest plus a
  checker, keep the two discoverably named and co-located so finding one implies
  the other.

**Migrating an existing tree toward this shape?** Lock the legacy form with a
gate that counts old-style monoliths as a *ceiling* — it can only stay flat or
shrink — so the debt is visible and one-directional while new artifacts adopt
the finer shape.

### Where it applies (scope)

Strongest for **naturally decomposable** trees — independent test fixtures,
config entries, generated units, doc/handoff files — where each artifact is
genuinely standalone and (ideally) tool-maintained. Applied with **judgment** to
cohesive hand-written code: there, one-concept-one-file can rightly *outweigh*
file count, and a language's idioms win (Rust `#[test]` fns stay in a
`mod tests`; a cohesive module groups related items). This is a guideline for
legible structure, not a mandate to shred cohesive code into wrapper files.

---

### Example: decomposing a monolith into a navigable tree

> One worked instantiation of the principle above — **not** the definition.

The clearest adopter is a fully-tooled conformance fixture tree, where a
generator emits the structure and a linter enforces it — so it goes **maximally
atomic** (one self-contained case per file) and the path is the grammar made
concrete: `<bucket>/<subject>/<dimension>/<case>`, where `dimension` is the
*concern* for tests (surface · behavior · errors · bench · real_world ·
security).

#### Before → after

**Before** — one file, eight unrelated behaviors mixed together:

```
std-libs/calendar/behavior.py        # 8 cases, one big file
```

**After** — the concern is a directory; each case is a named leaf:

```
std-libs/calendar/behavior/
  isleap_rule.py                 # leap-year rule (a few representative years, one table)
  leapdays_counts.py
  monthrange_february.py
  setfirstweekday_roundtrip.py
  ...
```

`ls behavior/` is now the spec — a reader jumps straight to the one case they
need, and no coverage is lost. Note `isleap_rule.py` keeps its several input
years *together* as one table (cohesion), rather than one-file-per-input (which
would be over-splitting).

#### Keep tooling and per-tree conventions with the tree

The mechanics that make a tree fully tooled — its layout spec, the
`generate → fill → lint` loop, the file template, and tree-local authoring
conventions (hermetic per-file headers, oracle verification, performance
baselines) — belong **with the tree**, not in this general guide. For the mamba
CPython suite that source of truth is
`apps/mamba/tests/cpython/conventions/FIXTURE-LAYOUT.md`: the six-dimension
table, the `fixture_gen` → fill → `fixture_lint` loop, PEP 723 `[tool.mamba]`
headers, the CPython oracle, and the perf-baseline flow.

---

## Authoring convention: every agent instruction is Goal / How / Acceptance / Never

> A typed delivery issue, a `SKILL.md`, and a dispatch injection are the same
> artifact wearing three costumes: **an instruction telling an agent to do one
> thing.** They get one structure — **GHAN** — so an author learns it once and a
> validator checks it everywhere.

### Why a structure at all

Sections without a machine consumer get filled with boilerplate. This is
measured, not asserted. This rule was written against a historical set of open
delivery work items:

| Section | Filled with a pure title echo |
|---|---|
| `## Scope` | 55 |
| `## Capability Alignment` | 44 |
| `## Acceptance Criteria` | 43 |
| `## Requirements` | 39 |

Only 7% carry an executable gate command and 41% carry a `file:line` coordinate.

The cause was upstream of the authors: the generator those bodies came from,
`default_structured_issue_body`, interpolated the title into every section it
emitted — `- AC1: {title} is implemented and verified.` — and its sole
anti-boilerplate check rejected only seven literals (`(fill)`,
`(replace-this)`, `tbd`, `todo`, `maybe`, `unclear`, `uncertain`). A title echo
was accepted by construction.

That generator is deleted with the crate that held it. `aw change skeleton`
(`uv run --project apps/aw aw ...`) emits the empty GHAN template in its
place and `aw change validate` is what refuses a body now. The numbers above still describe the tracker,
though — those bodies were written under the old generator and nothing has
rewritten them.

So the rule that generates GHAN's shape is: **every section MUST name the
consumer that reads it and the condition under which that consumer refuses it.**
A section with no refusal condition is decoration and MUST NOT be added.

### The layering GHAN sits in

GHAN is the **authored** layer only. Phase progress — which behavior or
maintenance phase has landed, and what it was measured against — is the
**machine** layer and lives in commit trailers, never in authored prose:

- *landed?* → the phase's `Refs #<iid>` commit exists in the tree, which is what
  the next phase's predecessor check looks for.
- *against which red?* → `E2E-Red:` and `Impl-Red:` name the cases and tests
  observed failing immediately before, so the following phase's green is
  attributable to something rather than merely asserted. Impl has no predecessor
  commit to parse a trailer back out of — it is the ladder's last phase, and
  there is no earlier `impl` commit — so the evidence moves to a mid-phase
  record instead: `red` writes it to `.aw/impl-red/<iid>.json`, and
  `apps/aw/src/aw/scripts/impl.py:581-645` (`C2`) reads that record back and
  refuses a run with no record on file, a record naming no failing test, a HEAD
  that has moved since the record was measured, or a test file whose bytes have
  changed since then.
- *reviewed against which bytes?* → `E2E-Change-Digest:`, which carries
  `leg.change_digest` — one sha256 over the work-item body **and** every path
  under review (`apps/aw/src/aw/scripts/leg.py:331`).
- *which maintenance evidence?* → `Maint-Contract:` and
  `Maint-Change-Digest:` name the maintenance contract and its measured bytes.

Two properties of that layer are load-bearing. The digest covers the work item
as well as the code because the question a reviewer answers is "does this change
satisfy this work item", so an edit to either side can flip the answer: edit the
body and the recorded verdict stops matching, and an approval cannot outlive the
requirement it approved. And the record lives on the commit rather than in a
state file, so a phase rebased away takes its record with it — a state file
would go on asserting a red for history that no longer exists.

The two layers meet at exactly one point: **the gate command named in
`## Acceptance` is the command the phase runs.** Everything else is independent.

### The four sections

Each is specified as *required content · machine consumer · refusal conditions*.

---

#### `## Goal` — one observable difference

**Required.** Exactly one sentence of the form:

> Under `<trigger>`, `<observation point>` goes from `<current value>` to
> `<target value>`.

Not "improve X", not "support Y", not "make Z better". If there is no value to
name on either side, the work is not yet bounded.

**Consumer.** `aw change validate` (`apps/aw/src/aw/scripts/change.py:289-294`).

**Refused when** the section still carries the skeleton placeholder, is written
as a list, or runs to more than one paragraph. That is the whole of what a
script can see. The rest of the shape — an observation point, a current value,
a target value, and exactly one of each — is what `product-plan` asks to make
explicit; nothing measures it, so a one-paragraph sentence naming nothing
passes `validate` and is still an unbounded goal.

The boundedness heuristic that used to back this section — `looks_too_large_for_atomic_wi`,
a phrase set matched against `title + goal` — is gone rather than ported. It
lived in the deleted crate's planner instead of in the section schema, and it is
the single rule the plugin's validator deliberately does not reproduce
(`change.py:31`).

---

#### `## How` — verified premises, then the path

**Required — three blocks, all of them.**

1. **Verified premises.** Each carries a `file:line` and states an *observation*,
   never an inference. "`issues.rs:2165` pushes an error and early-returns for a
   non-structured body" qualifies; "this should block it" does not.
2. **Change points.** A `file:line` list. **This list is simultaneously the write
   allowlist** — a dispatch profile's `--write`, a guard's permitted paths, an
   execpolicy allowance.
3. **Frozen decisions and explicit exclusions.** Branches already decided
   against, with the reason.

Block 1 exists because an agent that re-derives what you already measured will
sometimes derive it wrong, and you will not notice. It is the one thing both
independently-designed dispatch briefs in this ecosystem put first and lint.

**Consumer.** This is policy only. The controller owns the mapping from the
change-point list to the frozen dispatch profile's `allowed_repo_writes`. It
must copy those paths exactly and refuse a mismatch before launch. No current
repository script builds or enforces this write contract.

**Refused when** a file path appears without a line or symbol; the current state
is described with "should"/"probably"/"appears to"; there are zero premises; or
the change-point list is empty (that is an intake `spike`, not a delivery
issue).

---

#### `## Acceptance` — a gate that has been seen to fail

**Required — a table plus a negative control plus a report contract.**

```markdown
| # | Gate command (verbatim) | Current observation | Target observation | Why it cannot hold by accident |
|---|---|---|---|---|
| 1 | `cargo test -p foo --lib` | 412 passed / 0 failed | 414 passed / 0 failed | both new assertions go red under the negative control below |

### Negative control (mandatory — a report without it is rejected)

Apply this exact mutation to `<file>`: <precise enough to be unambiguous —
which block, from where, to where, which value>. Re-run `<gate>`. **Your new
test MUST FAIL.** Quote the verbatim failure output including the test name and
the left/right assertion values. Then restore `<file>` byte-for-byte; it must
end at sha256 `<hash>`, verified with `shasum -a 256`. Re-run `<gate>` and
return to the target observation.

### Report back, with measured numbers

- gate pass/fail counts: before, under mutation, after restore
- the verbatim mutation failure output
- `shasum -a 256` of `<file>` after restore
- the exact name of the test that was added
```

Three things are deliberate:

- **"Why it cannot hold by accident" is a column, not a footnote.** It forces the
  author to answer *would this already be green?* while writing, which is the
  only moment the answer is cheap. Without it, a zero-discrimination gate passes
  review unchallenged.
- **The negative control belongs to Acceptance, not to the verification
  procedure.** In a procedure, an executor can report "I ran the gate, it was
  green". In Acceptance, failing to produce the verbatim red output *is* a failed
  criterion. A gate nobody has seen fail proves nothing: a test written against
  the implementation just produced passes by construction.
- **Counts, not exit codes.** `cargo test` with a filter matching nothing exits
  `0` and prints `0 passed`. An acceptance criterion that checks the exit code is
  blind to that entire class of false green.

**When the gate's baseline is not green, the count alone is not a criterion —
the roster is.** A target of "2 failed" lets an executor break one test and fix
another and still satisfy it. Name each pre-existing failure verbatim (test path,
assertion site, left/right values), state that exactly those are tolerated, and
make any third failure — or any change to those two — a miss. Measuring the
baseline is what surfaces this; an author who never ran the gate will write
"0 failed" and hand the executor an unreachable target.

**Consumer.** Row 1's gate command becomes `EvidenceBinding.verifier`; the table
shape and the three negative-control elements are validated at authoring time.

**Refused when** the table has no rows; any row's gate command is not verbatim
executable; current and target observations are identical; the negative control
is missing; or it lacks a mutation description, a must-fail assertion, or a
restore hash.

---

#### `## Never` — subject line, then two closed lists

**Required.** A first line fixing the addressee, because the same heading has two
different ones in practice: a dispatch skill's prohibitions address the
*controller*, a work item's address the *executor*. Unlabelled, the two get mixed
into one list and neither is enforceable.

> The subject of every restriction below is the agent executing this work item,
> not the dispatcher.

**Then both lists — neither is optional.**

- **MUST NOT touch.** Name the *near misses* explicitly; a worker that strays
  strays into the adjacent file, never a random one. All git mutation is
  forbidden by default — `add`, `commit`, `stash`, `checkout`, `restore`,
  `branch`, `reset`. Restore files by editing them back, not with git.
- **MUST NOT do.** The moves that manufacture a false green: editing the test
  instead of the implementation; weakening an existing assertion; adding
  `#[ignore]`/`skip`; narrowing a test selector until it matches nothing (a
  `cargo test` filter that matches zero tests exits 0).

**Consumer.** The touch list is the complement of the guard allowlist — an
escaped write is a finding. The do list is the source of a dispatch round's
fabrication tells, checked line by line at acceptance.

**Refused when** the subject line is missing; every entry is generic with nothing
specific to this work item; or the touch list intersects `## How`'s change
points.

### Applying GHAN outside work items

A `SKILL.md` and a dispatch injection use the same four sections with one
adjustment: a skill describes a durable capability rather than a defect, so its
`## How` carries the operating sequence in place of verified premises about a
broken current state. When a skill *does* assert current behavior, the `file:line`
rule applies to it unchanged.

---

## Service archetype: durable-only, HA, HTTP/2 + OpenAPI, k8s-native

> The ecosystem's long-running network services share one shape. A new service
> of this kind — a broker, a store, an orchestrator, an index — **copies the
> archetype** rather than reinventing its transport, consensus, deployment, or
> gates. The wins are concrete: one transport stack to learn, one HA engine to
> harden, one set of gate files an agent can find by `ls` before opening
> anything. The production data plane is durable-only: no service-archetype
> adopter may acknowledge accepted state changes from an in-memory-only path, and
> every stateful service must pair local/raft durability with scheduled off-node
> snapshots to object storage. Durable-only applies to the mutation path as a
> whole, not only the primary write path: an admin or migration verb that
> writes state outside the normal request path must either route through it
> or be explicitly checkpointed and awaited before any dependent
> orchestration step (restart, cutover, failover) proceeds.

Reference instantiations: **`keep`** (KV / claim-check store), **`relay`**
(broker), **`lumen`** (search / dedup index), and **`loom`** (workflow
scheduler). Planned service placeholders follow the same archetype:
**`tape`** (topic replay journal), **`defer`** (delayed task dispatch),
**`cube`** (OLAP service), and **`beam`** (GPU vector database).

Use these portfolio boundaries when scoping a work item or assigning an agent:

| Service | Owns | Does not own |
|---------|------|--------------|
| `loom` | workflow state, DAG scheduling, runner selection, timers, fair dispatch | broker delivery, payload bytes, replay archive |
| `relay` | online broker delivery, ordered log, broadcast fan-out, work-queue leasing | workflow decisions, long-term replay/archive, task HTTP dispatch |
| `keep` | KV/result storage, claim-check payloads, collections, durable values | broker delivery, workflow orchestration, analytical scans |
| `tape` | topic history, offset/time replay, consumer checkpoints, retention/backfill | online broker delivery, workflow decisions |
| `defer` | delayed HTTP task lifecycle, retry/DLQ, rate limits, dedupe keys | pub/sub fan-out, topic replay archive |
| `cube` | columnar facts, OLAP scan/filter/group-by/aggregate, rollups, partitions | search ranking, vector ANN, KV payload storage |
| `beam` | GPU vector indexes, vector ingest/rebuild, nearest-neighbor query | lexical/perceptual/duplicate search, OLAP aggregation |
| `lumen` | exact/lexical/semantic/perceptual/duplicate search in one service | OLAP aggregation, vector-only GPU DB ownership |

Capability-profile traits (`[capability.profile].traits` in a project's
`aw.toml`) name a baseline of required capabilities from this archetype. The
table below used to be spliced in from the `aw` CLI's trait registry; that CLI
is deleted and nothing reads `[capability.profile]` today, so this is now
hand-maintained policy — read it as a checklist an author applies, and expect
no signal when a project drifts from it.

| Trait | Derives | Enforces | About |
|---|---|---|---|
| `http2_api` | `http2-api-list` | [Transport — h2c + OpenAPI on one port](#transport-h2c-openapi-on-one-port) | Project owes a public HTTP/2 (h2c) + OpenAPI transport baseline, not full OpenAPI completeness. |
| `kubernetes_native` | `kubernetes-native-deployment` | [Deploy artifacts — image, cluster API, operator, instance](#deploy-artifacts-image-cluster-api-operator-instance) | Project owes a Kubernetes-native deployment baseline: image, cluster API, operator, instance. |
| `primary_replicas` | `primary-replicas` | [HA — `raft-core`, sharded and strongly consistent](#ha-raft-core-sharded-and-strongly-consistent) | Project owes a primary/replica HA topology baseline; select only for projects that actually support that topology. |
| `standard_endpoints` | `standard-operational-endpoints` | [Standard endpoints — one operational surface, one contract three ways](#standard-endpoints-one-operational-surface-one-contract-three-ways) | Project owes the standard /healthz, /readyz, /metrics, /openapi.json, /docs operational surface on one port. |
| `ec_gated` | `ec-gates-configured` | [EC gates — `vat`-driven, evidence under `external-contracts/`](#ec-gates-vat-driven-evidence-under-external-contracts) | Project owes vat-driven EC gate files (vat.toml/meter*.toml/guard*.toml) with evidence under external-contracts/. |
| `cli_std` | `cli-standard-surface` | [CLI convention: every CLI ships `llm`, `upgrade`, `issue`](#cli-convention-every-cli-ships-llm-upgrade-issue) | Project owes the mandatory llm/upgrade/issue CLI surface every tool in the ecosystem ships. |
| `chainable_output` | `chainable-output-conformance` | [CLI convention: stdout tells the agent the next step](#cli-convention-stdout-tells-the-agent-the-next-step) | Project owes chainable stdout: every structured/terminal output carries a runnable next command or an explicit terminal marker. |
| `cli_facing` | `cli-interface` | — | Project is primarily driven through a CLI surface; no settled CONTRIBUTING.md doc home yet. |
| `competitive_replacement` | `competitor-feature-parity`, `competitor-performance` | — | Project aims to replace or match an existing competitor tool; no settled CONTRIBUTING.md doc home yet. |
| `long_running` | `long-running-stability` | — | Project runs as a long-lived process; no settled CONTRIBUTING.md doc home yet. |
| `network_exposed` | `security-hardening` | — | Project exposes a network-reachable surface; no settled CONTRIBUTING.md doc home yet. |
| `agent_facing` | `developer-agent-experience` | [DX convention: every service and CLI ships a Developer & Agent Experience capability](#dx-convention-every-service-and-cli-ships-a-developer-agent-experience-capability) | Project is primarily driven by agents rather than humans and must own the Developer & Agent Experience capability baseline. |
| `stateful_storage` | `stateful-service-workload` | [Service workload profiles — common, StatefulSet, Deployment](#service-workload-profiles-common-statefulset-deployment) | Service owns durable application state, so it selects the StatefulSet profile: stable identity, PVC/storage durability, peer topology, backup/restore, and an explicit workload-kind migration handoff. |
| `service` | expands: `http2_api`, `kubernetes_native`, `standard_endpoints`, `ec_gated`, `cli_std`, `chainable_output` | — | Umbrella for the common service baseline; expands to transport, deploy artifacts, operational endpoints, EC gates, and CLI conventions. Without stateful_storage the primary workload is a Deployment; stateful_storage selects the StatefulSet profile. |

### Service workload profiles — common, StatefulSet, Deployment

Every service composes one common baseline and exactly one primary workload
profile. This is a service/instance ownership decision, not a replica-count
shortcut: ancillary Jobs and CronJobs do not change the primary profile.

#### Common

All profiles share server/runtime composition, standard health/readiness/
metrics surfaces, authentication and TLS policy, graceful shutdown/drain,
image plus layered k8s artifacts, operator reconciliation, labels/selectors,
owner references, ServiceAccount, ordinary client Service, and applicable
PDB/HPA helpers. Product-specific state machines, downstream quotas, schemas,
and routing policy remain in the service.

#### StatefulSet-specific

Declare `stateful_storage` when Pods own durable application state or require
stable identity/topology. This selects the StatefulSet profile and adds the
`stateful-service-workload` baseline: durable acknowledgement, PVC-backed
storage, headless Service, downward-API ordinal/peer discovery, Raft/replication
when HA is required, backup/restore, and stateful scale/upgrade rules.

#### Deployment-specific

A `service` without `stateful_storage` selects the Deployment profile. Pods own
no durable application state or stable identity: use an ordinary Deployment and
ClusterIP Service, readiness plus preStop drain, and rollout/disruption/HPA
policy. Do not add PVC, headless Service, Raft, peer DNS, or ClientIP session
affinity. If Pods consume a bounded remote dependency, scale and rolling surge
must respect that external capacity; pgpool's PostgreSQL connection budget is
the reference case.

Primary workload kind is not a live renderer toggle. The shared reconciler
applies current children but does not garbage-collect an old child of another
kind, so Deployment↔StatefulSet changes require an explicit migration handoff
that removes or transfers traffic from the stale workload before the new one
becomes authoritative.

A service is not "done" until it satisfies every row:

| Dimension | Requirement | Reference / gotcha |
|-----------|-------------|--------------------|
| **Shape** | Workspace member that is **both `lib` and `bin`** — embeddable as a crate, runnable as a server. Metadata via `version/edition/authors/license = .workspace`. | every service `Cargo.toml` |
| **Transport** | HTTP/2 cleartext (**h2c**) **+** HTTP/1.1 on **one port**, with an OpenAPI surface (`utoipa`). | Compose **`libs/service-http`** and **`libs/transport-h2c`** — built on `hyper-util` `auto::Builder`, **not `axum::serve`** (HTTP/1-only). The same crate's client side (`h2c_client`/`H2cPool`) is the in-tree client. |
| **Standard endpoints** | The same operational surface on the one port: **`/healthz`** (liveness), **`/readyz`** (readiness), **`/metrics`** (Prometheus), **`/openapi.json`** (machine OpenAPI), **`/docs`** (Swagger UI). Probes + scrape **depend** on these, so they stay auth-exempt and always-on. | Prefer **`libs/service-http`** standard probe/admin route helpers. `lumen` is the reference for the full surface. The contract is reachable three ways — **`<cli> spec`** (offline) ≡ **`/openapi.json`** (served) ≡ **`/docs`** (browsable) — one OpenAPI, three access paths. |
| **Observability / trace context** | Every request is traced and correlatable with zero extra infrastructure: the shared trace layer **accepts W3C version-00 `traceparent`** (strictly validated — invalid input is treated as absent) and **generates a fresh local root context when none arrives**; every request span carries `trace_id`/`span_id`/`parent_span_id`/`trace_flags`, and those fields flow into the structured stdout schema (`axiom.service.log.v1`) that the sift collector ingests, so cross-service log correlation works even without a trace exporter. The `otlp` feature upgrades the same context to full OpenTelemetry export. Per-op request counters/latency are served on `/metrics`. | Compose **`libs/service-http`** `trace_layer()` (`CorrelatingMakeSpan` + `request_trace_context`) — never a hand-rolled span/correlation layer. Planned at the same seam: response `Server-Timing` (#2490) and outbound `traceparent` injection for service-to-service clients. A capability a service gets from this shell belongs in that service's Observability capability row — undocumented shared behavior is undiscoverable behavior. |
| **Auth** | Every service uses the same bearer-token shape: server env `<SVC>_AUTH=off|required` plus `<SVC>_TOKEN_REGISTRY_FILE=/var/run/secrets/<svc>/token-registry.json`; clients use `<SVC>_URL` + `<SVC>_TOKEN` and send `Authorization: Bearer <token>`. | Compose **`libs/service-auth`** for middleware and **`libs/claim-token`** for signed-token verification when needed. In k8s/cloud, the registry file is mounted from a Kubernetes Secret, CSI Secret Store, or cloud Secret Manager sync. Do not add one-off auth headers, per-service token env names, or inline server token lists as the production path. |
| **OpenAPI client codegen** | Generate typed clients from the service's **own** OpenAPI via **`libs/openapi-codegen`** (`openapi-codegen`) — **never** hand-rolled or an external tool. Expose it on the CLI: `<cli> spec gen --lang ts\|py\|rust --out <dir>`. Adopters get a typed client with **no external codegen step**. | `lumen spec gen` is the reference; the polyglot core (ts/py/rust) was extracted so any CLI composes it. |
| **Durability / ack boundary** | **Mandatory for the StatefulSet profile:** an accepted mutation to service-owned state is durable before success. A Deployment-profile proxy may own no durable mutation at all; durability remains with its downstream system. | Stateful services compose **`libs/storage-durable`** plus a service-owned durable log/state store and `raft-core`/`raft-runtime` when replicated. Deployment proxies document downstream durability and prove drain/reconnect behavior instead of inventing local persistence. |
| **HA / consensus** | **Mandatory for any stateful service:** sharded, strongly-consistent state replicated with **`libs/raft-core`** driven by **`libs/raft-runtime`** — the replication path **wired** (a `RaftStateMachine` impl), not a DTO-only / "later slice" stub. Follower tails the leader over h2c; snapshot/compaction comes from the host. | Use `raft-core`+`raft-runtime`, **not `openraft`** and **not** a hand-rolled driver. The raft path may be a Cargo feature (`keep`); `lumen` is the reference adopter (`EngineSm`). |
| **Backup / restore** | Stateful services expose consistent snapshot/restore from their state machine and use **`libs/service-backup`** for destination/policy/sink/runner shape. A production instance must configure a scheduled object-storage snapshot job; manual/local snapshots are break-glass or local-dev paths, not the service-archetype baseline. | `raft-runtime` owns snapshot install + log compaction. The service admin/CLI produces snapshot bytes; the backup runner uploads to `file://`, `s3://`, or `gs://` destinations (the GCS adapter is unconditional in `service-backup`, authenticates via workload-identity ADC in-cluster, and is GKE-proven); the operator schedules, wires secrets/IAM, reports status, and never serializes service data itself. |
| **Core neutrality** | Keep domain/payload knowledge **out of the transport core** where feasible, so the core is reusable. | `relay` carries an opaque JSON body and "knows nothing about workflows" (#120). |
| **Deploy** | `Dockerfile` (+ `.release` / `.bench` variants); `<cli> dockerfile render`; **k8s-native** kustomize tree (`k8s/base` + `k8s/overlays`); `<cli> k8s crd/operator/instance`; exactly one primary workload profile. StatefulSet identity/peers come from the downward API; Deployment Pods use ordinary identity plus drain-aware rollout. | Use **`libs/service-k8s`** for CR/operator/render shape. `keep/k8s`, `lumen k8s` (+ `operator` feature), `relay/k8s`, and `loom/deploy` are adoption surfaces; when they differ, converge them toward the shared kit instead of copying local YAML. Shared multi-tenant backends are optional platform work, not the default service archetype. |
| **Operations** | Alert rules with runbooks, control-plane self-observability, `status.conditions[]`, network isolation, and verifiable release artifacts. A service that emits signal but cannot notice its own breach is not production-ready. | see *Operations baseline* below. Instrumentation (the Observability row) is the raw signal; this is what makes the signal act. |
| **Authoring ladder** | `aw.toml` names the project; changes are authored one phase at a time through `e2e` → `impl`, red first. | see the write-order table in `CLAUDE.md`. Any `tech-design/` tree and any `SPEC-MANAGED` / `HANDWRITE` marker still in source is residue — the producer that owned them is deleted, and the `.rs` is the authoring surface. |
| **EC gates** | Evidence-contract gates wired below. | see *EC gates* under *Conformance* below. |
| **CLI** | The bin ships `llm` / `upgrade` / `issue`. | see the *CLI convention* below. |

### Control plane and data plane responsibilities

*(policy-only — judgment, not trait-enforced; the split is a repository service
archetype rule until `libs/service-k8s` can validate it mechanically.)*

Every Kubernetes-native service composes a shared **control plane** and a
per-instance **data plane**. The control plane is the CRD, Operator, reconcile
loop, status, finalizers, and lifecycle resources. Its common mechanics belong
in `libs/service-k8s`; each service supplies only its domain schema, policy,
and operator defaults. The Operator normally runs in `<svc>-system`.

The data plane is the workload rendered for one service instance and the
resources that carry its traffic and durable state. For a StatefulSet-profile
service, that includes the StatefulSet, client/headless Services, PVCs, PDBs,
and applicable backup or maintenance workloads. It belongs in the instance's
app or service-owned namespace; do not require a fixed shared data-plane
namespace. A service's data-plane behavior remains domain-specific, but the
control-plane/data-plane split is a service-archetype contract, not a
service-specific capability.

When a reconcile's rendered shape stops including a resource it rendered
previously — a conditional HPA, a per-mode Service, or any other
conditionally-rendered child — the service operator must explicitly delete
that child. The shared `libs/service-k8s` reconcile loop renders desired state;
it does not garbage-collect a resource that an earlier render produced and
the current render no longer wants. The deletion must be idempotent, scoped
to the names/labels the operator itself stamps (never a foreign or
unrelated resource), and logged.

A mounted PVC is not durability. Rendering a `PersistentVolumeClaim`
template only makes storage available; the operator's rendered defaults
must also activate the service's own durable-persistence path (data-dir /
persistence env or equivalent) so the service actually writes to that
storage. The deploy baseline for a stateful service includes a
pod-delete-and-recreate proof that data written before the deletion is
still present after the pod is recreated.

## Shared libraries: naming, the kit, and the shape they produce

Three rules that compound. The naming grammar tells you what a library
owns before you open it; the kit tells you which libraries you are
required to compose; the source layout tells you what a service looks
like once it has. Skip any one and the other two stop being checkable.

The libraries live in [faberline/core](https://github.com/faberline/core) as
`crates/<name>` and are git dependencies here, pinned by tag; a `libs/<name>`
path below means that crate. Change a library in faberline/core, tag it, and
move this workspace's `tag` in the change that adopts it.

### Shared-library naming grammar

*(policy-only — a repository authoring policy, not a project capability trait.
It had a validator, `assert_semantic_names.sh`, which is deleted with the crate
that ran it; nothing checks the grammar now.)*

`libs/<name>` must expose both the library's stable responsibility and its
abstraction level before an agent opens `Cargo.toml`. A shared library is named
for what it owns, not for an implementation accident (`h2c`), an ambiguous
role (`host`, `operator`), or an outcome broader than its actual mechanism
(`durability`, `metrics`). Use these responsibility families:

| Family | Owns | Current examples |
|--------|------|------------------|
| `server-*` | Protocol-neutral lifecycle and concrete protocol server runtimes. | `server-lifecycle`, `server-tcp`, `server-http` |
| `transport-*` | Client/server wire transport and connection management, independent of app policy. | `transport-h2c` |
| `service-*` | A complete cross-app integration capability or policy shell that a service wires directly. | `service-http`, `service-observability`, `service-auth`, `service-backup`, `service-k8s` |
| `storage-*` | Storage mechanics below a service's domain durability and ack-boundary policy. | `storage-durable` |
| `metrics-*` | Metric primitives and a named exposition/backend contract, not the whole observability capability. | `metrics-prometheus` |
| `peer-*` | Peer/replication-specific security material and policy. | `peer-tls` |
| `raft-*` | Consensus algorithm and its executing runtime. | `raft-core`, `raft-runtime` |
| `cli-*`, `ui-*` | Shared CLI or renderer-neutral UI layers. | `cli-std`, `ui-runtime` |

Within a family, parallel names mean parallel abstraction levels. Reserve
`core` for an actual algorithmic core such as `raft-core`; lifecycle,
transport, orchestration, or integration crates use the more precise noun.
Protocol terms remain valid domain vocabulary (`h2c`, HTTP, Raft), but a bare
protocol is not enough as a library path when the layer would otherwise be
ambiguous. Likewise, app modules and commands may still be named `operator` —
`<cli> k8s operator run` is a product role — while the reusable library that
implements the wider Kubernetes integration is `service-k8s`.

A rename is atomic across all three identities:

```text
libs/server-http/       # semantic directory
package = server-http   # Cargo package
crate = server_http     # Rust identifier
```

Do not keep internal compatibility aliases for retired library identities;
update Cargo dependencies, Rust imports, specs, EC gates, scripts, and active
docs in the same change. Existing app API/CLI vocabulary is unaffected unless
the product contract itself changes.

### The shared service kit — compose these libs, do not hand-roll

*(policy-only — judgment, not trait-enforced)*

A service is mostly *wiring* shared libs together. **Adopting them is
mandatory** — a service that re-implements any of these is a defect, not a
variation. `lumen`, `keep`, `relay`, and `loom` may be at different adoption
stages, but new work moves them toward this common kit. If the common kit is
missing a hook, extend the shared lib first; do not fork the pattern into one
project. Shared service capabilities live in `libs/*`; projects supply only
domain-specific state machines, policy decisions, schemas, route wiring, and
operator defaults.

| Lib | Role |
|-----|------|
| **`libs/raft-core`** | the step-driven raft **consensus core** (serde-only; replaced openraft). |
| **`libs/raft-runtime`** | the raft **host**: h2c peer transport, the single apply loop, snapshot/log compaction, read-your-write `propose`, and k8s topology + auto-mode. Services provide a `RaftStateMachine` and receive HA, backup, read-consistency, and bounded outcomes. |
| **`libs/service-k8s`** | the **k8s operator scaffold + render toolkit**: `ManagedService`, `ClusterSpec`, resources, owner refs, Services/PDB/CronJobs, StatefulSet primitives, and PVC resize. It owns common operator mechanics; apps supply domain policy and defaults. |
| **`libs/server-lifecycle`** | bind configuration, shutdown/drain, readiness signals, connection budgets, and metrics hooks shared by servers. |
| **`libs/server-tcp`** | accept loop, per-connection supervision, admission budgeting, and drain-aware shutdown for raw protocols and poolers. |
| **`libs/server-http`** | the **listener-level HTTP runtime**: the sole HTTP accept owner, composing `server-tcp` admission, connection metrics, supervision, and bounded drain with `transport-h2c` per-connection HTTP/1.1+h2c handling. |
| **`libs/transport-h2c`** | the **HTTP/2 wire transport/client**: h2c client helpers (`h2c_client`/`H2cPool`) plus an optional per-connection HTTP/1.1+h2c handler; it never binds or owns a listener. |
| **`libs/service-observability`** | the **protocol-neutral observability integration**: typed logging configuration, stable service identity, optional OTLP exporter + W3C propagation primitives, the `MetricsProvider` contract, and lifecycle connection counters backed by `metrics-prometheus`. It owns no HTTP routes or request middleware. |
| **`libs/service-http`** | the **HTTP service policy shell**: standard probe/admin routes, lifecycle readiness/signal adapters, HTTP request-context propagation, runtime delegation, and the shared **HTTP error envelope** (`ErrorEnvelope` + the `ApiErr` status/kind builder). Existing observability names are compatibility re-exports from `service-observability`; it owns no protocol-neutral observability state, listener, or drain state. |
| **`libs/service-auth`** | the **request-auth shell**: shared `Authorization: Bearer` extraction, reject/inject middleware, the `Verifier` trait every service implements, and **`role_map`** — the standard token-registry verifier (`Role` hierarchy, `TokenClaims` with wildcard grants, registry-file loader, `StaticRoleMapVerifier`) implementing the archetype's `<SVC>_TOKEN_REGISTRY_FILE` contract. Token crypto belongs in **`libs/claim-token`** when signed tokens are needed; resource-policy *decisions* stay in the service handlers (`role_map` supplies the mechanism). |
| **`libs/claim-token`** | the **scoped claim-check token primitive**: HMAC signing and verification over bounded key scopes shared by issuers and storage services. |
| **`libs/storage-durable`** | the **durable local storage primitive layer**: shared `FsyncPolicy`, temp-file atomic replace with file + parent-dir sync, CRC-framed append logs with torn-tail recovery/compaction, and sequence-named local snapshot stores. Services supply domain codecs and state-machine semantics; they do not hand-roll fsync/rename/frame parsing. |
| **`libs/service-backup`** | the **backup contract**: tagged runtime destination/policy schema, the flat CRD-safe `ScheduledBackupPolicy` (`schedule`/`destination`/`retentionSecs`) with validated runtime conversion, `BackupSink`, local + S3-compatible object-store sinks (feature `s3`; GCS destinations parse/round-trip but runners fail loudly until a real GCS adapter lands), and a runner primitive. Services produce consistent snapshot bytes; runners upload them; operators add only app-specific auth/secret fields around the shared schedule policy. |
| **`libs/peer-tls`** | **peer mTLS material loading**: `PeerTlsConfig::from_env(<PREFIX>)`, PEM cert/key/CA loaders, rustls server/client config builders, and the Once-guarded default-crypto-provider install. (h2c stays cleartext by design; this covers the mutually-authenticated peer/replication port.) |
| **`libs/metrics-prometheus`** | the **Prometheus metric primitives**: dep-free counter/gauge primitives + the text-format encoder — the standard implementation behind `service-observability`'s `MetricsProvider` implementations and HTTP `/metrics` adapters. |
| **`libs/openapi-codegen`** | the **typed client generator**: one OpenAPI IR with TypeScript, Python, and Rust emitters consumed by each service's `spec gen` command. |
| **`libs/cli-std`** | the **standard CLI** commands (`llm` / `upgrade` / `issue`). |
| **`libs/build-stamp`** | the **build stamp** (a `[build-dependencies]` crate): `stamp("<PREFIX>")` emits the `<PREFIX>_GIT_SHA` / `<PREFIX>_BUILT_AT` / `<PREFIX>_TARGET` rustc-env lines that feed `cli-std`'s `ToolInfo` — one implementation instead of a per-service `build.rs` copy. |

**k8s-native auto-mode + discovery.** A StatefulSet-profile service defaults to
single-node and turns on raft **only when the StatefulSet scales out** — `raft_runtime::cluster::
replica_mode()` is `true` when `REPLICAS_PER_SHARD > 1` (a downward-API value). So
`<svc> serve` needs **no flags or cluster env** for local/single-node dev; k8s
scaling flips it to replica mode automatically, with node id / membership / peers
derived from the downward API by `ClusterTopology::from_env` (a local
`<SVC>_PEERS=host:port,…` override runs a multi-node group on one machine). Do not
hand-roll the pod-ordinal or peer-DNS math.

**Operator/render convergence.** The Kubernetes topology that drives that
auto-mode is also shared. A service CR should flatten or mirror
`service_k8s::ClusterSpec`/`ResourceSpec` unless it has a concrete product reason
not to, implement `service_k8s::ManagedService`, and render shared shapes with
`libs/service-k8s::render` helpers. For the StatefulSet profile, identity,
`SHARD_COUNT`, `REPLICAS_PER_SHARD`, `VOTER_COUNT`, headless-service env,
labels/selectors, owner refs, PDB/client/headless Service shapes, and
maintenance CronJobs are library contracts. Do not duplicate that YAML/JSON
construction in `lumen`, `keep`, `relay`, or `loom`; extend `libs/service-k8s`
when the helper is incomplete.

### Service source layout — the same kit produces the same shape

*(policy-only — judgment today; nothing mechanically checks this layout.)*

Adopting the same kit is only half the convergence. Two services that compose
identical libraries should also be **navigable identically** — an agent that has
read one service's `src/` should be able to predict the next one's without
opening a file. Today they cannot: the HTTP shell is called `server` in `tape`,
`defer`, and `relay`, `http` in `keep`, `service` in `beam`, and `api` in
`lumen`; the OpenAPI module is `spec` in some and `openapi` in others, and
`tape` carries both.

**The rule.** `apps/<svc>/src/` holds exactly three kinds of thing: the
**entry point**, the **external contract**, and the **domain state machine**.
Everything else is thin wiring over a `libs/` crate. Wiring that has grown
*body* is the signal that the shared lib is missing a hook — extend the lib,
do not let the app keep the logic.

#### Top level

```text
required  aw.toml  Cargo.toml  build.rs  build.sh  install.sh
          README.md  CAPABILITIES.md  CONTRIBUTING.md  llms.txt
          Dockerfile  Dockerfile.release  Dockerfile.test  compose.yaml
          src/  tests/  k8s/  clients/  docs/  scripts/  observability/
          tech-design/  external-contracts/
          vat.toml  guard-<scope>-security.toml  meter-<scope>-<dimension>.toml
optional  benches/  examples/  proptest-regressions/  rig*.toml
```

#### `src/` slots

| Slot | Owns | Shape comes from | Size |
|------|------|------------------|------|
| `bin/<svc>.rs` | the CLI verb tree — `serve`, `spec`, `dockerfile`, `k8s`, `backup`, plus `cli-std`'s `llm`/`upgrade`/`issue`. **Carries no logic.** | `cli-std`, `service-k8s` | thin |
| `lib.rs` | module declarations and re-exports | — | thin |
| `config.rs` | domain-specific settings only; the rest composes `service_http::ServiceConfig` | `service-http` | thin |
| `api/` | axum handlers; every error goes out as `service_http::{ErrorEnvelope, ApiErr}` | `service-http` | domain |
| `spec.rs` | the **single** `utoipa::OpenApi` aggregate plus `spec gen` → `openapi_codegen::generate` | `openapi-codegen` | thin |
| `types.rs` | external DTOs | — | domain |
| `dx.rs` | the DX capability (see *DX convention* below) | — | thin |
| `raft_sm.rs` | the `raft_runtime::RaftStateMachine` impl — **apply / snapshot / restore only** | `raft-runtime` | domain |
| `operator/{mod,crd,render,reconcile}.rs` | CRD types + CEL rules, domain defaults, the `ManagedService` impl | `service-k8s` | thin |
| `<domain>/` | state machines, storage semantics, scheduling policy — **the one place body belongs**, named in product vocabulary | itself | domain |

#### Modules that must not exist

A service that re-implements a kit responsibility is a defect, not a variation.
Each row names what the app is allowed to keep after the move.

| If `src/` contains | It belongs in | Keep only |
|---|---|---|
| `raft.rs` — peer transport, apply loop, snapshot/compaction, topology, peer DNS | `raft-runtime` | `raft_sm.rs` |
| `tls.rs` / `peer_tls.rs` — PEM loading, rustls builders | `peer-tls` | the env-prefix constant |
| `metrics.rs` — registry, text encoder, the `/metrics` handler | `metrics-prometheus` + `service-http` | domain metric *declarations* |
| `auth.rs` — bearer extraction, middleware, registry-file loading | `service-auth::role_map` | the domain `Role` mapping and resource decisions |
| `backup.rs` — sinks, upload, retention | `service-backup` | producing consistent snapshot bytes |
| hand-rolled fsync / atomic rename / CRC framing | `storage-durable` | the domain codec |
| a hand-rolled h2c client, or any `bind`/`listen` | `transport-h2c` / `server-http` | — |
| a hand-rolled `llm` / `upgrade` / `issue` | `cli-std` | topic registration |
| a `build.rs` that computes git sha / build time itself | `build-stamp` | the one `stamp("<PREFIX>")` call |
| operator code emitting StatefulSet/Service/PDB/CronJob/NetworkPolicy JSON | `service-k8s::render` | domain default values |

#### Canonical names

Three names are settled here because the drift above is otherwise unresolvable:

- **The HTTP surface is `api/`.** Not `server` — `libs/server-http` is the
  listener owner and an app no longer binds anything, so the name misdescribes
  the layer. Not `http` — a bare protocol is not a responsibility, per the
  naming grammar above.
- **The OpenAPI module is `spec.rs`**, matching the `<cli> spec gen` verb it
  serves. There is exactly one; a second hand-rolled contract module alongside
  a generated one is drift by construction.
- **`dx.rs` is present in every service**, not only the two that happen to
  ship it.

## Service contracts: transport, spec, endpoints, auth, HA

What a service promises to everything outside it — the wire it speaks,
the document that describes it, the operational surface every adopter can
assume, the identity check on every request, and the consistency model
behind them. These are contracts, not implementation choices; a service
that varies here is a service integrators cannot write one client for.

### Transport — h2c + OpenAPI on one port

`axum::serve` speaks HTTP/1 only. To serve h2c (HTTP/2 cleartext, no TLS — the
in-cluster default) alongside HTTP/1.1 on a single port, build the connection
with `hyper-util`'s `auto::Builder`. In-tree clients are `reqwest` over h2c
(rustls, no openssl). Describe the surface with `utoipa` so the OpenAPI doc is
generated from the handlers, never hand-maintained.

### OpenAPI client codegen — typed clients from the spec

*(policy-only — judgment, not trait-enforced; codegen-from-spec discipline
for a service's clients, not itself a separate baseline capability — enforced
transitively via `http2_api`'s `http2-api-list` baseline, which already
requires the service's own OpenAPI surface to exist)*

Because the OpenAPI doc is the source of truth, the typed clients adopters use
are **generated from it**, never hand-written and never produced by an external
tool. The shared `libs/openapi-codegen` (`openapi-codegen`) is the polyglot
core — a language-neutral IR feeding per-language emitters (TypeScript: types +
fetch/axios client + TanStack Query hooks; Python: pydantic + generated
sync/async HTTP/2 runtime that speaks h2c for `http://` and ALPN h2 for
`https://`; Rust: serde + reqwest). A service **composes** it
behind a CLI verb:

```
<cli> spec gen --lang ts|py|rust --out <dir>
```

so an adopter goes from "the service is up" to "a typed client in my language"
with no external codegen step. Reference: `lumen spec gen` (feeds the binary's
own `openapi_json()` into `openapi_codegen::generate`). Do **not** add a
second codegen path — extend the shared crate (a new emitter / capability) so
every service benefits.

### Standard endpoints — one operational surface, one contract three ways

Every service exposes the same five endpoints on its one port, so an operator,
a probe, a scraper, or an agent finds the same surface without per-service
lookup:

```
/healthz       liveness  — k8s livenessProbe
/readyz        readiness — k8s readinessProbe (gates traffic)
/metrics       Prometheus exposition — the ServiceMonitor scrape target
/openapi.json  the machine-readable OpenAPI (utoipa-generated)
/docs          Swagger UI — the human-browsable render of that OpenAPI
```

`/healthz` and `/readyz` are **not optional**: the StatefulSet/Deployment the
operator renders points its probes at them, and `/metrics` is the scrape target —
so all three stay **auth-exempt and always-on** (a 401 on `/healthz` fails the
probe). They are the intersection of the *Transport* and *Deploy* rows.

The integration contract is reachable **three equivalent ways** — same OpenAPI,
different access path:

| Path | Context |
|------|---------|
| **`<cli> spec [--format openapi\|openapi-yaml\|json-schema] [--shapes] [--fields]`** | **offline** (no server) — for agents, CI, codegen input. The offline twin of `/openapi.json`; `spec gen` is its codegen sub-verb. |
| **`/openapi.json`** | served, machine-readable |
| **`/docs`** | served, human-browsable (Swagger UI) |

So `spec` belongs to **this** archetype (a service has an OpenAPI to emit), **not**
`cli-std` — which carries only the universal `llm`/`upgrade`/`issue` that *every*
CLI ships, services and non-services alike. `lumen` is the reference for all of it.

### Service auth — one Bearer-token contract

*(policy-only — judgment, not trait-enforced; candidate for a future
`service_auth` trait once `libs/service-auth` adoption is mechanically
checkable)*

Service auth is shared infrastructure, not a per-project design space. Every
long-running service uses `libs/service-auth` for request authentication:
extract `Authorization: Bearer <token>`, verify it through a service-supplied
`Verifier`, reject with the shared JSON error shape, and inject the authenticated
principal into handlers. Services use the shared registry verifier
(`service_auth::role_map::StaticRoleMapVerifier` — role hierarchy, wildcard
grants, registry-file loader) or signed tokens through `libs/claim-token`, but
the HTTP contract and middleware shape stay the same.

Production server config follows one env pattern:

```
<SVC>_AUTH=required
<SVC>_TOKEN_REGISTRY_FILE=/var/run/secrets/<svc>/token-registry.json
```

Local/dev may use `<SVC>_AUTH=off`. Production must not depend on inline token
lists as the primary path. In Kubernetes or cloud-native deployments, the
operator/render layer mounts the registry file from a Kubernetes Secret, CSI
Secret Store, or cloud Secret Manager sync, and owns the env wiring. Clients do
not need connection strings for auth; they use `<SVC>_URL` for routing and
`<SVC>_TOKEN` for credentials, then send `Authorization: Bearer <token>`.

**The registry's map key is never the credential.** A document keyed by bearer
secret cannot be read by anyone not also entitled to authenticate as everyone in
it, so the permission table — the thing an operator most needs to review —
inherits the blast radius of the secrets. The registry carries **two disjoint
key namespaces**: `tokens` keys are bearer secrets, `identities` keys are
provider-verified subjects (a Google email, a Kubernetes ServiceAccount). A
presented bearer value resolves against `tokens` only and a verified identity
against `identities` only, so a secret whose text happens to be a valid email
can never inherit that email's grants, and an `identities`-only registry holds
no credential at all — it is ordinary reviewable configuration.

A section-less document stays valid and is read entirely as `tokens`, which is
what lets a deployed registry survive the change untouched. **The
discriminator has two clauses, and the second is not optional**: a document is
sectioned only when every top-level key is `tokens` or `identities` *and* no
top-level value carries a `subject` field. Without the second clause a flat
registry whose one secret is literally spelled `tokens` is read as a section
and silently loses its only credential. Any second decoder — a CLI resolving a
dev token, a migration tool — must replicate both clauses and pin the copy with
a test; `libs/cli-std`'s `connect::bearer_secrets` is the reference, duplicated
rather than shared because `service-auth` depends on `cli-std` and not the
reverse.

**Authentication is on unless a CR asks for it off, and there is exactly one
place a registry can come from.** The operator's `spec.auth` defaults to
`required`, not to off — a field that fails open is a field that ships
unauthenticated whenever someone forgets it. When both a Kubernetes Secret and
a CSI `SecretProviderClass` name a registry, reject the CR with an
`x-kubernetes-validations` CEL rule at `kubectl apply` instead of resolving it
with a documented precedence: with a silent winner there is no way to tell
which registry is actually being served, and the loser looks applied.

Two spellings that look interchangeable and are not: the CRD enum variant is
**`disabled`**, the process env value is **`off`**. Kubernetes parses CRs as
YAML 1.1, where a bare `off` is the boolean `false` — so a `type: string` field
defaulting to `"off"` rejects the very value its own description tells readers
to write, and the error names a type mismatch rather than the missing quotes. A
free-string auth field is worse still: `auth: requred` is accepted and read as
"not required", which turns a typo into an open service. Use a unit enum
(`#[serde(rename = "disabled")]`) — structural schemas represent those fine;
it is only enums with divergent per-variant schemas they cannot express.

Keep the boundary explicit: `libs/service-auth` authenticates callers;
service handlers authorize per resource, tenant, collection, queue, workflow, or
admin action. Standard probe/spec/scrape endpoints stay auth-exempt according to
the standard-endpoint contract above.

### HA — `raft-core`, sharded and strongly consistent

**HA is mandatory for any stateful service — not a "wire it later" slice.** A
DTO / cluster-state surface (`/debug/cluster`) without the `raft-core` replication
path actually wired does **not** satisfy the HA row; the service is not
production-ready until writes are ordered and replicated through `raft-core`.

State is **sharded** and **strongly consistent**, replicated by the shared
`libs/raft-core` engine (serde-only; it replaced `openraft` across the
ecosystem). The leader owns writes; followers tail it over h2c. Node identity
and the peer set come from the Kubernetes **downward API** on a StatefulSet —
nothing is hand-configured per replica. Gate consensus behind a Cargo feature
only when a single-node mode is a legitimate deployment (e.g. `keep`).

A stateful sharded service scales storage and compute along two independent,
autonomous axes: storage ownership grows through disk-usage-driven shard
splits, compute capacity grows through CPU-driven replica/HPA scaling, and
neither axis has a human gate. HPA owns compute scaling only — it never
changes shard ownership or storage topology.

## Deployment: artifacts, tenancy, and the control/data split

How the thing reaches a cluster, who it shares that cluster with, and
which decisions belong to the operator rather than the pod.

### Deploy artifacts — image, cluster API, operator, instance

Image construction is not a Kubernetes subcommand. Every k8s-native service CLI
ships:

```
<cli> dockerfile render --variant source|release [--version <tag>] [--out <path-or-dir>]
```

`source` renders the workspace-build Dockerfile used by dev/CI; `release`
renders the small production image that fetches a published `<project>@<version>`
binary. The same image artifact feeds compose, kind, and real registries.

Kubernetes output is split by lifecycle layer:

```
<cli> k8s crd render [--out <path>]
<cli> k8s operator render [--namespace <ns>] [--out <path-or-dir>]
<cli> k8s operator run
<cli> k8s instance render --profile dev|staging|prod|template [--out <path-or-dir>]
```

`crd` owns the cluster-scoped API. `operator` owns the control plane, normally in
an independent namespace such as `<svc>-system`; `operator run` is the controller
process/container entrypoint. `instance` renders the app-namespace custom
resource that an application team applies next to the app it integrates with.

**Every manifest checked in under `k8s/` must be `include_str!`-gated against
its renderer.** A test that renders into a temp dir, or asserts against the
library function the renderer calls, proves the *renderer* — it never reads the
file a `kubectl apply -k` user actually applies, so that file can silently rot
for releases. `lumen` shipped exactly that: `k8s/operator/crd.yaml` had drifted
to carry no `x-kubernetes-validations` at all, meaning a new CEL rule passed its
own unit test while the applied CRD still accepted what the rule forbade. The
gate is one test comparing `include_str!("../k8s/…")` byte-for-byte to the
render function — sound because `cli_std::artifact::write_or_print` writes the
body verbatim, so `--out` reproduces the renderer's output exactly.

### Deploy tenancy — dedicated first, shared only when justified

*(policy-only — judgment, not trait-enforced)*

The service archetype is **dedicated-first**. A service must be able to run as
its own data plane — one service instance, one app namespace or service-owned
namespace, one primary workload selected by profile, and, for stateful
services, one storage/backup surface,
and one operational SLO envelope. This dedicated/standalone mode is mandatory
because it is the simplest reliable production shape: ownership, upgrades,
backups, failure blast radius, and delete/finalizer behavior are all local to the
service instance.

Shared multi-tenant backends are a separate platform capability, not the default.
They are appropriate only when the product explicitly needs many small tenants to
share a physical data plane and the platform is ready to own the extra control
loops. A shared mode requires placement, metering, quota/rate-limit enforcement,
tenant identity, backend capacity accounting, promotion/demotion policy,
migration, endpoint/secret rotation, backup partitioning, and finalizer semantics.
Those moving parts materially increase operational complexity, and most service
deployments do not need them.

When a shared mode exists, keep the API resource paths service-domain scoped, not
Kubernetes-namespace scoped. Namespace is deployment and service-discovery
context; it belongs in DNS, RBAC, CR metadata, and status endpoints, not in HTTP
paths. For example a tenant may reach
`http://lumen.lumen-shared.svc/collections/users/index`, while the HTTP route
remains `POST /collections/{collection_id}/index`.

Use this default decision rule:

| Mode | Default? | Use when |
|------|----------|----------|
| **Dedicated / standalone** | Yes | Most production services, high isolation, simple ownership, clear backup/restore and SLO boundaries. |
| **Shared backend** | No | A platform team explicitly owns placement, metering, quotas, migration, endpoint switching, and tenant lifecycle. |
| **Promote to dedicated** | Optional | A shared tenant exceeds sustained usage, SLO, storage, or isolation thresholds and migration is controlled by policy. |

## Operations baseline — notice the breach, watch the watcher, answer "converged?"

*(policy-only — judgment, not trait-enforced; promote to a capability trait once
`libs/service-k8s` can render and validate these mechanically.)*

**Instrumentation is not operations.** The *Observability / trace context* row
above buys per-request spans, correlation, and counters — the raw signal. A
service that stops there has a perfect record of an outage nobody was told
about. This section is the other half: something must *notice* a breach, the
thing doing the reconciling must be observable *itself*, and a deployer must be
able to ask "is it converged?" without reading logs.

Every row is mandatory for a Kubernetes-native service, and adoption is
currently uneven across `lumen`/`tape`/`relay`/`keep`/`defer`.

**Roll out through `lumen` first**, then in this order:

```text
lumen → tape → defer → relay → keep → sift → (remaining services)
```

`lumen` leads because it is already the reference adopter for standard
endpoints, HA, and `prometheus_rule`, and because it is the service with live
integrators — the only one where a row gets exercised against real traffic
rather than an acceptance harness. A row is proven in `lumen`, lands its
mechanism in `libs/service-k8s`, and only then moves down the order. **Do not
open the same row in several services in parallel**: the second adopter's job is
to compose the shared mechanism, not to re-derive it. When a row needs a
mechanism, **extend `libs/service-k8s` so every adopter gets it once** — never
fork the pattern into one project.

Leading the order does not mean inventing every row. Some rows already shipped
in a service further down it before `lumen` reached them — the `network-policy`
component originated in `defer`, `relay`, and `tape`, and `replicas: 2` in
`defer`'s operator. Where that is true, `lumen`'s job is to **adopt the existing
pattern and pull it into `libs/service-k8s`**, not to write a second one. Check
the other services for the row before authoring it.

As of 2026-07-27, `lumen` has closed four rows and pulled each mechanism into
the shared layer: **control-plane observability** (#2620/#2621),
**`status.conditions[]`** (#2601), **network isolation** (#2603), and **operator
HA** (#2602). For the next adopter those four are no longer authoring work —
`libs/service-k8s` renders them and the job is composition plus a cluster proof.
Rows still open everywhere: **SLO-shaped latency alerting**, **both scaling
axes**, and **verifiable release artifacts**.

`sift` (in `projects/sift`, not `apps/`) sits late in that order for a
structural reason, not a priority one. Like `lumen` it is something other
services integrate *with*, but the coupling is a **one-way versioned schema** —
emitters write `axiom.service.log.v1` to stdout
(`libs/service-observability/contracts/axiom.service.log.v1.schema.json`) and
the sift collector ingests it; nobody calls sift on the request path. So
`sift`'s own operations rollout blocks no emitter, and no emitter's rollout
blocks `sift` — it can move out of order whenever it is convenient. One row is
an exception to its late slot: **control-plane observability lands in `sift`
early**, because a blind operational-event platform is the single failure that
hides every other service's failure.

| Dimension | Requirement | Reference / gotcha |
|-----------|-------------|--------------------|
| **Alert rules** | The operator renders a **`PrometheusRule`** beside its `ServiceMonitor`, behind the same `spec.observability` switch. Baseline for a stateful service: no ready replicas, pod crash-looping, PVC near full, scheduled backup job failing, storage degraded. Every alert carries a **`runbook` annotation naming the first command to run**. | `lumen`'s `operator/render.rs` `prometheus_rule` is the reference. **An alert on a series nothing publishes is worse than no alert** — if the signal lives in the CR's `status`, it needs a `customresourcestate` config or a driver-side gauge before the alert means anything (see `LumenReshardWorkflowStalled`, which can only read a fence gauge and misses `PrepareSplit`/`Splitting` stalls entirely). |
| **Early warning before the wall** | Any resource with a hard limit (disk, quota, connection budget) pairs its **degraded** alert with an **approaching** alert that fires earlier. | A degraded alert alone only tells you the outage already started. `LumenPvcNearFull` (10% free, 10m) is the early warning `LumenStorageDegraded`'s own runbook cross-references. |
| **SLO-shaped latency alerting** | Where a latency **histogram** exists, alert on quantiles or error-budget burn rate — not on a static-threshold counter. | A histogram referenced only from a runbook *string* is instrumentation nobody automated: `lumen_search_latency_seconds_bucket` ships, yet the only latency alert is `rate(lumen_slow_queries_total[5m]) > 0.1`. |
| **Control-plane observability** | The operator exposes its **own `/metrics`** (reconcile attempts/failures/duration, leader-election state), ships its own `ServiceMonitor` in `<svc>-system`, emits Kubernetes **`Event`s** for reconcile decisions and failures, and owns at least two alerts: **operator absent** and **reconcile error rate non-zero**. | Shipped in `libs/service-k8s`'s shared controller, not per app: `metrics::ControllerMetrics` + `metrics::serve` (listener address from `OPERATOR_METRICS_ADDR`, default `0.0.0.0:9090`) and a `kube::runtime::events::Recorder` wired into `controller::run`. `lumen` is the reference adopter (#2620/#2621) — the per-app work is the `ServiceMonitor` and the two alerts, not the mechanism. **Data-plane alerts cannot see a control plane that stopped reconciling** — every data-plane alert reading green is exactly what a wedged operator looks like, and `kubectl describe <cr>` with no Events cannot tell a deployer why nothing happened. Note the Recorder's **6-minute dedup window**: a tight reconcile-fail loop emits one Event, so the failure *counter* is the alerting signal and the Event is the explanation. |
| **Status is the convergence API** | `status` carries `observedGeneration`, `additionalPrinterColumns` for the fields a human greps, and a standard **`conditions[]`** array (`metav1.Condition` shape: `type`/`status`/`reason`/`message`/`lastTransitionTime`/`observedGeneration`) with at least `Ready` and `Progressing`. | Without `conditions[]`, **`kubectl wait --for=condition=Ready` has nothing to read** and Argo/Flux cannot assess health without a bespoke Lua hook. A custom `phase` string is for humans; `conditions[]` is for automation. Both are needed — they are not substitutes. The shared shape is `service_k8s::service::{Condition, ConditionFact, ConditionStatus}`; `lumen` is the reference adopter (#2601), emitting `Ready`, `Progressing`, and a domain condition. **`status_patch` is a pure function by contract, so it must not read a clock** — a service returns clock-free `ConditionFact`s and the controller stamps `lastTransitionTime`, carrying the prior value forward when `status` is unchanged. Get that backwards and every reconcile rewrites the timestamp, making a stuck condition look freshly flipped. Note also that a merge patch **replaces the whole array**, so the projection must re-emit conditions it did not change. |
| **Network isolation ships with the instance** | Every service ships a **`k8s/components/network-policy`** kustomize component: default-deny ingress plus explicit allows for client traffic, peer/consensus traffic, and metrics scrape. | `defer`, `relay`, and `tape` are the original hand-written components; the shared renderer is now `service_k8s::render::common::network_policy`, adopted by `lumen` (#2603) for the operator-rendered path a CR user gets without kustomize. It is a **component, not a base resource** — a cluster whose CNI does not enforce NetworkPolicy opts out by not composing it, instead of silently believing it is isolated. Two traps: the controller's `plural_for()` fallback pluralizes to `networkpolicys`, so the kind needs an explicit `"NetworkPolicy" => "networkpolicies"` arm; and **kind's default `kindnet` CNI does not enforce NetworkPolicy at all**, so a kind run proves the object renders and applies, never that traffic is blocked. |
| **Operator HA is on by default** | A leader-elected operator's rendered Deployment defaults to **`replicas: 2`**. | Shipping the lease (`libs/service-k8s::lease`) and then `replicas: 1` means a node drain stops reconciliation until a fresh Pod schedules — the failover machinery is built and switched off. `defer` and `lumen` (#2602) both ship it. **`replicas: 2` without a `PodDisruptionBudget` is still one drain away from zero** — the pair needs a PDB keeping one Pod up (`maxUnavailable: 1`, or the equivalent `minAvailable: 1` at two replicas), or the node-drain case the second replica exists for evicts both at once. |
| **Both scaling axes stay autonomous** | See *HA* above: storage grows by disk-driven shard split, compute by CPU-driven replica scaling, neither gated on a human. | **A CRD field that still advertises autoscaling bounds while the renderer has stopped emitting the autoscaler is a defect in both directions** — either wire the replacement policy or remove the field. An inert knob whose doc comment promises elasticity is worse than an honest absence. |
| **Release artifacts are verifiable** | Published container images carry a **cosign signature, an SBOM, and build provenance**; the release workflow produces them and the deploy handoff names the verification command. | A sha256 on the release tarball proves the tarball — it says nothing about the image a cluster actually pulls. Without attestation, no admission policy can enforce "only run images we built". |

## Conformance: what proves a service actually complies

Every rule above is a claim until something fails on it. Three mechanisms
do the failing: the `e2e/` suite for externally observable behavior, EC gates
for evidence-backed behavior, and the dogfood rules for the surfaces a test
suite never exercises. Architecture and profile shape is the fourth question,
and nothing runs it any more — the catalogue below is a hand-applied checklist,
not a gate.

### Test layout — `e2e/*.rs`, explicitly declared, no autodiscovery

Every `apps/<p>` and `libs/<p>` crate owes an `e2e/` tree. It is where
externally observable behavior is judged, it is written in Rust, and it runs
under plain `cargo test -p <crate>`.

| Where | What lives there | How it runs |
|---|---|---|
| `{apps,libs}/<p>/e2e/*.rs` | externally observable behavior, one file per case | `cargo test -p <crate>` |
| `{apps,libs}/<p>/src/**` | rules observable only inside the implementation, colocated | `cargo test -p <crate> --lib` |

Two rules make the tree honest:

- **Declare every case.** Set `autotests = false` in the crate's `Cargo.toml`
  and give each `e2e/*.rs` its own `[[test]]` stanza whose `path` is
  `e2e/<name>.rs`. With autodiscovery off the manifest is the inventory: what
  runs is a reviewable list in `Cargo.toml`, not whatever happens to be sitting
  in the directory. An undeclared file is not built at all, so adding or
  renaming a case has to show up in the diff to take effect.
  `libs/service-http` is the reference shape: eight `e2e/*.rs`, eight
  `[[test]]` stanzas, `autotests = false`.
- **Write the case first.** The `e2e/*.rs` case lands before the `src/**`
  change it judges, and must be observed failing for the stated reason before
  the change makes it pass. A case written afterwards is fitted to the
  implementation it was supposed to constrain.

`{apps,libs}/<p>/tech-design/`, `{apps,libs}/<p>/external-contracts/`, and
`{apps,libs}/<p>/tests/` are superseded: no authored source belongs in them.
Do not create one, do not add a file to one, and migrate a surviving case into
`e2e/` rather than editing it in place. The Python spec model and its
`src/cases/*.py` verifiers are retired across the repository — never author a
new one. The only thing that may still appear under `external-contracts/` is
generated evidence written by an EC gate run, which is output, never contract.

### Architecture/profile conformance checklist

*(policy-only, and unenforced — read the paragraph below before using this
table as a gate.)*

A project's reference profile shape (kind/surface, workload, state ownership,
replication, serving role) implies the conformance findings catalogued below:
shared-service-kit adoption gaps, profile-shape negative assertions, and
observability/Raft telemetry gaps.

**Nothing runs this catalogue.** It was `aw review --project <project>`, spliced
in from two live rule registries and drift-tested against them; the command, the
registries, and the drift test are all deleted. What is left is a hand-applied
checklist — useful when reviewing a service against the archetype, and worth
nothing as evidence. A rule below that a project violates produces no finding,
no exit code, and no signal of any kind. Do not cite one as a gate in a
`## Acceptance` table.
| Rule ID | Family | Fires when |
|---|---|---|
| `shared-kit:server-tcp` | `shared-kit` | served-surface bind/listen setup |
| `shared-kit:server-lifecycle` | `shared-kit` | retry/backoff and health-check plumbing |
| `shared-kit:service-observability` | `shared-kit` | structured logging/metrics setup |
| `shared-kit:raft` | `shared-kit` | raft consensus/leader-ingest plumbing |
| `negative-assertion:pgpool:statefulset-shape` | `negative-assertion` | Deployment/ExternalState profile carries StatefulSet/PVC/headless-service manifest content or a raft dependency |
| `negative-assertion:tape:raft-or-primary-replica-signal` | `negative-assertion` | StatefulSet/ReplicatedLog profile carries raft-dependency or primary/replica-role source markers |
| `negative-assertion:relay-defer:passive-replica-signal` | `negative-assertion` | StatefulSet/RaftConsensus profile carries a primary_replicas trait or primary/replica-role source markers |
| `negative-assertion:lumen:raft-leader-ingest-signal` | `negative-assertion` | StatefulSet/PrimaryReplica profile carries a raft dependency plus leader-ingest source markers |
| `obs:structured-logging-metrics-adoption` | `obs` | service surface has not adopted libs/service-observability for structured logging/metrics/correlation |
| `obs:w3c-context-propagation-adoption` | `obs` | service surface has not adopted libs/service-http or libs/transport-h2c for W3C trace-context (traceparent) propagation |
| `raft:proposal-routing-telemetry-gap` | `raft` | raft leader-ingest surface has no proposal-routing telemetry (local-vs-forwarded proposal counts, forward duration, or forwarded bytes) |
| `raft:leader-route-and-replication-lag-telemetry-gap` | `raft` | raft leader-ingest surface has no leader-route or replication-lag telemetry (leader-route retries/changes, commit/applied lag, or peer RPC visibility) |
| `raft:high-cardinality-label-antipattern` | `raft` | a metric-emission site carries a high-cardinality label key (queue/topic/message_id/message) in the same file as a raft telemetry metric |
| `raft:trace-context-continuity-gap` | `raft` | raft leader-ingest surface has no trace-context continuity evidence (traceparent/trace_id/span_id propagation or tracing::instrument/info_span! spans) across the forwarded-proposal path |
| `raft:follower-local-mutation-outside-consensus` | `raft` | a follower/replica-role marker co-occurs with an explicit consensus-bypass marker in the same file -- a follower appears to mutate local state outside raft consensus |
| `raft:loss-of-leader-fail-open-bypass` | `raft` | an explicit fail-open marker allows accepting writes without a confirmed leader / bypassing the quorum check on loss-of-leader |
The split this catalogue sat in is worth keeping even without the commands that
implemented it: **readiness, gates, and production-blocker status** are one
question, and **architecture/profile shape** is another. They never substitute
for each other, so a project that satisfies its gates has said nothing about
whether it matches the archetype.

### EC gates — `vat`-driven, evidence under `external-contracts/`

Every service carries a fixed set of **evidence-contract (EC) gate files**, each
`SPEC-MANAGED` and pointed at a contract under `external-contracts/`. They are
named so `ls` tells you what is enforced before you open anything:

- **`vat.toml`** — the EC test runner; backs integration tests with **real
  services / emulators** (never hand-rolled mocks) and lists the setup steps.
- **`meter*.toml`** — performance/efficiency/stability EC gates
  (`meter.toml` + `meter-<scope>-<dimension>.toml`, e.g.
  `meter-keep-performance.toml`, `meter-search-{efficiency,stability}.toml`),
  run via `vat run meter-*`; evidence under
  `external-contracts/competitor-performance/`.
- **`guard*.toml`** — the security EC gate (`guard-<scope>-security.toml`),
  run via `vat run guard-security`; evidence under
  `external-contracts/security-hardening/`.

A breach is a non-zero-exit finding, and it blocks the phase that runs the gate.
The `SPEC-MANAGED` marking on these files named a regenerator that no longer
exists: nothing regenerates an `AW-EC-TOOL` block today, so the `.toml` is the
authoring surface and editing the contract it cites propagates nowhere.

These `vat.toml` / `meter*.toml` / `guard*.toml` gate files are runner
configuration and they survive; what they write under `external-contracts/` is
generated evidence. The Python EC *project* they used to point at —
`pyproject.toml` as inventory plus `src/cases/*.py` verifiers — does not
survive. Behavioral assertions live in `e2e/*.rs` per
[Test layout](#test-layout-e2ers-explicitly-declared-no-autodiscovery). Never
author a new Python EC case, in any project including Agentic Workflow itself.

### Service dogfood rules — keep the whole surface honest

*(policy-only — judgment, not trait-enforced)*

Recent service hardening work exposed a repeated failure mode: one slice moves to
the new archetype while build scripts, CRDs, k8s overlays, tests, or EC gates
still point at the old backend. Treat the following as contract, not cleanup
advice:

- **One active data plane.** A service may keep legacy compatibility code only
  when it is explicitly labelled legacy and still tested. The production build,
  Dockerfiles, release workflow, `build.sh`, k8s manifests, operator render
  path, examples, README/HA docs, perf modes, and EC gates must all name the
  same active data plane. Retired backends must not remain in active gates.
- **Shared capability before service-local code.** Cross-service mechanics
  belong in `libs/*`, not inside one project's `src/`: h2c transport, probe and
  metrics routes, auth extraction and role maps, raft hosting, backup
  destination/policy runners, operator rendering, build stamping, CLI
  conventions, and generated client codegen. A service may own product-specific
  state transitions and API shape, but if two services need the same mechanism,
  the mechanism is a library capability with service adopters.
- **State-owning production is durable-only.** The StatefulSet profile does not
  have a volatile production mode. Accepted service-owned writes must survive
  process restart and pod reschedule, and replicated services keep raft/log
  replay on the write path. Deployment-profile proxies may be stateless, but
  must identify their downstream durability boundary and prove drain/reconnect
  behavior instead of claiming local persistence.
- **Direct install is not the HA story.** Kustomize `base` / overlays should be a
  small direct install for kind and smoke tests. Stateful production HA goes
  through the operator CR path that renders StatefulSet topology and the
  downward-API env consumed by `raft-host`; Deployment-profile availability
  goes through replica, disruption, rollout, drain, and external-capacity
  policy instead.
- **Operator owns lifecycle, not bytes.** The operator creates RBAC,
  ServiceAccounts, Services, StatefulSets/Deployments, PDBs, CronJobs, Secrets,
  status, and finalizers. It does not serialize service data. Snapshot bytes are
  produced by the service state machine/admin surface; `raft-host` installs
  snapshots and compacts logs; `libs/service-backup` runners upload/prune them.
- **Scheduled object snapshots are baseline.** A production stateful service CR
  must render or reference a periodic backup job that writes consistent
  snapshots to object storage. Local filesystem sinks are for development,
  migration handoffs, or explicit break-glass recovery; they do not satisfy the
  production service archetype. The backup path is independent of live replica
  sync: raft/log replication keeps peers current, object snapshots provide cold
  disaster recovery and empty-PVC/bootstrap seeds.
- **Namespace is deployment context.** The service instance CR lives with the app
  namespace unless a platform team intentionally separates app and backend
  lifecycle. Operator/control-plane resources normally live in `<svc>-system`.
  HTTP paths stay service-domain scoped; do not add Kubernetes namespace names to
  public routes.
- **CRDs must be Kubernetes OpenAPI compatible.** Generated CRDs are not done
  until `kustomize build` and CRD render pass. Normalize schema details that
  Kubernetes rejects, such as `format: uint32` / `format: uint64`; express
  unsigned integer intent as `type: integer` plus `minimum: 0`.
- **Rustls provider is binary startup work.** Any binary that links rustls-backed
  clients, including operator mode or raft/online CLI paths, installs the
  process-level crypto provider before parsing commands or starting async work.
- **Kind gates exercise the current service, not retired dependencies.** A
  service dogfood script must not build or deploy a retired external component
  just because old manifests still mention it. If the current kind gate is
  intentionally single-node, say so and keep replay/HA claims in separate gates.
- **Peer-service benchmarks are calibration, not every-run work.** Once
  Postgres/OpenSearch or similar competitor baselines are captured, regular
  production gates should run the service-only regression path against retained
  floors. Rerun peers only for explicit recalibration or new comparison rows.
- **Tune for volume, not toy traffic.** The service archetype standardizes on
  HTTP/2 because the product target is sustained high QPS and large working
  sets over multiplexed connections. Low-QPS / single-client loopback rows are
  smoke tests and regression diagnostics: they expose fixed overhead and broken
  fast paths, but they are not the win condition. Do not contort a service to
  beat a competitor on tiny cheap requests if doing so weakens the high-volume
  path. Release-relevant performance claims should center throughput, tail
  latency, RSS/footprint, recovery behavior, and peer comparison at enough
  concurrency and corpus size for HTTP/2 pooling, sharding, mmap, batching, or
  raft/log amortization to matter.

## CLI convention: every CLI ships `llm`, `upgrade`, `issue`

> Every binary a human or agent runs must answer three questions without prior
> knowledge: *how do I drive this?* (`llm`), *am I current?* (`upgrade`), and
> *what's broken — what's already reported, and how do I file it?* (`issue`).
> These three are
> **mandatory** on every CLI surface in the ecosystem (`mamba`, `jet`, `lumen`,
> `vat`, `aw`/`cclab`, and any new tool) — the agent-facing contract that lets a
> tool an agent has never seen self-onboard, self-update, and file a structured
> defect using the binary alone.

A new CLI is not "done" until all three appear in `--help`.

**Positionals are for subcommands and a verb's one primary object; structured
parameters are flags.** A positional names a *subcommand* (`jet build`, `jet
issue search`) or the verb's single natural object — an id, a query, or free-form
prose (`issue view <n>`, `issue search [query]`, `issue create [msg…]`). Anything
the command *selects or configures* — a topic, title, version, tag, state — is a
named flag (`--topic`, `--title`, `--version`, `--state`), so the grammar stays
unambiguous as the surface grows.

| Subcommand | Signature | Contract |
|------------|-----------|----------|
| `llm` | `<cli> llm [--topic <topic>] [--format md\|json]` | Offline (no server/network) docs that teach an agent to drive the tool. Topic via `--topic` (not positional); default `outline` (a topic map); per-tool topics follow its domain. Markdown default, `--format json` for machine-readable. |
| `upgrade` | `<cli> upgrade [--version <tag>] [--check]` | Self-update to the latest `<project>@*` GitHub release. `--check` = report whether newer exists, no install; `--version` = pin a tag. |
| `issue` | `<cli> issue search [query]` · `view <n>` · `create [--title <t>] [msg…]` | Read **and** write the tool's issues on the tracker. `search` finds this tool's issues (filtered to `app:<name>`; omit the query to list recent), `view <n>` prints one, `create` files a structured issue (auto-attaching `--version` + OS/arch + context, tagged with the `app:<name>` label). |

`llm` is a **cross-scope agent index**, not a feature-local doc page. It must
teach the smallest command/topic an agent should read across the tool's real
operating scopes: CLI grammar, workflow, API/spec, auth, deployment,
operator/k8s, storage/backup, data movement, integration adapters, diagnostics,
and domain recipes. When a change alters any public command, generated
artifact, API contract, operator/CRD field, deployment topology, auth model,
backup/import/export path, or issue/upgrade flow, the change must either update
the relevant `llm --topic ...` body or explicitly verify that the existing topic
already teaches the new behavior. Treat a stale `llm` topic as a public CLI
regression.

`llm` topics should be assembled from one in-code topic registry, not from
duplicated hand-written command lists. The default `outline` is generated from
that registry and is the cross-scope table of contents; detail topics own their
domain body. Tests should lock both levels: the outline advertises every topic
with the convention-canonical `--topic <id>` flag, and every advertised command
parses through the real binary. For services, add focused topic assertions for
the scopes the service exposes (for example deployment topology, storage/backup,
auth, integration, and query recipes) so cross-scope drift is caught by the same
patch that introduces it.

Internal libraries may contribute agent-facing `llm` fragments, but they do not
become standalone user-facing CLIs just to publish docs. A reusable lib with an
operational contract agents need to understand (for example `libs/service-k8s`,
`libs/raft-runtime`, `libs/service-backup`, `libs/service-auth`, `libs/transport-h2c`, or
`libs/openapi-codegen`) should expose a small `cli_std::llm::Topic` provider or
constructor from its Rust API. The consuming project decides whether that topic
belongs in its own `llm` registry, may wrap or prefix the id to fit the
project's vocabulary, and may omit irrelevant library topics. This keeps the
source of shared behavior close to the library while keeping the final
agent-facing outline scoped to the actual tool the agent is driving.

Library-contributed topics must describe the library contract, not the consuming
project's product policy. Project topics own final choices such as "Lumen uses
three raft voters" or "this CRD exposes `spec.serving.backup`"; library topics
own reusable mechanics such as raft ordinal math, backup destination semantics,
auth registry shape, or operator render layers. When a library contract changes,
update its topic provider and at least one consuming project's `llm` regression
test if that project exposes the contract.

Implementation notes not obvious from the signature:

The logic for all three lives in the shared **`libs/cli-std`** crate (`cli_std`),
which is **clap-agnostic**: each CLI keeps its own clap registration — so it owns
the convention's flag shape (`--topic`, not a positional) — and delegates the
behavior to the crate, parameterized by a `cli_std::ToolInfo` it fills from its
own `build.rs` stamps (project, repo, target triple, version, git sha — emit
them with `libs/build-stamp`'s `stamp("<PREFIX>")`, not a hand-rolled
`build.rs`). A tool
provides only its clap surface, that `ToolInfo`, and (for `llm`) its topic list;
the crate does the rest. The network paths (`upgrade` install, `issue`
search/view/create) sit behind cli-std's `online` feature — enable it in release
builds. Reference adopters: `apps/jet` and `apps/lumen`.

- **`llm`** — `cli_std::llm::render(project, version, topics, topic, format)`. The
  tool supplies `&[cli_std::llm::Topic]` (`id`/`summary`/`body` — the one in-code
  source of truth) and cli-std renders the `outline` topic map + the
  standard-command footer. Pure offline; always builds.
- **`upgrade`** — `cli_std::upgrade::run(&tool, opts)`: the in-binary form of
  `projects/<project>/install.sh` — detect target (`<arch>-<os>`) → download the
  matching `*.tar.gz` → verify `.sha256` → **atomically** replace the binary.
  Fail loudly on checksum mismatch; never leave a half-written binary.
- **`issue`** — `cli_std::issue::{search, view, create}`. `search`/`view` are
  read-only GitHub API GETs (tokenless on public repos); `create` submits via the
  API when `GITHUB_TOKEN` is set, else prints a pre-filled `issues/new` URL. Pass
  the tracker's `app:<name>` label in `CreateOptions.label` so it is applied
  on submit **and** carried into the URL fallback's `&labels=`; `search` filters
  to that same label. The group is named `issue` (**not** `report`), leaving
  domain `report` verbs (`jet report` = HTML **test** reports) untouched.

A fourth verb, **`connect`**, is a convention for **k8s-native service CLIs**
specifically (not every CLI — see *Deploy artifacts — image, cluster API,
operator, instance* above for the `kubernetes_native` project baseline):
`<cli> connect --namespace <ns>
(--service <svc> | --cr <cr-name>) [--secret <secret>] -- <cmd>...` spawns a
`kubectl port-forward` for the duration of a wrapped command and tears it down
(kill + wait) on exit regardless of the wrapped command's status, resolving a
bearer token from a token-registry Secret when one is in play. Its
implementation home is `cli_std::connect` (`libs/cli-std/src/connect.rs`,
behind the `k8s` feature): the port-forward process lifecycle (`ChildGuard`,
`free_local_port`, `wait_for_local_port_ready`) and the token-registry Secret
resolution chain (`kubectl_get_json`, `cr_tokens_secret`,
`resolve_cr_tokens_secret`, `secret_data_bytes`, `select_token`,
`resolve_token`) are universal to any k8s-native service CLI — a tool adopts
`connect` by supplying only its own flag surface, its CR-kind lookup
convention (the `resource_kind` string passed to `resolve_cr_tokens_secret`),
and a role mapping into `cli_std::connect::Role`. `apps/lumen`
(`lumen connect`, extracted #1321/#1376) is the reference adopter; keep/relay/
loom/beam adopting `connect` is tracked as follow-up work per project, not
required by this convention alone.

## CLI convention: stdout tells the agent the next step

> Every CLI's machine-readable output must tell the agent what happens next —
> a runnable command, or an explicit "done" — so the agent never has to guess
> **what now?** after a command finishes.

A CLI's structured/terminal output (its JSON payload or fixed final stdout
line — not incidental logs) **MUST** carry either (a) `next` — a command
string the agent can execute verbatim, or (b) an explicit terminal marker
meaning "done — report completion to the user." There is no third state: an
output that is neither runnable nor terminal is a defect. Errors **MUST**
carry a remediation next step too; the runnable-or-terminal rule applies on
the failure path exactly as it does on success.

Emitted commands **MUST** be executable, not aspirational: multi-level verbs
must exist on the real clap surface and chain-required positionals/flags must
be present. A `next` that doesn't resolve against the binary's own `--help`
is worse than no `next` at all.

The working example in this repo is the `aw` plugin's phase scripts. Every one
of `e2e.py` and `impl.py` ends by printing the command that follows it, and the
last phase prints that the change is closed instead — so an agent driving the
ladder never has to infer the next verb. That is the convention in its
simplest conforming form: a fixed trailing stdout line, no envelope.

The repo used to carry a stronger version of this, and it is worth naming what
was lost. The `aw` CLI emitted an `aw.cli.v1` envelope, and a `chain.rs`
registry re-parsed every command string it could emit back through the real
clap tree, so an unrunnable `next` was a build failure rather than a runtime
surprise. That CLI is deleted and nothing replaced the check: **no gate today
asserts that a printed next command is runnable.** A phase script that prints a
typo prints it green.

Simple CLIs without a full envelope **MAY** use the lighter conforming form —
a single `next` field in JSON output, or a fixed trailing stdout line. The
wire format is free; the runnable-or-terminal semantics are not.

Completeness and chainability measure different things, and satisfying one says
nothing about the other. *Complete* (是否做完) asks whether a project's
capability, gate, and traceability coverage is finished; this convention asks
whether one command's output is *executable by the next agent* (轉不轉得動 —
handoff, not completeness). A CLI can be finished by every coverage measure and
still emit unchainable output, and vice versa.

## DX convention: every service and CLI ships a Developer & Agent Experience capability

> An agent's first contact with a tool is offline (docs, schemas, `--help`)
> long before it is online (a live connection). This convention makes that
> first contact a **capability** — with the same gate discipline as any other
> product surface — instead of incidental documentation nobody tests.

Every service-archetype adopter (see "Service archetype" above) and every
ecosystem CLI (see "CLI convention: every CLI ships `llm`, `upgrade`,
`issue`" above) **MUST** own one `Developer & Agent Experience` capability
(`Type: AgentFirst`) in its README `## Capabilities` section, decomposed
into four work-root sub-domains:

| Sub-domain | Owns |
|---|---|
| `offline-contract` | The tool's machine-readable interface committed to the repo without a live process — OpenAPI/JSON-schema/proto, CLI `--help`, generated reference docs. |
| `agent-onboarding` | The tool's self-teaching surface — `<cli> llm` topics, README quickstart/recipes — anything an agent reads before it writes its first command. |
| `interactive-tooling` | Live-surface ergonomics — `<cli> connect`, REPLs, watch/tail modes — anything that assumes a reachable deployed or remote instance. |
| `integration-contract` | Client-visible behavioral contracts that span calls — retry/error-code semantics, pagination, idempotency, versioning — the promises a caller's code, not just its docs, depends on. |

`interactive-tooling` applies only where the tool has a deployed or remote
surface to connect to (a k8s-native service, or a CLI with a `connect` verb
per the convention above); a pure-local CLI with no remote surface **MAY**
omit that sub-domain, but **MUST** say so explicitly in its work-root table
(e.g. `n/a — no remote surface`) rather than leaving the row out silently.

Each sub-domain is an ordinary work-root row (ID, gap/claim refs, status,
`sub-domain: <name>` in its Gate/Evidence column) — it does not require its
own H3 heading; one `### Developer & Agent Experience` capability with all
four sub-domains in its Work Root table satisfies this convention.

Gate expectations:

- `offline-contract` **MUST NOT** lag the live surface it describes: a
  committed schema/spec artifact that can drift from generated output
  **MUST** carry a byte-diff test between the committed file and live
  generation (normalized only for trailing-newline noise), not a
  human-review process.
- `agent-onboarding` and `integration-contract` text **MUST** be
  test-asserted, not merely present — a unit or CLI test that asserts on the
  exact advertised commands, codes, and thresholds, so a later edit that
  silently invalidates the prose fails CI instead of drifting.
- `interactive-tooling`, where present, follows the verification norms of
  its underlying convention (`connect`'s port-forward lifecycle, etc.)
  rather than inventing a new one.

`apps/lumen` is the reference instantiation: `### Developer & Agent
Experience` (`developer-agent-experience`, `AgentFirst`) with
`offline-contract` (`clients/openapi.json` byte-diff-tested against
`lumen spec --format openapi`), `agent-onboarding` (`lumen llm` topics
test-asserted by `tests/spec_cli.rs`), `interactive-tooling` (`lumen
connect`, `lumen query`), and `integration-contract` (the routed
multi-shard retry-code contract, test-asserted). A project that declares
the `agent_facing` trait derives the `agent-task-navigation` baseline; the
trait does not imply Kubernetes, HA, backup, or performance traits.

## Meta-doc content contract

> Every doc fact carries exactly one of: a generator (a projection with a
> named source), a validator (a contract or scanner that fails when the fact
> goes stale), or an explicit policy-only marker. A fact restated only in
> prose — none of the three — rots the first time reality moves, because
> nothing catches the drift.

Meta docs split by *layer*, not by topic. A fact belongs to exactly one layer.
There used to be a machine-readable source for the matrix below — `meta_docs.rs`
drove placement and section validation, and `aw meta init` / `sync` / `check`
were the sole writer and checker. Both are deleted. The matrix is hand-maintained
now, and there is no single checker for every row. Project product-document rows
are the exception: `scripts/meta/readme_contract.py` validates a README, and
`scripts/meta/project_docs_contract.py` validates an adopted README, STATUS,
and ROADMAP set plus its adopted protocol, generated-client, indexing,
querying, GKE, client-integration, and linked migration guides.
Other rows still depend on their named reader or a human review.

| Layer | Doc | Fact owner | Required headings | Inherits |
|---|---|---|---|---|
| repo | `/AGENTS.md` | what a Codex reviewer may and may not do to this checkout | none | none — deliberately not a projection of anything |
| repo | `/CLAUDE.md` | the work-item lifecycle, per-phase write roots, and the rules that refuse against them | none | none — the only file loaded at launch; there is no separate rules layer |
| repo | `/README.md` | repository identity, inventory, install, and discovery entrypoints | `## Contributing` | none |
| repo | `/CONTRIBUTING.md` | repo-wide authoring contracts, CLI conventions, and META-doc taxonomy | `## Meta-doc content contract` | none |
| project | `<project>/README.md` | project identity, primary user workflow, functional surface, product promises, capability sources, and the gate that verifies each | `## Brief`<br>`## Primary workflow`<br>`## Contract discovery`<br>`## Capabilities`<br>`## Supporting documents` | repo README + CONTRIBUTING |
| adopted project set | `<project>/STATUS.md` | current source support, material limits, and the executable gate for each supported surface | `## Scope`<br>`## State definitions`<br>`## Support matrix`<br>`## Evidence policy` | project README |
| adopted project set | `<project>/ROADMAP.md` | future outcomes, completion boundaries, and explicit non-goals | `## Purpose`<br>`## Near-term outcomes`<br>`## Later outcomes`<br>`## Non-goals` | project README + STATUS |
| optional interface guide | `<project>/docs/protocol.md` | map protocol facts to their canonical source and discovery surface without copying the full wire contract | `## Purpose`<br>`## Contract map`<br>`## Use the protocol`<br>`## Current boundaries`<br>`## Supporting documents` | project README + STATUS + ROADMAP |
| optional generated-client guide | `<project>/clients/README.md` | committed and generated artifact model, generation workflow, language differences, connection inputs, and current client limits | `## Contract`<br>`## Generate`<br>`## Language matrix`<br>`## Connect`<br>`## Current boundaries`<br>`## Verification`<br>`## Supporting documents` | project README + STATUS + ROADMAP |
| optional indexing guide | `<project>/docs/indexing.md` | source-data ownership, schema, write, durability, rebuild, and activation contracts, with current and target behavior kept separate | `## Purpose`<br>`## Contract map`<br>`## Data ownership`<br>`## Schema contract`<br>`## Write contract`<br>`## Durability`<br>`## Rebuild and activation`<br>`## Current boundaries`<br>`## Supporting documents` | project README + STATUS + ROADMAP |
| optional querying guide | `<project>/docs/querying.md` | selection, scoring, result controls, facets, limits, failures, and compatibility without copying the live wire schema | `## Purpose`<br>`## Data ownership`<br>`## Contract map`<br>`## Search model`<br>`## Result controls`<br>`## Facets and metrics`<br>`## Limits and failures`<br>`## Compatibility and migration`<br>`## Current boundaries`<br>`## Supporting documents` | project README + STATUS + ROADMAP |
| optional GKE guide | `<project>/docs/gke.md` | current and target GKE support tiers, runtime topology, Kubernetes-native boundaries, placement, storage, disruption, identity, networking, and acceptance | `## Purpose`<br>`## Contract map`<br>`## Support tiers`<br>`## Runtime size and topology`<br>`## Kubernetes-native contract`<br>`## GKE Standard Regional profile`<br>`## Storage, placement, and disruption`<br>`## Identity and networking`<br>`## Verification`<br>`## Current boundaries`<br>`## Supporting documents` | project README + STATUS + ROADMAP |
| optional client-integration guide | `<project>/docs/client-integration.md` | generated-client, workload-template, source integration, and caller responsibility boundaries | `## Purpose`<br>`## Contract map`<br>`## Responsibility boundary`<br>`## Connection profiles`<br>`## Generated client behavior`<br>`## Kubernetes workload template`<br>`## Source integration`<br>`## Failure handling`<br>`## Verification`<br>`## Current boundaries`<br>`## Supporting documents` | project README + STATUS + ROADMAP + generated-client guide when adopted |
| optional migration guide | `<project>/docs/migration-*.md` linked by the README | one versioned compatibility window, caller actions, activation rules, offline tools, and verification | `## Purpose`<br>`## Compatibility window`<br>`## Schema migration`<br>`## Request migration`<br>`## Response migration`<br>`## Managed activation`<br>`## Migration tools`<br>`## Verification`<br>`## Supporting documents` | project README + STATUS + ROADMAP + indexing/querying guides when adopted |
| project | `<project>/CONTRIBUTING.md` | project-local authoring, verification, migration, and contribution rules | `## Brief`<br>`## Authoritative Inputs`<br>`## Local Workflow`<br>`## Verification` | repo CONTRIBUTING |

The two root rows changed shape when the CLI went away, and the change is not
cosmetic. `AGENTS.md` and `CLAUDE.md` were one document in two runtime flavors,
kept in sync by a mirror test. They are now two documents with one reader each:
`codex exec` loads `AGENTS.md` from its workdir automatically and Claude Code
loads none of it, so editing `AGENTS.md` changes what the reviewer is told and
nothing else. Neither file carries the `##` headings its old row demanded,
because the producer that required them is gone.

The project row lost a sibling for the same reason. `<project>/CAPABILITIES.md`
was its own META-doc until 2026-08-17, when all 62 of them were deleted and the
five that carried content merged into their project's `README.md`. The doc type
failed on its own evidence: 57 of the 62 were the identical empty template with
a zero-row Capability Index, the two largest (`apps/relay`, `apps/mamba`)
had bypassed the template entirely, and `libs/service-http/CAPABILITIES.md`
stated in its own prose that the real contract lived in the README. Nothing
mechanical had ever read one — a repo-wide grep of `plugins/`, `.claude/`, and
`scripts/` found two hits, both example strings inside help text. Product
promises are now a `## Capabilities` section of the project README, which is
where a reader looks for them.

The `<!-- aw:meta:* -->` marker pairs still wrapping the first ~20 lines of most
project `README.md` and `CONTRIBUTING.md` files are from the same producer and
are equally inert — 66 of them, across 65 files, down from 126 before the
`CAPABILITIES.md` deletion. They mark where a splice used to land; nothing
splices there now, and the content between them is ordinary hand-edited prose.

The repo/global layer applies across the repository and teaches an agent how
to operate in the checkout. `AGENTS.md` and `CLAUDE.md` exist only at this
layer. `LICENSE` remains the legal text and the only root uppercase meta file
without a `.md` extension.

The project/app layer owns one deliverable's product contract and scoped local
conventions. `apps/<name>` is for app-facing binaries/services; legacy and
library-like deliverables may still live under `projects/<name>`. Scoped
convention docs remain allowed only next to the tree they govern; generated
evidence/docs require an explicit producer, validator, or policy-only marker.

Project-layer docs are written for agents first: they should answer "what is
this project promising?", "where is the source of truth?", "what am I allowed
to edit?", and "how do I prove the change?" without making the agent read a
full design book.

`apps/<name>/README.md`, `libs/<name>/README.md`, and a surviving
`projects/<name>/README.md` are the product contract as well as the front door.
They lead with the task a user came to perform. Architecture and ownership do
not come before the product purpose, the primary workflow, or the main public
operations.

The fixed shape is:

- `# <project>`
- `## Brief` — what this project does, what the caller supplies, what the
  project returns, and the important product boundary.
- `## Primary workflow` — the ordered path through the main user task, with the
  smallest useful examples.
- one or more project-specific functional `##` sections — for example, an
  index service describes how to build an index and which query methods it
  supports; a library describes its public API and composition points.
- `## Contract discovery` — the maintained commands, generated contracts, or
  runtime endpoints that own details too large for the front door.
- `## Capabilities` — the flat product-capability index and its source-backed
  verification details.
- `## Supporting documents` — links to getting-started, architecture,
  deployment, operations, runbooks, and the project CONTRIBUTING file when
  those documents exist.

Every capability is an equal product promise. Do not classify capability rows
as Core/Non-Core, primary/secondary, or ready/not-ready. Work-item state and
self-graded status fields belong outside the durable product contract.

`## Capabilities` starts with one `### Capability index` table using these
columns: `Capability | ID | User promise | Sources`. Each index row maps to one
flat `### <Capability>` section in the same order. The detail carries:

- `ID` — a stable lower-kebab-case identifier;
- `Promise` — observable behavior stated from the user's point of view;
- `Sources` — one bullet per direct source, including that source's material
  contribution;
- `Gate` — one or more verbatim consumer verification commands.

Sources use three forms. `apps/<name>` supplies app-specific behavior and
composition. `libs/<name>` supplies a reusable mechanism.
`external:<name>` supplies an outside runtime, authority, or provisioned
contract. A capability can name several sources, and a source can support
several capabilities. The source list is not an ownership class.

The deterministic validator is
`scripts/meta/readme_contract.py`. The `project-readme-check` skill that added
a clean-context reader after it was deleted on 2026-09-02; the script is now
the whole check, and a semantic read is a review step a human asks for.
The script checks format, links, source paths, and resolvable gate names. It
does not execute the gates or decide whether their behavior proves the promise.

A project can adopt `STATUS.md` and `ROADMAP.md` together. Until it does, its
README remains valid under the README-only contract. After adoption,
`## Supporting documents` must link both companion files and all three files
form one product-document set.

`STATUS.md` reports current source support. Its fixed shape is:

- `# <project> status`
- `## Scope` — what source or release the statements describe, and what they
  do not prove.
- `## State definitions` — one table defining `Supported`, `Limited`, and
  `Not supported`.
- `## Support matrix` — one flat table with `Surface | ID | State | Supported
  scope | Limits | Evidence`.
- `## Evidence policy` — where execution records live and when a row changes.

Every support ID is stable lower-kebab-case. `Supported` and `Limited` rows
name a resolvable executable gate. A `Limited` row also links its boundary to a
ROADMAP outcome or non-goal. A `Not supported` row links its evidence cell to
one of those same destinations and never claims a current gate. This document
names required gates. It does not claim they ran in the current session.

`ROADMAP.md` reports future outcomes. Its fixed shape is:

- `# <project> roadmap`
- `## Purpose` — the document boundary and the tracker relationship.
- `## Near-term outcomes`
- `## Later outcomes`
- `## Non-goals`

Each outcome is one flat H3 section. Its fields are `ID`, `Outcome`,
`Boundary`, `Completion evidence`, and `Tracking`. Each non-goal has `ID` and
`Reason`. The ID must match the H3 Markdown anchor so STATUS links stay stable.
Use `Tracking: Not assigned.` until a real tracker link exists. A release
Milestone owns the epic grouping, development order, and version identity.
Atomic issues own work state and assignees. The tracker owns schedules and
delivery history. Do not copy those fields or completion percentages into
ROADMAP.

A release Milestone title uses the SemVer core form
`<project>@<major>.<minor>.<patch>`. Each field is a non-negative integer
without leading zeroes. The normal next release increments minor and resets
patch to zero. A major bump, patch bump, or exact version is a human override.
If the project has no prior release Milestone, the human chooses its initial
version. Calculate the normal next version from all open and closed release
Milestones. Do not derive it from a package manifest or Git tag. This planning
policy does not replace project-local build or release version rules.

A project can also adopt conventional supporting guides. They are optional.
When a conventional protocol, client, indexing, querying, GKE, or
client-integration file exists, the project README must link it from
`## Supporting documents`. A versioned `docs/migration-*.md` guide is adopted
only when the README links it. The full-set checker includes the exact bytes of
every adopted guide.

`docs/protocol.md` is a protocol index. Its `## Contract map` begins with
`Fact | Canonical source | Discovery`. It points each fact class at the
machine-readable or runtime-generated source that owns it. It must not become a
second hand-maintained copy of a route inventory, schema, query grammar, or
retry table.

`clients/README.md` is the project generated-client guide. Its
`## Language matrix` begins with
`Language | Generated form | Transport | Auth input | Current limits`. It says
which artifacts are committed, generated, or published. It also states the
real toolchain, connection, authentication, and language-specific limits.

`docs/indexing.md` owns durable indexing semantics. Its `## Contract map`
begins with `Fact | Canonical source | Discovery`. It distinguishes the live
schema, write, durability, and rebuild contract from future outcomes.

`docs/querying.md` owns query semantics that are too detailed for the README.
Its `## Contract map` uses the same columns. It separates current requests from
future scoring, result, facet, metric, limit, and failure behavior.

`docs/gke.md` owns one project's GKE support tiers and its boundary between a
portable Kubernetes contract and a provider-specific installation profile. It
keeps current acceptance evidence separate from a future production profile.

`docs/client-integration.md` owns the split between service behavior, generated
client behavior, Kubernetes workload changes, and caller-owned source work. It
uses the same `Fact | Canonical source | Discovery` contract map and keeps a
planned template or client behavior out of the current support claim.

A linked `docs/migration-*.md` is a versioned caller guide. Its compatibility
table begins with `Surface | 0.4.x | 0.5.0 | Required action`. It must state
both sides of each change, the caller action, Managed activation, available
migration tools, and verification. Historical migration notes that are not
linked from the README remain outside the adopted product-document set.

The full-set validator is `scripts/meta/project_docs_contract.py`. It applies
the README validator, checks both companion formats, resolves gates and
anchors, validates adopted supporting guides, and rejects deterministic
cross-document conflicts. The clean-context read that used to follow it (the
deleted `project-readme-check` skill) is now a review step a human asks for:
give only the exact adopted files to one context-free reader when meaning or
contradictions — what a script cannot decide — need checking.

`apps/<name>/CONTRIBUTING.md` / `projects/<name>/CONTRIBUTING.md` is the local
operating guide. It does not restate root authoring rules and does not carry
the product promises that belong in the README. It explains how an agent safely
changes this deliverable. The required shape is:

- `# <Project> Contributing`
- `## Brief` — one to three sentences, copied into the project README's
  `## Contributing` section.
- `## Authoritative Inputs` — ordered source-of-truth list for product,
  behavior, generated code, external contracts, and local conventions.
- `## Local Workflow` — the commands, generators, lifecycle routes, and edit
  boundaries an agent should use for this project.
- `## Verification` — exact local checks, test gates, service dependencies,
  skip rules, and evidence commands needed before claiming completion.

Optional sections are allowed when they save an agent from guessing:
`## Architecture Boundaries`, `## Data and Services`, `## Migration and
Compatibility`, `## Release`, and `## Meta Docs`. Keep them scoped to this
project. If a rule applies to every project, move it back to this root
CONTRIBUTING instead.

`CLAUDE.md` and `AGENTS.md` are **not** project-layer docs. Project-specific
agent behavior must be expressed through the project
README/CONTRIBUTING contract, scoped convention docs,
skills/templates, or command output. Template sources may contain a
`CLAUDE.md` filename when they are producer inputs, but they are not live
project-layer meta docs.

Three rules govern every cell above:

- **Single-level rule** — content lives at exactly one level; every other
  level links to it, never restates it.
- **Fact-ownership rule** — a fact with no generator, no validator, and no
  policy-only marker is a defect, not a stylistic choice; add whichever of
  the three fits before landing the fact.
- **Level heuristic** — "how do all projects do X" belongs in CONTRIBUTING;
  "what does this project promise" belongs in its own README; "how does an
  agent operate here" belongs in CLAUDE/AGENTS; "what exists" belongs in the
  repo-root README.

What still enforces any of this, and what does not:

- **Enforced.** `.claude/aw/verification/check_plugin.py` asserts the lifecycle
  skills on disk are exactly the registered set. The repository-neutral
  project-document scripts enforce the README-only and adopted document-set
  rows above. No one script enforces every row in this chapter.
- **Policy, unenforced.** The root carries only the doc files this contract
  names; a stray root doc is a defect, not a new home.
- **Policy, unenforced.** `apps/**/{CLAUDE,AGENTS}.md` and
  `projects/**/{CLAUDE,AGENTS}.md` are forbidden except for declared
  template-source files. Measured on 2026-08-17: zero violations, by hand.
- **Retracted.** CLAUDE and AGENTS were held to one document in two runtime
  flavors by a mirror test. They now diverge on purpose — see the two root rows
  above — and the test that forbade it is deleted along with everything else in
  this list: the root-doc producer/check tests, the capability-contract
  consistency check, and the scanners that swept agent-facing docs for retired
  command literals. That last one is why a doc explaining that a command was
  retired can now carry the command's literal text with nothing objecting; the
  distinction between "documents the removal" and "still tells an agent to run
  it" is a reader's job again.

Payload corollary: when a lifecycle section has a schema, its agent payload is
data and the prose around it is a rendering of that data; prose-only payloads
remain Markdown. There is no CLI doing the rendering any more, so today this
means "write the schema-shaped part as data and do not let a hand-written
summary drift from it", not "run a projector".

## Project build and release contract

Every project `build.sh` must use the same two-mode contract. The per-project
`<project>-build-{debug,release}` skill wrappers are deleted (2026-09-02), and
`lumen-build-release` folded into `build-release` (2026-09-04):
`build-release <app>` is the candidate-first release chain for lumen, tape,
sift, keep, relay, and defer, and its first step is the release mode below. A
plain compile is `cargo build -p <crate>` directly; `build-debug` is a local
kind cluster run for keep/defer/relay/loom and not this contract. The
versioned debug/release modes below run the project's own
`apps/<name>/build.sh` from the repository root.

### Debug builds

Debug builds are local install checkpoints:

1. If the tree is dirty, commit first with a debug checkpoint commit.
2. Read the project's configured base version and check whether
   `<project>@<version>` already exists locally or on `origin`.
3. If the tag exists, advance to the next version using the repository base-64
   carry rule: patch increments first; when patch would exceed 63, reset patch
   to 0 and increment minor; when minor would exceed 63, reset minor to 0 and
   increment major.
4. Build with the Cargo debug profile and a SemVer build-metadata suffix:
   `<base-version>+<git-sha>`.
5. Install the debug binary locally, then restore manifest and lockfile edits so
   the debug-only version suffix is not left in the worktree.

### Release builds

Release builds are not complete until the GitHub Release is visible, and the
tag is never the first artifact. The required chain is `build-release
<project>`, whose steps are:

1. **release-prep**: run the project `build.sh release` from the repository
   root. A legacy script checks local and remote tag collisions for
   `<project>@<version>`, advances the version with the same base-64 carry rule
   when needed, runs the Cargo release build, installs the local release binary,
   commits version files as `release(<project>): <project>@X.Y.Z`, and prints
   `RELEASE_TAG=<project>@X.Y.Z`. A lumen-shaped script (lumen, tape, and every
   app listed in `scripts/release/apps.sh`) reads the version from
   `Cargo.toml`, syncs the Kubernetes image pins, and builds without bumping or
   committing; the version bump is an ordinary commit. Neither may create or
   push the tag.
2. **land**: run `git-land` as-is so the release commit lands on `main`. Do not
   tag before this step; rebases and squash merges can orphan a pre-land tag.
3. **candidate**: `scripts/release/candidate.sh <project> <version> <sha>`
   dispatches `<project>-release-candidate.yml` from the landed `main` sha,
   watches it, and downloads the immutable candidate bundle (archives, digest
   pinned image, signature, provenance, SBOM attestations, final manifest).
4. **verify**: the project's `verify-release-candidate.sh --mode full` re-proves
   the bundle independently of the workflow's own verdict.
5. **GKE gate**: the candidate image `ghcr.io/chrischeng-c4/<project>@<digest>`
   is deployed and verified on GKE by the project's harness, and the evidence
   is bound into `<project>-gke-receipt.json` plus its `.sha256` sidecar.
6. **promote**: `scripts/release/promote.sh` creates the one annotated
   `<project>@X.Y.Z` tag at the candidate's exact commit, dispatches
   `<project>-release.yml` at that tag with the receipt, and watches it; the
   promotion retags the same digest as semver and `latest` and publishes the
   GitHub Release from the candidate bytes, never rebuilding.
7. **public verification**: `verify-release-artifacts.sh --mode public` must
   pass before the release is reported complete or the tracker is closed.

`scripts/project-build-monitor-release.sh` remains for projects still on a
tag-triggered `<project>-release.yml`; it is not part of the chain above.

The release identity is always:

```
<project>@X.Y.Z        # e.g. lumen@0.4.4, vat@0.3.62
```

Projects should release **independently**. A releasing project should prefer its
own `version` in its project `Cargo.toml`, so bumping one project does not bump
the others. A few crates still inherit `[workspace.package].version`; until they
are migrated, their `build.sh` must explicitly name the version source and every
manifest it edits. Do not silently bump the workspace version for an unrelated
project release.
