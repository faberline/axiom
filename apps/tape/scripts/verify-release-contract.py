#!/usr/bin/env python3
"""Static and fixture oracle for Tape's build-once release contract."""

from __future__ import annotations

import argparse
import copy
import hashlib
import io
import json
import re
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[3]
CANDIDATE_PATH = ROOT / ".github/workflows/tape-release-candidate.yml"
PROMOTION_PATH = ROOT / ".github/workflows/tape-release.yml"
GKE_MAKER = ROOT / "apps/tape/scripts/make-gke-release-receipt.py"
GKE_HARNESS = ROOT / "acceptance/gcp/scripts/verify-tape.sh"
CANDIDATE_VERIFIER = ROOT / "apps/tape/scripts/verify-release-candidate.sh"
ARTIFACT_VERIFIER = ROOT / "apps/tape/scripts/verify-release-artifacts.sh"
KIND_SCRIPT = ROOT / "apps/tape/scripts/kind-e2e.sh"
DOCKERFILE = ROOT / "apps/tape/Dockerfile.release"
CARGO_TOML = ROOT / "apps/tape/Cargo.toml"
STATEFULSET = ROOT / "apps/tape/k8s/base/statefulset.yaml"
OPERATOR_DEPLOYMENT = ROOT / "apps/tape/k8s/operator/deployment.yaml"
PROMOTION_WORKFLOW_SHA256 = "3042fba754460473df9ae899173243d3d504543ec0524cd3e91e01acf986ad9e"
KIND_SERVER_STATEFULSET_SELECTOR = (
    "  stateful_image=\"$(kubectl -n \"$NAMESPACE\" get statefulset \"$TAPE_NAME\" "
    "-o jsonpath='{.spec.template.spec.containers[?(@.name==\"server\")].image}')\""
)
KIND_SERVER_POD_INVENTORY = (
    '  assert_named_pods_use_candidate "$NAMESPACE" "$SERVER_LABEL" server 1'
)

TAPE_GATES = (
    "python3 apps/tape/scripts/verify-release-contract.py --self-test",
    "cargo test --locked -p tape",
    "cargo test --locked -p tape --features operator,backup",
    "cargo test --release --locked -p tape --test tape_perf_gate",
    "uv run --python 3.13 --no-project scripts/meta/project_docs_contract.py check apps/tape --format json",
    "bash scripts/raft-implementor-build.sh",
    "bash apps/tape/e2e/raft_soak.sh",
)
LIBRARY_GATES = (
    "bash scripts/faberline-core-test.sh service-k8s storage-durable service-backup raft-core raft-runtime",
    "cargo test --locked -p relay --test raft_cluster",
    "bash scripts/raft-implementor-build.sh",
)
CANDIDATE_JOBS = {
    "identity",
    "build",
    "tape-release-gates",
    "ghcr-image-and-attest",
    "manifest",
    "verify-candidate",
    "verify-libraries",
    "kind-amd64",
    "kind-arm64",
    "result",
}
RESULT_NEEDS = (
    "identity",
    "build",
    "tape-release-gates",
    "manifest",
    "ghcr-image-and-attest",
    "verify-candidate",
    "verify-libraries",
    "kind-amd64",
    "kind-arm64",
)
FINAL_JOBS = {
    "identity": "success",
    "build": "success",
    "tape-release-gates": "success",
    "manifest": "success",
    "ghcr-image-and-attest": "success",
    "verify-candidate": "success",
    "verify-libraries": "success",
    "kind-amd64": "success",
    "kind-arm64": "success",
    "result": "success",
}
TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
)


class ContractError(RuntimeError):
    pass


def fail(message: str) -> None:
    raise ContractError(message)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def top_block(text: str, key: str) -> str:
    lines = text.splitlines()
    marker = f"{key}:"
    try:
        start = lines.index(marker)
    except ValueError:
        fail(f"top-level {key} block is absent")
    end = len(lines)
    for index in range(start + 1, len(lines)):
        if lines[index] and not lines[index].startswith((" ", "\t")):
            end = index
            break
    return "\n".join(lines[start:end])


def workflow_inputs(text: str) -> set[str]:
    block = top_block(text, "on")
    if len(re.findall(r"^  workflow_dispatch:$", block, re.MULTILINE)) != 1:
        fail("workflow must have exactly one workflow_dispatch trigger")
    if re.search(r"^  (?:push|pull_request|schedule|workflow_call):", block, re.MULTILINE):
        fail("workflow has a non-manual trigger")
    return set(re.findall(r"^      ([a-z][a-z0-9_]*):$", block, re.MULTILINE))


def job_ids(text: str) -> set[str]:
    block = top_block(text, "jobs")
    return set(re.findall(r"^  ([a-z0-9][a-z0-9_-]*):$", block, re.MULTILINE))


def job_block(text: str, job: str) -> str:
    lines = text.splitlines()
    marker = f"  {job}:"
    try:
        start = lines.index(marker)
    except ValueError:
        fail(f"job {job} is absent")
    end = len(lines)
    for index in range(start + 1, len(lines)):
        if re.fullmatch(r"  [a-z0-9][a-z0-9_-]*:", lines[index]):
            end = index
            break
    return "\n".join(lines[start:end])


def step_block(job: str, name: str) -> str:
    lines = job.splitlines()
    marker = f"      - name: {name}"
    try:
        start = lines.index(marker)
    except ValueError:
        fail(f"step {name!r} is absent")
    end = len(lines)
    for index in range(start + 1, len(lines)):
        if lines[index].startswith("      - "):
            end = index
            break
    return "\n".join(lines[start:end])


