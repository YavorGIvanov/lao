# Stage 1 implementation plan

## Outcome

Ship the smallest real proof of this product:

```text
Codex + Claude Code → LAO gate → conservative router → llama.cpp or native cloud
```

The user keeps each coding harness and its existing login. Cloud remains the default. One explicit, bounded, text-only task may run on one verified local model. `lao off` restores the original client configuration even if the daemon is unavailable.

Stage 1 targets this 24 GiB Apple M4 Mac. It is a working end-to-end proof, not the final router, catalog, evaluator, or cross-platform release.

## Rules

- Follow [AGENTS.md](AGENTS.md) and the [README manifesto](README.md).
- Build one vertical slice. Add nothing that it does not exercise.
- Use real Codex, Claude Code, llama.cpp, and saved-login paths when the check is safe and cheap.
- Never read or copy a harness credential.
- Keep cloud as the default until local fit and protocol behavior are proven.
- Keep every noncritical component as a disabled draft.
- Finish every task with focused adversarial review and simplification.
- Keep [architecture.html](architecture.html), this plan, the README, and the [product architecture](PRODUCT_VISION_AND_ARCHITECTURE.md) consistent.

## Active components

Only these components may grow during Stage 1:

| Component | Responsibility |
|---|---|
| `svc/codex`, `svc/claude` | supported client detection and exact settings transaction |
| `svc/gate` | local caller check, credential isolation, protocol ingress and exact egress |
| `svc/route` | cloud-safe decision; one explicit local canary later |
| `svc/run` | Apple fit guard and one owned llama.cpp child |
| `svc/model` | one immutable artifact record and verified local file |
| `app/daemon` | compose the request path and adopt the launchd listener |
| `app/cli` | `install`, preview, doctor, smoke, and `off` only |

The matching `api/*` packages remain the semantic boundaries. Services never import sibling implementations; applications wire them.

`capture`, `vault`, `eval`, `train`, their workers, generic local RPC, and future backends remain untouched disabled drafts. Do not finalize or expand them in Stage 1.

## Proof ledger

Already proven and retained:

- the 29-package boundary skeleton and architecture checker;
- streaming/keep-alive transport prototype;
- isolated installed-client probes for Codex and Claude Code;
- saved-login native cloud E2Es through the private gate for both clients;
- caller-token, credential-origin, path, header, TLS, redirect, and cancellation gates;
- a separate router contract that currently always chooses cloud;
- launchd listener adoption and opt-in crash/prebind lifecycle proof;
- pinned llama.cpp supervision with a private loopback bearer, real Qwen output, stop, and port reuse.

Not yet complete:

- P0-05 transactional persistent client configuration;
- a production local protocol bridge from either harness to llama.cpp;
- one owned artifact flow;
- the installed local/cloud/off acceptance run.

## Stage 1 tasks

### S1-01 — Apple runtime guard

Status: complete (2026-08-30).

Keep the hardware work inside `run`; add no hardware package.

- Detect the 24 GiB Apple unified pool once. Never add Metal memory to host memory.
- Resolve Light, Auto, and Maximum with the documented non-negative formula.
- Use the pinned llama.cpp device report as the Metal working-set cap.
- Read current macOS memory availability and pressure immediately before a cold load.
- Disable llama.cpp's unbudgeted prompt cache, use one slot, and keep threads bounded.
- Reject a working-set estimate above the fresh budget.
- Require the loaded context to equal the artifact's supported context.
- Own process stop and listener cleanup.

Stage 1 uses Light and the verified 32K Qwen lifecycle fixture. Current measured 24 GiB ceilings at 72% OS availability are approximately 6.0 GiB Light, 9.28 GiB Auto, and 11.28 GiB Maximum; these values change with live pressure.

Acceptance:

- budget is resolved before load and pressure denies a new load;
- the real 32K fixture stays below the 6 GiB Light ceiling;
- llama.cpp reports an effective 32K context rather than silently shrinking it;
- unauthorized local access is rejected and a real model request succeeds;
- stop leaves no process, key file, or listener;
- focused tests, lint, boundary check, and extraction pass.

Deferred: discrete GPU pools, cgroups, Linux/Windows probes, multi-GPU, automatic resident polling, and GPU utilization sampling.

### S1-02 — One artifact

Status: pending. Depends on S1-01.

Use `model` for one compiled-in artifact record only:

- upstream URL and immutable revision;
- expected byte length and SHA-256;
- license, template, native context, llama.cpp build, and expected working set;
- bounded temporary download followed by hash verification and atomic rename;
- reuse the already verified cached file when it matches.

Acceptance:

