---
name: build-release
description: "Publish one verified release of tape, sift, keep, relay, or defer from an immutable, digest-pinned candidate that passed GKE acceptance: candidate workflow → independent verifier → GKE receipt → one annotated <app>@<version> tag at the landed sha → no-rebuild promotion → public verifier. loom gets the GKE acceptance leg only; anything else is refused. Use when the user asks to release, ship, promote, or run release acceptance for an app."
user-invocable: true
---

# build-release

A release is not a compile flag and not a tag. It is one immutable candidate
built from a landed `main` sha, proven twice (the workflow's own gates, then
the controller's independent verifier), deployed digest-pinned to GKE, and
only then named by one annotated `<app>@<version>` tag whose promotion
republishes the same bytes. The mechanics live in `scripts/release/*.sh` and
the per-app workflows; your job is to route the app, run each step in order,
stop at the first refusal, and relay what was measured. The local counterpart
for keep, defer, relay, and loom is `build-debug`.

Never use the retired tag-first workflow. Do not create a tag to start a
build. Do not create a tag from a dirty worktree. Do not rebuild, re-sign, or
re-attest after candidate acceptance.

## Routes

| App | Route |
|---|---|
| `tape`, `sift`, `keep`, `relay`, `defer` | the ten steps under "Required order"; `scripts/release/apps.sh` is the per-app table every driver reads |
| `lumen` | released from https://github.com/faberline/lumen by that repository's own `build-release`; say so and stop |
| `loom` | `scripts/build/release.sh loom` — GKE acceptance only (see "loom"); no candidate, no tag, no GitHub Release |
| anything else | `scripts/release/apps.sh` refuses with `refused: unknown release app: <app> (known: tape sift keep relay defer)` and `scripts/build/release.sh` with `refused: release route not wired for <app>`; relay the line and stop — never fall back to `cargo build --release`, and never wire a route yourself |

Per-app facts, read from `scripts/release/apps.sh` rather than from memory:

| App | Root | Verifiers | GKE gate (step 5) | Receipt maker (step 6) |
|---|---|---|---|---|
| tape | `apps/tape` | `apps/tape/scripts/verify-release-{candidate,artifacts}.sh` | `acceptance/gcp/scripts/run.sh` with `ACCEPTANCE_APPS=tape` and `TAPE_IMAGE=<candidate root digest reference>` | `apps/tape/scripts/make-gke-release-receipt.py` from the run's `run.json`, `images.json`, `acceptance.json`, `cleanup.json` |
| sift | `projects/sift` | `scripts/release/verify-release-{candidate,artifacts}.sh --app sift` | `acceptance/gcp/scripts/run.sh` with `ACCEPTANCE_APPS="lumen sift"`, `LUMEN_IMAGE=<current lumen release digest reference>`, `SIFT_IMAGE=<candidate root digest reference>` | `scripts/release/make-gke-release-receipt.py --app sift --backend gcp` |
| keep, relay, defer | `apps/<app>` | `scripts/release/verify-release-{candidate,artifacts}.sh --app <app>` | `scripts/build/release.sh <app> --image ghcr.io/chrischeng-c4/<app>@sha256:<root digest>`, with `main` checked out at the candidate's exact commit (the receipt binds the run's head sha to the candidate) | `scripts/release/make-gke-release-receipt.py --app <app> --backend gke-acceptance` from the `gh run view --json …` record its `--help` names and the downloaded evidence bundle |

## Required order

Run every step from the repository root on `main`. Each driver prints the
command that follows it and exits `0` on success, `1` on a red run or a
mismatch, `2` on refusal; a `1` or `2` ends the attempt — report the printed
reason and stop. The watches outlast the foreground limit: run steps 3, 5,
and 7 with `run_in_background` and poll their output.

1. Run release preparation: `apps/<app>/build.sh release`
   (`projects/sift/build.sh release` for sift). It reads the version from
   the app's `Cargo.toml`, syncs the Kubernetes image pins, and builds
   locally; it neither bumps the version nor commits. Record
   `<app>@<version>`. A version bump is an ordinary commit made before this
   step.
2. Run `git-land`. Record the landed `main` sha and the merged pull request.
3. Dispatch `<app>-release-candidate` from `main` at that exact sha:
   `scripts/release/candidate.sh <app> <version> <sha>` (20–60 minutes). It
   watches the run and downloads the run-scoped bundle
   `<app>-release-candidate-<run>-<attempt>`. Wait for the final candidate manifest.
   It must bind every candidate job conclusion (the job set
   `scripts/release/apps.sh` lists for the app), the dispatched sha, the
   version, the run id and attempt, every archive, and the digest-pinned
   image `ghcr.io/chrischeng-c4/<app>@sha256:<root digest>`; the driver
   exits `1` when it does not.
4. Independently run the candidate verifier in full mode. Stop on any mismatch.
   The verifier is the app's `verify-release-candidate.sh --mode full`
   against the downloaded bundle (per-app table above). It re-proves the
   manifest and its sidecar, every archive, the image, its signature,
   provenance, and SBOM attestations, and the workflow's job set. Its exit is
   the acceptance; the workflow's own green is not.