def run_script(step: str) -> str:
    lines = step.splitlines()
    for index, line in enumerate(lines):
        match = re.fullmatch(r"(\s*)run:\s*(.*)", line)
        if not match:
            continue
        indent = len(match.group(1))
        value = match.group(2)
        if value not in {"|", "|-", ">", ">-"}:
            return value
        body: list[str] = []
        for next_line in lines[index + 1 :]:
            if next_line and len(next_line) - len(next_line.lstrip()) <= indent:
                break
            body.append(next_line[indent + 2 :] if next_line else "")
        return "\n".join(body)
    fail("named step has no run command")


def exact_commands(script: str) -> tuple[str, ...]:
    commands = []
    for raw in script.splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or line == "set -euo pipefail":
            continue
        commands.append(line)
    return tuple(commands)


def all_run_scripts(text: str) -> list[str]:
    lines = text.splitlines()
    scripts: list[str] = []
    index = 0
    while index < len(lines):
        match = re.fullmatch(r"(\s*)(?:-\s+)?run:\s*(.*)", lines[index])
        if not match:
            index += 1
            continue
        indent = len(match.group(1))
        value = match.group(2)
        if value not in {"|", "|-", ">", ">-"}:
            scripts.append(value)
            index += 1
            continue
        body: list[str] = []
        index += 1
        while index < len(lines):
            line = lines[index]
            if line and len(line) - len(line.lstrip()) <= indent:
                break
            body.append(line[indent + 2 :] if line else "")
            index += 1
        scripts.append("\n".join(body))
    return scripts


def action_values(text: str) -> list[str]:
    values = []
    for line in text.splitlines():
        match = re.match(r"^\s*(?:-\s+)?uses:\s*([^\s#]+)", line)
        if match:
            values.append(match.group(1))
    return values


def assert_actions_pinned(label: str, text: str) -> None:
    values = action_values(text)
    if not values:
        fail(f"{label} has no parsed action references")
    for value in values:
        if not re.fullmatch(r"[^@\s]+@[0-9a-f]{40}", value):
            fail(f"{label} has mutable action reference {value}")


def shell_tokens(line: str) -> list[str]:
    try:
        lexer = shlex.shlex(line, posix=True, punctuation_chars=";&|")
        lexer.whitespace_split = True
        lexer.commenters = "#"
        return list(lexer)
    except ValueError:
        return []


def shell_fragments(script: str) -> list[list[str]]:
    logical = re.sub(r"\\\n[ \t]*", " ", script)
    fragments: list[list[str]] = []
    for line in logical.splitlines():
        tokens = shell_tokens(line.strip())
        if not tokens:
            continue
        start = 0
        for index, token in enumerate(tokens):
            if token in {";", "&&", "||", "|", "&"}:
                if index > start:
                    fragments.append(tokens[start:index])
                start = index + 1
        if start < len(tokens):
            fragments.append(tokens[start:])
    return fragments


def assert_no_shell_wrapper(tokens: list[str]) -> None:
    for token in tokens:
        if PurePosixPath(token).name not in {"bash", "sh", "zsh", "dash", "ksh"}:
            continue
        fail("promotion invokes a shell wrapper, which can hide a rebuild command")


def assert_no_rebuild(text: str) -> None:
    if any(value.startswith("docker/build-push-action@") for value in action_values(text)):
        fail("promotion invokes docker/build-push-action")
    forbidden = {
        ("docker", "build"),
        ("docker", "buildx", "build"),
        ("podman", "build"),
        ("buildah", "bud"),
        ("cargo", "build"),
        ("nix", "build"),
    }
    for script in all_run_scripts(text):
        pending = shell_fragments(script)
        while pending:
            tokens = pending.pop()
            if "eval" in tokens:
                fail("promotion contains eval, which can hide a rebuild command")
            assert_no_shell_wrapper(tokens)
            for sequence in forbidden:
                for index in range(0, len(tokens) - len(sequence) + 1):
                    if tuple(tokens[index : index + len(sequence)]) == sequence:
                        fail(f"promotion contains rebuild command: {' '.join(sequence)}")


def assert_reviewed_promotion_bytes(text: str) -> None:
    actual = hashlib.sha256(text.encode("utf-8")).hexdigest()
    if actual != PROMOTION_WORKFLOW_SHA256:
        fail(
            "promotion workflow bytes are not in the reviewed SHA-256 allowlist: "
            f"expected {PROMOTION_WORKFLOW_SHA256}, got {actual}"
        )


def assert_release_versions(
    cargo_toml: str,
    dockerfile: str,
    statefulset: str,
    operator_deployment: str,
) -> str:
    perf_target = '''[[test]]
name = "tape_perf_gate"
path = "e2e/tape_perf_gate.rs"
test = false'''
    if cargo_toml.count('name = "tape_perf_gate"') != 1 or cargo_toml.count(perf_target) != 1:
        fail("Tape performance target must be opt-in with test = false")
    matches = re.findall(r'^version = "([0-9]+\.[0-9]+\.[0-9]+)"$', cargo_toml, re.MULTILINE)
    if len(matches) != 1:
        fail("Tape Cargo manifest must carry one exact package version")
    version = matches[0]
    if dockerfile.count(f"ARG TAPE_VERSION=tape@{version}") != 1:
        fail("release Dockerfile version does not match Tape Cargo version")
    expected_image = f"image: ghcr.io/chrischeng-c4/tape:{version}"
    if statefulset.count(expected_image) != 1:
        fail("Tape StatefulSet image version does not match Tape Cargo version")
    if operator_deployment.count(expected_image) != 1:
        fail("Tape operator image version does not match Tape Cargo version")
    return version


