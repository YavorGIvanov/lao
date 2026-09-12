# Local Agent Optimizer

Local Agent Optimizer (LAO) is an open-source layer that lets users keep Codex or Claude Code while a conservative router delegates bounded work to a local model. The cloud harness remains the planner and verifier; uncertain work stays Cloud.

This is a working Apple Silicon research proof. Local execution works, but faster tasks, better quality, and net savings have not been established. Signed distribution and release certification remain open; see the [release plan](IMPLEMENTATION_PLAN.md#next-steps).

## Install

LAO detects Codex, Claude Code, or both on PATH and manages only the selected clients. It never reads or copies their credential stores.

For a source installation, install Git and Rust with Cargo, then run:

```sh
git clone https://github.com/YavorGIvanov/lao.git && cd lao && ./install.sh && lao install
```

`lao install` checks machine fit, downloads verified runtime/model artifacts (about 2.7 GB initially), starts the service, applies settings transactionally, and warms the local path in the background. Unsupported configurations stop safely. Partial installation rolls back; recovery conflicts retain snapshots for `lao off`. Reinstallation verifies and reuses a healthy setup.

To manage one client when both are present, use `lao install --client codex` or `lao install --client claude`; `--client both` requires both. Upgrades retain the selection; run `lao off` before changing it. Inspect proposed settings with `lao preview --client codex` or `--client claude`.

Minimum versions are Codex 0.151.0 and Claude Code 2.1.251. Newer releases are admitted when CLI capability checks pass, without an allowlist update. Preflight does not certify future versions or native-cloud behavior. Local lifecycle and synthetic protocol checks passed on Codex 0.153.4 and Claude Code 2.1.251; see [compatibility evidence](IMPLEMENTATION_PLAN.md#r11--distribute-without-setup-barriers).

For contributor testing, `sh package.sh` builds `target/release/lao-macos-arm64.tar.gz` on Apple Silicon. Extract the archive, run its `install.sh`, and follow the printed setup command. The receiving Mac needs no source checkout, Git, Rust, or Cargo. These archives are unsigned and not published releases; checksums detect corruption, not publisher identity. The archive includes [dependency and model notices](THIRD_PARTY_NOTICES.txt), covered by its checksum manifest. The binary installer stages upgrades, rolls back command failures, and recovers interrupted replacements on the next run, including after SIGKILL. Conflicting edits or damaged recovery records retain snapshots for inspection. Signing, notarization, physical power-loss testing and full service/settings upgrade certification remain open.

## Normal use

Keep using `codex` or `claude` in your project. The harness can call LAO's `execute` tool for a bounded implementation packet:

```text
Codex / Claude planner
        ↓ one bounded packet
LAO semantic router
   ├─ Cloud → current harness continues
   └─ Local → OpenCode → Qwen3 / llama.cpp
```

Planning, broad changes, and uncertain work stay Cloud. Each Local packet gets fresh disposable state and exact file permissions, enforced by worker tools and a macOS sandbox. The harness must review the actual diff and verify the outcome, even if the worker reports completion. Installed settings auto-approve only `lao.execute`.

## Test the experience

```sh
lao status  # Service, selected clients, and local cache readiness; no model request
lao smoke   # Selected harnesses and local model; sanitized pass/fail and elapsed time
```

Cloud work remains available while the local cache warms. To disable LAO and restore managed settings:

```sh
lao off
```

This stops owned services, restores unchanged settings exactly, preserves unrelated client edits, and refuses conflicts in LAO-owned entries. Failed workers may leave edits: inspect them before retrying. See the [architecture map](architecture.html#failure-title) for trust boundaries and recovery limits.

`off` retains downloaded models, runtimes, router/worker caches, and installed binaries. To restore settings and remove those artifacts too, run:

```sh
lao uninstall
```

This deletes LAO's private application state and cache directories, its installed binary pair, ownership receipt, and matching command links. It preserves client homes, external runtimes, unrelated files, and your source checkout or archive. For custom binary locations, supply the same `LAO_PREFIX` and `LAO_BIN_DIR` used with `install.sh`. Conflicting settings, changed binaries or pending binary recovery stop removal. Older source installs without an ownership receipt need `install.sh` rerun first. If removal was interrupted after deleting the CLI, rerun the archive's `bin/lao uninstall` with the same locations.

## Manifesto

### Product

- Keep the user's harness.
- Stay invisible by default.
- Keep setup to one command.
- Ask only at real trust boundaries.
- Never ask twice for the same consent.
- Improve a little at first.
- Learn from measured outcomes.
- Push the practical boundary of speed and efficiency.
- Treat mediocre performance as unfinished work.
- Move reusable work off the user's critical path.
- Never make the user wait for work that can happen safely in the background.
- Use local only when it helps.
- Keep cloud as the safe path.
- Never consume the whole machine.
- Measure fit before download.
- Measure quality before promotion.
- Keep user data local and encrypted.
- Ask before capture, cloud eval, spend, or training.
- Make every recommendation explainable.
- Prefer evidence to claims.

### Code

- Choose the simplest elegant solution that works.
- Write less code.
- Keep names short.
- Keep comments rare.
- Comment why, not what.
- Reuse before rebuilding.
- Pin what we reuse.
- Add code only for a proven need.
- Hide upstream details behind our API.
- Own a component only when evidence justifies it.
- Build one narrow vertical slice at a time.
- Delete before abstracting.
- End every change with a careful simplification pass.
- Avoid speculative frameworks.
- Optimize for a human reading the code tomorrow.
- Make invalid states hard to express.
- Fail closed at trust boundaries.
- Test contracts and outcomes.
- Test real paths end to end when cheap.
- Keep the hot path small.
- Measure and optimize the hot path before accepting it.

### Architecture

- One monorepo is not one monolith.
- Every component owns its state.
- No shared database.
- Components communicate through versioned APIs only.
- Components never import sibling implementations.
- Keep optimization policy and state in its own component.
- Only apps wire concrete components together.
- Link the hot path when it saves resources.
- Isolate secrets, data, runtimes, eval, and training.
- Scaffold every strategic boundary early.
- Implement deferred behavior only when needed.
- Future-proof with seams and fixtures, not extra machinery.
- Keep extraction possible, not mandatory.
- Keep the visual architecture map current with the code.

## Contributor workflow

Read [AGENTS.md](AGENTS.md) for shared instructions and required checks, the [visual architecture map](architecture.html) to find code owners, and [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) for acceptance criteria. Any coding agent or harness can use the guide; supply it as repository context if it is not loaded automatically.

The [product vision](PRODUCT_VISION_AND_ARCHITECTURE.md) explains longer-term decisions. Deferred features are not instructions to build them now. Installed-client and model tests remain opt-in; archive installation checks run with `sh test/install.sh`.

## Status

The proof supports native Responses/Messages traffic through a private credential gate and bounded MCP packets through OpenCode. The macOS worker sandbox has no unrestricted fallback, but uses deprecated `sandbox-exec` and is not portable release hardening.

The [paired-evidence checker and public fixtures](docs/benchmarks/README.md) are implemented. R9b now has a [registered subscription workflow collector](docs/benchmarks/workflow.md), twelve bounded public tasks and a reserved repository holdout. The Codex comparison and separate two-task Claude smoke are authorized. The complete cold comparison verified 36/36 native Cloud objectives and 22/36 hybrid objectives; all hybrid delegations returned Cloud. Warm and Claude collection are paused at the user’s request. Local canaries do not establish user benefit.

Capture, encrypted task storage, personal evaluation, and training remain disabled. Linux/Windows, NVIDIA/AMD, and additional certified engines follow the first Mac release. Current implementation and dated evidence belong in the [implementation plan](IMPLEMENTATION_PLAN.md).

## Advanced configuration

The default is `--router semantic --runtime llama-cpp`. `--router safe` keeps automatic work in Cloud; `--router vllm-semantic` uses a user-managed vLLM Semantic Router decision endpoint.

`--runtime external` connects to an existing protected IPv4-loopback endpoint:

```sh
LAO_EXTERNAL_ADDR=127.0.0.1:8000 \
LAO_EXTERNAL_KEY_FILE=/absolute/path/to/owner-only/runtime.key \
lao install --runtime external
```

LAO does not install or manage that server. vLLM and SGLang are candidate implementations behind this API, not certified integrations.