5. Run the app's GKE gate against the candidate image (per-app table
   above). It is paid, and the human triggers it. Deploy only
   `ghcr.io/chrischeng-c4/<app>@sha256:<root digest>` from the manifest,
   never a rebuilt or retagged image. Keep the evidence the gate leaves
   behind: `run.json`, `images.json`, `acceptance.json`, and
   `cleanup.json` for tape and sift; the `gke-acceptance` run record and
   evidence bundle for keep, relay, and defer.
6. Bind the evidence into the receipt `scripts/release/apps.sh` names for
   the app (`<app>-gke-receipt.json`) with the app's receipt maker, then
   write the `.sha256` sidecar as `<sha256>  <receipt name>` next to it. The
   receipt binds the candidate's final manifest bytes, commit, run, attempt,
   and image digests to one passed run. It never carries a kubeconfig,
   token, or cluster credential; the makers refuse to read one.
7. The controller creates one annotated `<app>@<version>` tag at the exact
   landed sha through
   `scripts/release/promote.sh <app> <version> <run-id> <attempt> <receipt>`
   (5–20 minutes). The script refuses a dirty tree, a branch other than
   `main`, a candidate that is not a successful `<app>-release-candidate`
   run of a commit on `origin/main`, a receipt or sidecar that does not bind
   that candidate, and a missing ruleset. It must not force, move, or delete
   a tag. Verify the active tag ruleset first: target `tag`; include only
   `refs/tags/<app>@*`; rules exactly `update` and `deletion`; no bypass
   actor; no `creation` rule. A missing ruleset is a release blocker, not
   something to create in the middle of a release.
8. Dispatch `<app>-release` at that exact tag: the same `promote.sh` run
   continues into the dispatch with `version`, `candidate_run_id`,
   `candidate_run_attempt`, and the receipt and sidecar bytes with their
   SHA-256s, then watches it. The promotion
   re-proves the tag, ruleset, candidate run, receipt, signature,
   provenance, and SBOM attestations before any public write; it retags the
   same digest as semver and `latest` and publishes the GitHub Release from
   the candidate bytes. When the tag already exists on `origin` at the
   candidate commit and the Release does not, `promote.sh` re-dispatches the
   same identity; when the Release exists it refuses and points at step 9.
9. Run the public verifier. It is the app's
   `verify-release-artifacts.sh --mode public` (`promote.sh` prints the
   invocation). It must prove the annotated tag, the public release assets
   and their hashes, the host binary version from a private `HOME`, the
   semver image root, a safe `latest`, the root signature, provenance, the
   child manifests, and the SPDX asset-attestation pairs.
10. Only after public verification succeeds, update and close the tracker.

## loom

`scripts/build/release.sh loom` dispatches
`.github/workflows/gke-acceptance.yml` at the pushed HEAD, watches it,
downloads the evidence bundle, and reads back the park step. Run it with
`run_in_background` (15–40 minutes) and relay its five report lines verbatim
plus the exit code: `run:` URL, `conclusion:`, `acceptance job:`,
`park step:`, `evidence:` directory. Exit `0` means the run and the park step
both concluded `success`. Exit `1` means the run was red or the park step was
not `success`: say first that the node pool may still be running, pass on the
script's `park.sh` instruction, and point at `evidence:` for the failing step.
Exit `2` is a refusal — a dirty tree, an unpushed HEAD, a workflow file not
yet on the default branch, or a sha+app that already has a successful run;
that last lifts only with `--rerun`, and only when the human names what
changed. The same script with `--image` is step 5 for keep, relay, and
defer.

## Reruns

- A red candidate is fixed on `main` and re-run as a new candidate with a
  new run id. A candidate is never patched, re-signed, or re-attested.
- A red GKE gate is re-run against the same candidate image; the receipt
  names the run that passed.
- A red promotion re-runs through `promote.sh` with the same tag, candidate,
  and receipt. A tag is never moved to another commit; a different candidate
  needs a new version.
- `scripts/build/release.sh --rerun` needs the human to name what changed; a
  paid re-run "to see if it passes this time" is not evidence.

## Controller boundaries

- The controller (the main session) owns Git, tags, workflow dispatch, the
  tracker, kind and GKE runs, and publication. Workers and subagents never
  receive credentials, never run `gh`, `kubectl`, or `gcloud` for a release,
  and never write a receipt.
- Never treat `cargo build -p <crate> --release` as a release; it proves
  nothing about deploy, verify, or publication.
- Never dispatch from a dirty or unpushed tree, and never work around a
  driver's refusal by calling `gh workflow run` or the `gh api` behind it
  yourself.
- Never call `acceptance/gke-harness/scripts/*.sh`, `terraform`, `kubectl`,
  or `gcloud` by hand for the `gke-acceptance` leg; the workflow owns
  deploy, verify, and park. The `acceptance/gcp` gates are controller
  scripts, invoked exactly as their usage lines say.
- Never edit source, manifests, lockfiles, workflows, or verifiers to make a
  step pass; the failure is the finding.
- Never put a kubeconfig, token, or cluster credential into a receipt, a
  report, or a tracker comment.