def assert_kind_candidate_identity(kind: str) -> None:
    for required in (
        'IMAGE_MODE="${TAPE_E2E_IMAGE_MODE:-local}"',
        "assert_named_pods_use_candidate",
        "TAPE_E2E_EXPECTED_RUNTIME_DIGEST",
        'tape --version',
    ):
        if required not in kind:
            fail(f"Kind prebuilt identity check lacks {required}")
    lines = kind.splitlines()
    if lines.count(KIND_SERVER_STATEFULSET_SELECTOR) != 1:
        fail("Kind prebuilt identity check lacks operator-rendered server StatefulSet selector")
    if lines.count(KIND_SERVER_POD_INVENTORY) != 1:
        fail("Kind prebuilt identity check lacks operator-rendered server pod inventory")


def parse_needs(job: str) -> tuple[str, ...]:
    match = re.search(r"^    needs: \[([^]]+)\]$", job, re.MULTILINE)
    if not match:
        fail("result job has no inline needs list")
    return tuple(item.strip() for item in match.group(1).split(","))


def check_contract(candidate: str, promotion: str) -> None:
    supporting = {
        "GKE receipt maker": GKE_MAKER.read_text(),
        "GKE harness": GKE_HARNESS.read_text(),
        "candidate verifier": CANDIDATE_VERIFIER.read_text(),
        "artifact verifier": ARTIFACT_VERIFIER.read_text(),
        "Kind script": KIND_SCRIPT.read_text(),
        "release Dockerfile": DOCKERFILE.read_text(),
        "Tape Cargo manifest": CARGO_TOML.read_text(),
        "Tape StatefulSet": STATEFULSET.read_text(),
        "Tape operator Deployment": OPERATOR_DEPLOYMENT.read_text(),
    }
    if workflow_inputs(candidate) != {"version", "commit"}:
        fail("candidate inputs must be exactly version and commit")
    if workflow_inputs(promotion) != {
        "version",
        "candidate_run_id",
        "candidate_run_attempt",
        "gke_receipt_b64",
        "gke_receipt_sha256",
        "gke_receipt_sidecar_b64",
        "gke_receipt_sidecar_sha256",
    }:
        fail("promotion inputs changed")
    if job_ids(candidate) != CANDIDATE_JOBS:
        fail("candidate job inventory changed")
    if job_ids(promotion) != {"verify-inputs", "publish-release"}:
        fail("promotion job inventory changed")
    assert_actions_pinned("candidate", candidate)
    assert_actions_pinned("promotion", promotion)

    identity = run_script(step_block(job_block(candidate, "identity"), "Prove exact landed main candidate identity"))
    for required in (
        '[[ "$GITHUB_REF" == "refs/heads/main" ]]',
        '[[ "$REQUESTED_COMMIT" == "$GITHUB_SHA" ]]',
        'git merge-base --is-ancestor "$GITHUB_SHA" origin/main',
        'refs/tags/tape@${cargo_version}',
    ):
        if required not in identity:
            fail(f"candidate identity proof lacks {required}")

    build = run_script(step_block(job_block(candidate, "build"), "Build exact release candidate binary"))
    if build != 'cargo build --release --locked -p tape --bin tape --features "operator backup self-update issue" --target ${{ matrix.target }}':
        fail("candidate binary does not carry the exact release feature set")

    tape_commands = exact_commands(
        run_script(step_block(job_block(candidate, "tape-release-gates"), "Run declared Tape gates"))
    )
    if tape_commands != TAPE_GATES:
        fail("candidate Tape gate execution set changed")
    library_commands = exact_commands(
        run_script(step_block(job_block(candidate, "verify-libraries"), "Run required service and Raft library gates without GKE"))
    )
    if library_commands != LIBRARY_GATES:
        fail("candidate shared-library gate execution set changed")

    result = job_block(candidate, "result")
    if parse_needs(result) != RESULT_NEEDS:
        fail("final candidate receipt does not depend on every required job")
    for job in FINAL_JOBS:
        if job == "result":
            if not re.search(r'(?:"result"|result):"success"', result):
                fail("final candidate receipt lacks the result binding")
            continue
        binding = re.compile(
            rf'(?:"{re.escape(job)}"|{re.escape(job)}):'
            rf'"\$\{{\{{ needs\.{re.escape(job)}\.result \}}\}}"'
        )
        if not binding.search(result):
            fail(f"final candidate receipt lacks the {job} result binding")
    if 'schema:"cclab.tape.candidate-manifest.v1"' not in candidate:
        fail("candidate manifest schema changed")
    if '--mode full' not in run_script(step_block(job_block(candidate, "verify-candidate"), "Verify full run-scoped candidate supply chain")):
        fail("candidate does not run full supply-chain verification")

    for job, digest in (("kind-amd64", "amd64_digest"), ("kind-arm64", "arm64_digest")):
        script = run_script(step_block(job_block(candidate, job), "Run prebuilt candidate kind e2e"))
        for required in (
            "TAPE_E2E_IMAGE_MODE=prebuilt",
            "TAPE_E2E_IMAGE=",
            "TAPE_E2E_EXPECTED_VERSION=",
            f"outputs.{digest}",
            "bash apps/tape/scripts/kind-e2e.sh",
        ):
            if required not in script:
                fail(f"{job} does not bind the prebuilt candidate: {required}")

    assert_no_rebuild(promotion)
    if promotion.count("--candidate-run-attempt") != 4:
        fail("promotion does not pass the exact run attempt to every verification")
    if '[[ "$GITHUB_REF" == "refs/tags/tape@${{ inputs.version }}" ]]' not in promotion:
        fail("promotion does not require the exact release tag")
    promote = run_script(step_block(job_block(promotion, "publish-release"), "Promote exact root digest to semver and safe latest"))
    if 'root_ref="${image_repo}@${root_digest}"' not in promote:
        fail("promotion source is not the immutable root digest")
    create_lines = [line.strip() for line in promote.splitlines() if "docker buildx imagetools create" in line]
    if len(create_lines) != 2 or any(not line.endswith('"$root_ref"') for line in create_lines):
        fail("promotion retag source is mutable")
    for required in (
        "candidate/tape-*.tar.gz candidate/tape-*.tar.gz.sha256 candidate/spdx-amd64.json candidate/spdx-arm64.json gke-receipt/tape-gke-receipt.json gke-receipt/tape-gke-receipt.json.sha256",
        "- GKE receipt SHA-256:",
        "- GKE receipt sidecar SHA-256:",
        "- Compatibility: no HTTP, CLI, OpenAPI shape, WAL format, snapshot format, CRD shape, or runtime storage default changed in this release.",
    ):
        if required not in promotion:
            fail(f"promotion release output lacks {required}")

    all_text = "\n".join(
        [
            candidate,
            promotion,
            supporting["GKE receipt maker"],
            supporting["Kind script"],
            supporting["release Dockerfile"],
        ]
    )
    if "LUMEN_" in all_text or "0.4.28" in all_text or "0.4.29" in all_text:
        fail("Tape release contract contains stale Lumen release semantics")
    if "tape.gke-release-receipt/v1" not in supporting["GKE receipt maker"] or "tape.gke-release-receipt/v1" not in supporting["artifact verifier"]:
        fail("GKE receipt schema is inconsistent")
    if '"tape-release-gates": "success"' not in supporting["GKE receipt maker"]:
        fail("GKE receipt maker does not require Tape gates")
    assert_tape_gke_failover(supporting["GKE harness"], supporting["GKE receipt maker"])
    if '"tape-release-gates":"success"' not in supporting["candidate verifier"] or '"tape-release-gates":"success"' not in supporting["artifact verifier"]:
        fail("candidate verifiers do not require Tape gates")
    kind = supporting["Kind script"]
    assert_kind_candidate_identity(kind)
    dockerfile = supporting["release Dockerfile"]
    assert_release_versions(
        supporting["Tape Cargo manifest"],
        dockerfile,
        supporting["Tape StatefulSet"],
        supporting["Tape operator Deployment"],
    )
    for required in (
        "ARG SOURCE=fetch",
        "AS binary-source-staged",
        "FROM binary-source-${SOURCE} AS binary-source",
        "COPY dist/linux/${TARGETARCH}/tape /tmp/tape",
    ):
        if required not in dockerfile:
            fail(f"release Dockerfile lacks {required}")
    for forbidden in ("ENV TAPE_DATA_DIR=", "VOLUME [", 'CMD ["serve"]'):
        if forbidden in dockerfile:
            fail("release preparation changed Tape runtime storage defaults")
    for line in dockerfile.splitlines():
        if line.startswith("FROM ") and "binary-source-${SOURCE}" not in line and "@sha256:" not in line:
            fail(f"release Dockerfile has mutable base image: {line}")
    assert_reviewed_promotion_bytes(promotion)