- preview shows exact download size, resolved memory budget, context, and artifact identity;
- wrong length or hash is rejected before promotion;
- one verified file is handed to `run` through the existing API;
- no signed catalog, recommendation engine, resume system, LRU, or preference database.

### S1-03 — One local protocol slice

Status: pending. Depends on S1-01 and S1-02.

Wire `gate`, `route`, and `run` in the daemon.

- Keep every normal request on native cloud.
- Add one explicit, non-secret local canary selector.
- Accept only bounded text input with no tools, images, audio, reasoning continuation, or unknown required fields.
- Translate one Responses request and one Messages request into the local llama.cpp call.
- Return the smallest correct streaming event sequence each harness needs.
- Strip every native credential and provider-specific secret before local egress.
- Reject unsupported local requests before output; do not silently change route mid-task.

Acceptance:

- real installed Codex and Claude Code each complete the same harmless local turn;
- llama.cpp receives no native credential, caller token, or provider-only header;
- ordinary requests still pass through to native cloud unchanged;
- cancellation stops local generation and no retry crosses a side-effect boundary.

### S1-04 — Transactional `install` and `off`

Status: pending. This is the remaining P0-05 work. Depends on S1-03.

Manage only the exact supported Codex and Claude Code settings.

- Bind and verify the launchd-owned listener before changing a client.
- Generate separate caller tokens for Codex and Claude.
- Never open either harness's auth store.
- Detect conflicting provider/auth configuration and fail without writes.
- Preserve original bytes and permissions.
- Use one owner-only lock and a minimal crash record.
- Apply each client independently and roll back partial failure.
- `lao off` restores original bytes with the daemon stopped.
- Install no hooks.

Acceptance:

- preview contains no live token;
- induced failure at each actual write boundary restores the original file;
- concurrent install is rejected cleanly;
- user edits after install are not overwritten blindly;
- off and uninstall never need provider credentials.

### S1-05 — Real installed acceptance

Status: pending. Depends on S1-04.

Run once from a clean supported state:

1. `lao install`.
2. One normal Codex cloud request using its saved login.
3. One normal Claude cloud request using its saved login.
4. One explicit bounded local canary through Codex.
5. The same local canary through Claude.
6. Restart the daemon and verify the path again.
7. `lao off` and verify byte-identical client restoration.

Record versions, routes, latency, context, peak worker RSS, and cleanup outcome. Never record credentials or raw private output.

Acceptance:

- both cloud and local outcomes succeed through both real harnesses;
- cloud remains the default;
- no credential reaches llama.cpp, logs, stdout, stderr, or support data;
- no orphan worker or listener remains;
- the user sees no repeated permission prompt during normal use;
- client settings are restored exactly.

### S1-06 — Adversarial review

Status: pending. Depends on S1-05.

Review only the Stage 1 trust boundaries:

- credential destination and caller confusion;
- listener ownership before configuration;
- unsupported input reaching local inference;
- memory/pressure escape;
- worker and socket cleanup;
- crash-safe install/off rollback.

Fix blockers. Do not expand the review into deferred products.

### S1-07 — Simplify and hand off

Status: mandatory final step. Depends on S1-06.

- Read the manifesto again.
- Remove unused types, options, tests, dependencies, prose, and indirection.
- Keep only evidence that protects a Stage 1 requirement.
- Run formatting, focused and workspace tests, Clippy with warnings denied, `cargo xtask check`, `cargo xtask extract`, and `git diff --check`.
- Update all four living documents together.
- Commit as `YavorGIvanov <yavorgenadiev@gmail.com>` with no other author or coauthor.

## Stage 1 exit gate

Stage 1 is complete only when a new user on the supported Mac can:

```text
install once → keep using Codex or Claude → use cloud normally → request one local canary → turn LAO off cleanly
```

The result must use saved harness authentication without LAO reading the real token, keep local inference within Light mode, and restore both clients exactly.

## Deferred backlog

After the exit gate, choose the next measured bottleneck. The long-term contracts and constraints remain in the product architecture.

Deferred product work:

- automatic difficulty routing, task stickiness, repair, escalation, and circuit breakers;
- model catalog signatures, multiple models, preferences, recommendations, and llama-swap;
- Ollama, LM Studio, ShoeHorn, FreeToken, NVIDIA, Linux, and Windows;
- hooks and task-boundary tracking;
- consented capture, scrub, snapshots, encrypted vault, retention, export, and deletion;
- personal replay evals, proprietary-model campaigns, reports, and promotion workflow;
- explicit training consent, dataset lineage, adapters, tuning, and rollback;
- background controls, observability, support bundles, updater, packaging, and release hardening;
- product-scale metrics and design-partner rollout.

These components stay independently drafted. They are not prerequisites for proving the core product loop.