def assert_tape_gke_failover(harness: str, maker: str) -> None:
    required_harness = (
        'local exclude="${1:-}"',
        'if [[ "$ordinal" == "$exclude" ]]; then',
        'wait_for_replacement_pod_uid() {',
        'if [[ -n "$replacement_uid" && "$replacement_uid" != "$previous_uid" && "$ready" == "True" ]]; then',
        'initial_term="$(jq -e \'.term\' "$EVIDENCE_DIR/kubernetes/tape-raftz-initial.json")"',
        'initial_pod_uid="$(pod_uid "$initial_leader")"',
        'kubectl -n tape delete "pod/tape-${initial_leader}" --wait=false',
        'start_pod_forwards "$initial_leader"',
        'survivor_leader="$(wait_for_leader "$initial_leader")"',
        'survivor_term="$(jq -e \'.term\'',
        'if (( survivor_term <= initial_term )); then',
        'replacement_pod_uid="$(wait_for_replacement_pod_uid "$initial_leader" "$initial_pod_uid")"',
        '--arg leader_pod_uid_before "$initial_pod_uid"',
        '--arg leader_pod_uid_after "$replacement_pod_uid"',
    )
    positions: list[int] = []
    for required in required_harness:
        position = harness.find(required)
        if position < 0:
            fail(f"Tape GKE failover harness lacks {required}")
        positions.append(position)
    if positions != sorted(positions):
        fail("Tape GKE failover harness observes replacement before survivor proof")
    if harness.count('if [[ "$ordinal" == "$exclude" ]]; then') != 2:
        fail("Tape GKE failover harness does not exclude the deleted ordinal from both port-forward loops")
    for required in (
        '"leader_pod_uid_before",',
        '"leader_pod_uid_after",',
        'or not isinstance(failover.get("leader_pod_uid_before"), str)',
        'or not isinstance(failover.get("leader_pod_uid_after"), str)',
        'or failover.get("leader_pod_uid_before") == failover.get("leader_pod_uid_after")',
    ):
        if required not in maker:
            fail(f"Tape GKE receipt maker lacks strict Pod UID proof: {required}")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if text.count(old) != 1:
        fail(f"self-test fixture for {label} is not unique")
    return text.replace(old, new, 1)


def expect_static_failure(name: str, candidate: str, promotion: str) -> None:
    try:
        check_contract(candidate, promotion)
    except ContractError:
        return
    fail(f"negative control passed: {name}")


def expect_failover_failure(name: str, harness: str, maker: str) -> None:
    try:
        assert_tape_gke_failover(harness, maker)
    except ContractError:
        return
    fail(f"negative control passed: {name}")


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n")


def write_sidecar(path: Path) -> None:
    path.with_name(path.name + ".sha256").write_text(f"{sha256(path)}  {path.name}\n")


def make_archive(path: Path, target: str) -> None:
    binary = b"#!/bin/sh\nprintf 'tape 0.5.0\\n'\n"
    readme = b"Tape release fixture\n"
    with tarfile.open(path, "w:gz", format=tarfile.PAX_FORMAT) as archive:
        directory = tarfile.TarInfo(f"tape-{target}/")
        directory.type = tarfile.DIRTYPE
        directory.mode = 0o755
        archive.addfile(directory)
        for name, body, mode in (
            ("README.md", readme, 0o644),
            ("tape", binary, 0o755),
        ):
            info = tarfile.TarInfo(f"tape-{target}/{name}")
            info.size = len(body)
            info.mode = mode
            archive.addfile(info, io.BytesIO(body))


def run_checked(command: list[str], name: str, *, success: bool) -> None:
    result = subprocess.run(command, text=True, capture_output=True, check=False)
    if result.returncode == 0 and not success:
        fail(f"negative control passed: {name}")
    if result.returncode != 0 and success:
        detail = (result.stderr or result.stdout).strip()
        fail(f"positive fixture failed: {name}: {detail}")


def fixture_command(root: Path, *, commit: str, attempt: str, receipt: Path, sidecar: Path) -> list[str]:
    return [
        "bash",
        str(ARTIFACT_VERIFIER),
        "--repo",
        "chrischeng-c4/axiom",
        "--tag",
        "tape@0.5.0",
        "--commit",
        commit,
        "--candidate-run-id",
        "42",
        "--candidate-run-attempt",
        attempt,
        "--mode",
        "fixture",
        "--candidate-receipt-dir",
        str(root / "candidate"),
        "--release-assets-dir",
        str(root / "release"),
        "--gke-receipt",
        str(receipt),
        "--gke-receipt-sidecar",
        str(sidecar),
    ]


def maker_command(
    root: Path,
    acceptance: Path,
    cleanup: Path,
    output: Path,
    *,
    run: Path | None = None,
) -> list[str]:
    return [
        sys.executable,
        str(GKE_MAKER),
        "--candidate-manifest",
        str(root / "candidate/final-candidate-manifest.json"),
        "--run",
        str(run or root / "evidence/run.json"),
        "--images",
        str(root / "evidence/images.json"),
        "--acceptance",
        str(acceptance),
        "--cleanup",
        str(cleanup),
        "--output",
        str(output),
    ]


def build_fixture(root: Path) -> tuple[str, dict[str, object], dict[str, object]]:
    candidate = root / "candidate"
    release = root / "release"
    evidence = root / "evidence"
    candidate.mkdir()
    release.mkdir()
    evidence.mkdir()
    artifacts = []
    for target in TARGETS:
        archive = candidate / f"tape-{target}.tar.gz"
        make_archive(archive, target)
        write_sidecar(archive)
        sidecar = archive.with_name(archive.name + ".sha256")
        artifacts.append(
            {
                "target": target,
                "archive": archive.name,
                "archive_sha256": sha256(archive),
                "sidecar": sidecar.name,
                "sidecar_sha256": sha256(sidecar),
            }
        )
    for arch in ("amd64", "arm64"):
        write_json(candidate / f"spdx-{arch}.json", {"spdxVersion": "SPDX-2.3"})
    commit = "a" * 40
    root_digest = "sha256:" + "1" * 64
    manifest = {
        "schema": "cclab.tape.candidate-manifest.v1",
        "repository": "chrischeng-c4/axiom",
        "workflow_path": ".github/workflows/tape-release-candidate.yml",
        "workflow_id": 1,
        "run_id": "42",
        "run_attempt": "3",
        "run_url": "https://github.com/chrischeng-c4/axiom/actions/runs/42/attempts/3",
        "source_ref": "refs/heads/main",
        "workflow_ref": "chrischeng-c4/axiom/.github/workflows/tape-release-candidate.yml@refs/heads/main",
        "commit": commit,
        "version": "0.5.0",
        "tag": "tape@0.5.0",
        "candidate_tag": "release-candidate-42-3",
        "pr": {"number": 1, "url": "https://github.com/chrischeng-c4/axiom/pull/1"},
        "image": {
            "repository": "ghcr.io/chrischeng-c4/tape",
            "root_digest": root_digest,
            "amd64_digest": "sha256:" + "2" * 64,
            "arm64_digest": "sha256:" + "3" * 64,
        },
        "artifacts": artifacts,
        "sboms": {
            "amd64": {"file": "spdx-amd64.json", "sha256": sha256(candidate / "spdx-amd64.json")},
            "arm64": {"file": "spdx-arm64.json", "sha256": sha256(candidate / "spdx-arm64.json")},
        },
        "jobs": FINAL_JOBS,
    }
    manifest_path = candidate / "final-candidate-manifest.json"
    write_json(manifest_path, manifest)
    write_sidecar(manifest_path)
    run = {
        "schema": "axiom.gcp.operator.run.v1",
        "project_id": "axiom-502607",
        "region": "asia-east1",
        "gke_zone": "asia-east1-a",
        "run_id": "t0500fixture",
        "git_sha": commit[:12],
        "git_dirty": False,
        "image_provenance": "prebuilt",
    }
    write_json(evidence / "run.json", run)
    write_json(evidence / "images.json", {"tape": f"ghcr.io/chrischeng-c4/tape@{root_digest}"})
    tape = {
        "schema": "axiom.gcp.tape.acceptance.v1",
        "operator_reconcile_1x1": "passed",
        "append_replay_lifecycle": "passed",
        "subscription_pull_ack_cursor": "passed",
        "subscription_lag_gauge": "passed",
        "pod_restart_data_retention": "passed",
        "gcs_backup": "passed",
        "gcs_object": "gs://redacted/object.json",
        "gcs_object_bytes": 128,
        "cold_restore_from_backup": "passed",
        "bootstrap_seed_uri_restart": "passed",
        "seed_cleared_rolling_restart_retention": "passed",
        "topology_1_to_3": {"from": 1, "to": 3, "ready_pods": 3},
        "raft_failover": {
            "leader_before": "0",
            "leader_after": "1",
            "leader_pod_uid_before": "leader-pod-uid",
            "leader_pod_uid_after": "replacement-pod-uid",
            "distinct": True,
            "term_before": 1,
            "term_after": 2,
            "leader_pod_replaced": "passed",
        },
        "post_failover_write_committed": "passed",
    }
    acceptance = {
        "schema": "axiom.gcp.operator.acceptance.v1",
        "project_id": run["project_id"],
        "region": run["region"],
        "run_id": run["run_id"],
        "backup_bucket": "redacted",
        "acceptance": {"tape": tape},
    }
    cleanup = {
        "schema": "axiom.gcp.operator.cleanup.v1",
        "project_id": run["project_id"],
        "region": run["region"],
        "gke_zone": run["gke_zone"],
        "run_id": run["run_id"],
        "verified_at": "2026-01-01T00:00:00Z",
        "status": "clean",
        "preserved": {"artifact_registry": True, "preexisting_apis": True},
    }
    write_json(evidence / "acceptance.json", acceptance)
    write_json(evidence / "cleanup.json", cleanup)
    receipt = root / "tape-gke-receipt.json"
    run_checked(
        maker_command(root, evidence / "acceptance.json", evidence / "cleanup.json", receipt),
        "GKE receipt fixture",
        success=True,
    )
    for source in [
        *(candidate / f"tape-{target}.tar.gz" for target in TARGETS),
        *(candidate / f"tape-{target}.tar.gz.sha256" for target in TARGETS),
        candidate / "spdx-amd64.json",
        candidate / "spdx-arm64.json",
        receipt,
        receipt.with_name(receipt.name + ".sha256"),
    ]:
        shutil.copy2(source, release / source.name)
    return commit, acceptance, cleanup


def self_test() -> None:
    candidate = CANDIDATE_PATH.read_text()
    promotion = PROMOTION_PATH.read_text()
    original_hashes = {
        path: sha256(path) for path in (CANDIDATE_PATH, PROMOTION_PATH, KIND_SCRIPT)
    }
    check_contract(candidate, promotion)

    static_mutations = {
        "tag-first-build": (
            candidate,
            replace_once(promotion, "on:\n  workflow_dispatch:", 'on:\n  push:\n    tags: ["tape@*"]\n  workflow_dispatch:', "tag-first-build"),
        ),
        "mutable-action-tag": (
            replace_once(
                candidate,
                "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1\n        with:\n          ref: ${{ github.sha }}",
                "actions/checkout@v4\n        with:\n          ref: ${{ github.sha }}",
                "mutable-action-tag",
            ),
            promotion,
        ),
        "missing-tape-gate-comment-fake": (
            replace_once(candidate, "          cargo test --locked -p tape --features operator,backup\n", "          # cargo test --locked -p tape --features operator,backup\n", "missing-tape-gate-comment-fake"),
            promotion,
        ),
        "missing-tape-gate-quoted-fake": (
            replace_once(candidate, "          cargo test --release --locked -p tape --test tape_perf_gate\n", "          echo 'cargo test --release --locked -p tape --test tape_perf_gate'\n", "missing-tape-gate-quoted-fake"),
            promotion,
        ),
        "missing-library-gate": (
            replace_once(candidate, "          bash scripts/faberline-core-test.sh service-k8s storage-durable service-backup raft-core raft-runtime\n", "          # bash scripts/faberline-core-test.sh service-k8s storage-durable service-backup raft-core raft-runtime\n", "missing-library-gate"),
            promotion,
        ),
        "missing-result-binding": (
            replace_once(candidate, "needs: [identity, build, tape-release-gates, manifest", "needs: [identity, build, manifest", "missing-result-binding"),
            promotion,
        ),
        "promotion-rebuild-command": (
            candidate,
            replace_once(
                promotion,
                "      pull-requests: read\n    steps:\n",
                "      pull-requests: read\n    steps:\n      - run: docker build .\n",
                "promotion-rebuild-command",
            ),
        ),
        "promotion-rebuild-line-continuation": (
            candidate,
            replace_once(
                promotion,
                "      pull-requests: read\n    steps:\n",
                "      pull-requests: read\n    steps:\n      - run: |\n          docker \\\n            build .\n",
                "promotion-rebuild-line-continuation",
            ),
        ),
        "promotion-rebuild-shell-wrapper": (
            candidate,
            replace_once(
                promotion,
                "      pull-requests: read\n    steps:\n",
                "      pull-requests: read\n    steps:\n      - run: bash -c 'docker build .'\n",
                "promotion-rebuild-shell-wrapper",
            ),
        ),
        "promotion-rebuild-shell-path-wrapper": (
            candidate,
            replace_once(
                promotion,
                "      pull-requests: read\n    steps:\n",
                "      pull-requests: read\n    steps:\n      - run: /bin/bash -c 'docker build .'\n",
                "promotion-rebuild-shell-path-wrapper",
            ),
        ),
        "promotion-rebuild-shell-option-wrapper": (
            candidate,
            replace_once(
                promotion,
                "      pull-requests: read\n    steps:\n",
                "      pull-requests: read\n    steps:\n      - run: bash --noprofile -c 'docker build .'\n",
                "promotion-rebuild-shell-option-wrapper",
            ),
        ),
        "promotion-rebuild-shell-variable-wrapper": (
            candidate,
            replace_once(
                promotion,
                "      pull-requests: read\n    steps:\n",
                "      pull-requests: read\n    steps:\n      - run: |\n          cmd='docker build .'\n          bash -c \"$cmd\"\n",
                "promotion-rebuild-shell-variable-wrapper",
            ),
        ),
        "promotion-rebuild-dynamic-shell-command": (
            candidate,
            replace_once(
                promotion,
                "      pull-requests: read\n    steps:\n",
                "      pull-requests: read\n    steps:\n      - run: |\n          runner=bash\n          \"$runner\" -c 'docker build .'\n",
                "promotion-rebuild-dynamic-shell-command",
            ),
        ),
        "promotion-mutable-source": (
            candidate,
            replace_once(promotion, 'docker buildx imagetools create --tag "$semver_ref" "$root_ref"', 'docker buildx imagetools create --tag "$semver_ref" "${image_repo}:${version}"', "promotion-mutable-source"),
        ),
        "missing-run-attempt": (
            candidate,
            promotion.replace("candidate_run_attempt", "candidate_attempt_removed"),
        ),
        "missing-receipt-sidecar-hash": (
            candidate,
            promotion.replace("gke_receipt_sidecar_sha256", "gke_sidecar_hash_removed"),
        ),
    }
    for name, (candidate_mutation, promotion_mutation) in static_mutations.items():
        expect_static_failure(name, candidate_mutation, promotion_mutation)

    harness = GKE_HARNESS.read_text()
    maker = GKE_MAKER.read_text()
    assert_tape_gke_failover(harness, maker)
    failover_mutations = {
        "failover-delete-waits-for-replacement": replace_once(
            harness,
            'kubectl -n tape delete "pod/tape-${initial_leader}" --wait=false',
            'kubectl -n tape delete "pod/tape-${initial_leader}" --wait=true',
            "failover-delete-waits-for-replacement",
        ),
        "failover-forwarding-includes-deleted-ordinal": replace_once(
            harness,
            'start_pod_forwards "$initial_leader"',
            "start_pod_forwards",
            "failover-forwarding-includes-deleted-ordinal",
        ),
        "failover-skips-only-one-port-forward-loop": harness.replace(
            'if [[ "$ordinal" == "$exclude" ]]; then',
            'if false; then',
            1,
        ),
        "failover-survivor-does-not-exclude-original": replace_once(
            harness,
            'survivor_leader="$(wait_for_leader "$initial_leader")"',
            'survivor_leader="$(wait_for_leader)"',
            "failover-survivor-does-not-exclude-original",
        ),
        "failover-omits-replacement-uid-readiness-check": replace_once(
            harness,
            'if [[ -n "$replacement_uid" && "$replacement_uid" != "$previous_uid" && "$ready" == "True" ]]; then',
            'if false; then',
            "failover-omits-replacement-uid-readiness-check",
        ),
    }
    for name, mutation in failover_mutations.items():
        expect_failover_failure(name, mutation, maker)

    cargo = CARGO_TOML.read_text()
    dockerfile = DOCKERFILE.read_text()
    statefulset = STATEFULSET.read_text()
    operator = OPERATOR_DEPLOYMENT.read_text()
    version = assert_release_versions(cargo, dockerfile, statefulset, operator)
    version_mutations = {
        "perf-target-default-selection": (
            replace_once(
                cargo,
                'path = "e2e/tape_perf_gate.rs"\ntest = false',
                'path = "e2e/tape_perf_gate.rs"',
                "perf-target-default-selection",
            ),
            dockerfile,
            statefulset,
            operator,
        ),
        "cargo-version-pin": (
            replace_once(
                cargo,
                f'version = "{version}"',
                'version = "9.9.9"',
                "cargo-version-pin",
            ),
            dockerfile,
            statefulset,
            operator,
        ),
        "dockerfile-version-pin": (
            cargo,
            replace_once(
                dockerfile,
                f"ARG TAPE_VERSION=tape@{version}",
                "ARG TAPE_VERSION=tape@9.9.9",
                "dockerfile-version-pin",
            ),
            statefulset,
            operator,
        ),
        "statefulset-version-pin": (
            cargo,
            dockerfile,
            replace_once(
                statefulset,
                f"image: ghcr.io/chrischeng-c4/tape:{version}",
                "image: ghcr.io/chrischeng-c4/tape:9.9.9",
                "statefulset-version-pin",
            ),
            operator,
        ),
        "operator-version-pin": (
            cargo,
            dockerfile,
            statefulset,
            replace_once(
                operator,
                f"image: ghcr.io/chrischeng-c4/tape:{version}",
                "image: ghcr.io/chrischeng-c4/tape:9.9.9",
                "operator-version-pin",
            ),
        ),
    }
    for name, values in version_mutations.items():
        try:
            assert_release_versions(*values)
        except ContractError:
            continue
        fail(f"negative control passed: {name}")

    kind = KIND_SCRIPT.read_text()
    assert_kind_candidate_identity(kind)
    kind_mutations = {
        "kind-statefulset-server-selector": (
            replace_once(
                kind,
                KIND_SERVER_STATEFULSET_SELECTOR,
                KIND_SERVER_STATEFULSET_SELECTOR.replace('name=="server"', 'name=="tape"'),
                "kind-statefulset-server-selector",
            ),
            "Kind prebuilt identity check lacks operator-rendered server StatefulSet selector",
        ),
        "kind-server-pod-inventory": (
            replace_once(
                kind,
                KIND_SERVER_POD_INVENTORY,
                KIND_SERVER_POD_INVENTORY.replace(" server 1", " tape 1"),
                "kind-server-pod-inventory",
            ),
            "Kind prebuilt identity check lacks operator-rendered server pod inventory",
        ),
    }
    for name, (mutation, expected) in kind_mutations.items():
        try:
            assert_kind_candidate_identity(mutation)
        except ContractError as error:
            if str(error) != expected:
                fail(f"negative control {name} failed for the wrong reason: {error}")
            continue
        fail(f"negative control passed: {name}")

    with tempfile.TemporaryDirectory(prefix="tape-release-contract-") as tmp:
        fixture = Path(tmp)
        commit, acceptance, cleanup = build_fixture(fixture)
        receipt = fixture / "tape-gke-receipt.json"
        receipt_sidecar = fixture / "tape-gke-receipt.json.sha256"
        run_checked(
            fixture_command(fixture, commit=commit, attempt="3", receipt=receipt, sidecar=receipt_sidecar),
            "release fixture",
            success=True,
        )
        run_checked(
            fixture_command(fixture, commit="b" * 40, attempt="3", receipt=receipt, sidecar=receipt_sidecar),
            "wrong-commit",
            success=False,
        )
        run_checked(
            fixture_command(fixture, commit=commit, attempt="4", receipt=receipt, sidecar=receipt_sidecar),
            "wrong-run-attempt",
            success=False,
        )
        bad_receipt = json.loads(receipt.read_text())
        bad_receipt["candidate"]["root_digest"] = "sha256:" + "4" * 64
        bad_receipt_path = fixture / "wrong-image-receipt.json"
        write_json(bad_receipt_path, bad_receipt)
        bad_receipt_sidecar = fixture / "wrong-image-receipt.json.sha256"
        bad_receipt_sidecar.write_text(f"{sha256(bad_receipt_path)}  tape-gke-receipt.json\n")
        run_checked(
            fixture_command(fixture, commit=commit, attempt="3", receipt=bad_receipt_path, sidecar=bad_receipt_sidecar),
            "wrong-image-digest",
            success=False,
        )
        bad_sidecar = fixture / "bad-receipt-sidecar.sha256"
        bad_sidecar.write_text(f"{'0' * 64}  tape-gke-receipt.json\n")
        run_checked(
            fixture_command(fixture, commit=commit, attempt="3", receipt=receipt, sidecar=bad_sidecar),
            "receipt-sidecar-mismatch",
            success=False,
        )
        incomplete = copy.deepcopy(acceptance)
        del incomplete["acceptance"]["tape"]["post_failover_write_committed"]
        incomplete_path = fixture / "evidence/incomplete-acceptance.json"
        write_json(incomplete_path, incomplete)
        run_checked(
            maker_command(fixture, incomplete_path, fixture / "evidence/cleanup.json", fixture / "incomplete-receipt.json"),
            "incomplete-gke-functional-result",
            success=False,
        )
        same_leader = copy.deepcopy(acceptance)
        same_leader["acceptance"]["tape"]["raft_failover"]["leader_after"] = "0"
        same_leader_path = fixture / "evidence/same-leader-acceptance.json"
        write_json(same_leader_path, same_leader)
        run_checked(
            maker_command(fixture, same_leader_path, fixture / "evidence/cleanup.json", fixture / "same-leader-receipt.json"),
            "same-failover-leader",
            success=False,
        )
        same_uid = copy.deepcopy(acceptance)
        same_uid["acceptance"]["tape"]["raft_failover"]["leader_pod_uid_after"] = "leader-pod-uid"
        same_uid_path = fixture / "evidence/same-uid-acceptance.json"
        write_json(same_uid_path, same_uid)
        run_checked(
            maker_command(fixture, same_uid_path, fixture / "evidence/cleanup.json", fixture / "same-uid-receipt.json"),
            "same-failover-pod-uid",
            success=False,
        )
        missing_uid = copy.deepcopy(acceptance)
        del missing_uid["acceptance"]["tape"]["raft_failover"]["leader_pod_uid_after"]
        missing_uid_path = fixture / "evidence/missing-uid-acceptance.json"
        write_json(missing_uid_path, missing_uid)
        run_checked(
            maker_command(fixture, missing_uid_path, fixture / "evidence/cleanup.json", fixture / "missing-uid-receipt.json"),
            "missing-failover-pod-uid",
            success=False,
        )
        non_increasing_term = copy.deepcopy(acceptance)
        non_increasing_term["acceptance"]["tape"]["raft_failover"]["term_after"] = 1
        non_increasing_term_path = fixture / "evidence/non-increasing-term-acceptance.json"
        write_json(non_increasing_term_path, non_increasing_term)
        run_checked(
            maker_command(fixture, non_increasing_term_path, fixture / "evidence/cleanup.json", fixture / "non-increasing-term-receipt.json"),
            "non-increasing-failover-term",
            success=False,
        )
        dirty = copy.deepcopy(cleanup)
        dirty["status"] = "dirty"
        dirty_path = fixture / "evidence/dirty-cleanup.json"
        write_json(dirty_path, dirty)
        run_checked(
            maker_command(fixture, fixture / "evidence/acceptance.json", dirty_path, fixture / "dirty-receipt.json"),
            "cleanup-not-clean",
            success=False,
        )
        missing_location = json.loads((fixture / "evidence/run.json").read_text())
        for field in ("project_id", "region", "gke_zone"):
            missing_location.pop(field)
        missing_location_path = fixture / "evidence/missing-location-run.json"
        write_json(missing_location_path, missing_location)
        run_checked(
            maker_command(
                fixture,
                fixture / "evidence/acceptance.json",
                fixture / "evidence/cleanup.json",
                fixture / "missing-location-receipt.json",
                run=missing_location_path,
            ),
            "missing-gke-location-identity",
            success=False,
        )

    for path, before in original_hashes.items():
        if sha256(path) != before:
            fail(f"self-test did not restore {path.relative_to(ROOT)} byte for byte")
    print("release contract self-test passed: positive fixture plus 39 negative mutations")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    try:
        if args.self_test:
            self_test()
        else:
            check_contract(CANDIDATE_PATH.read_text(), PROMOTION_PATH.read_text())
            print("release contract passed")
    except ContractError as error:
        raise SystemExit(f"release contract refused: {error}") from error


if __name__ == "__main__":
    main()
