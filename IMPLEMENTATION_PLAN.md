# Implementation plan

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
| `svc/route` | cloud-safe decision; one explicit local canary |
| `svc/run` | Apple fit guard and one owned llama.cpp child |
| `svc/model` | one immutable artifact record and verified local file |
| `app/daemon` | compose the request path and adopt the launchd listener |
| `app/cli` | `install`, preview, smoke, and `off` only |

The matching `api/*` packages remain the semantic boundaries. Services never import sibling implementations; applications wire them.

`capture`, `vault`, `eval`, `train`, their workers, generic local RPC, and future backends remain untouched disabled drafts. Do not finalize or expand them in Stage 1.

## Proof ledger

Already proven and retained:

- the 29-package boundary skeleton and architecture checker;
- streaming/keep-alive transport prototype;
- isolated installed-client probes for Codex and Claude Code;
- saved-login native cloud E2Es through the private gate for both clients;
- caller-token, credential-origin, path, header, TLS, redirect, and cancellation gates;
- a separate router contract that chooses Local only for the explicit canary and Cloud otherwise;
- launchd listener adoption and opt-in crash/prebind lifecycle proof;
- pinned llama.cpp supervision with a private loopback bearer, real Qwen output, stop, and port reuse;
- one immutable Qwen artifact record, exact cached-file verification, and a read-only `lao preview`.

Stage 1 exit evidence now also includes the clean installed local/cloud/restart/off acceptance run on the supported Mac.

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

Status: complete (2026-08-30).

Use `model` for one compiled-in artifact record only:

- upstream URL and immutable revision;
- expected byte length and SHA-256;
- license, template, native context, llama.cpp build, and expected working set;
- bounded temporary download followed by hash verification and atomic rename;
- reuse the already verified cached file when it matches.

Acceptance:

- preview shows exact download size, resolved memory budget, context, and artifact identity;
- wrong length or hash is rejected before promotion;
- one verified path is exposed through the model API for S1-03 app composition;
- no signed catalog, recommendation engine, resume system, LRU, or preference database.

### S1-03 — One local protocol slice

Status: complete (2026-08-30).

Wire `gate`, `route`, and `run` in the daemon.

- Keep every normal request on native cloud.
- Add one explicit, non-secret local canary selector.
- Give the router only `Context(client, operation, canary)`; retain the request and every secret in the gate.
- Accept the selector only on a non-empty, length-bounded Responses or Messages request with the `application/json` media type. This is a fixed canary gate, not body parsing or a general local-task classifier.
- Bind Local only to the dynamic loopback endpoint returned by the owned runtime.
- Pass request bodies and response streams through the pinned llama.cpp server's native Responses and Messages HTTP/SSE; add no translation or SSE parser.
- Expose the local artifact as `lao-local`, never as its filesystem path.
- Strip every native credential and provider-specific secret before local egress.
- Reject unsupported local transport shapes before output; do not silently change route mid-task.

The S1-03 proof used `LAO_LOCAL_CANARY=1` plus synthetic per-client caller tokens. S1-04 now owns those internal launch settings and generated caller keys; they are not a user-facing activation interface.

Current evidence:

- direct real probes show pinned llama.cpp 10280 serves valid Responses and Messages SSE at 32K;
- the gate accepts only the exact canary selector, consumes it, and rejects selector/decision mismatch;
- Local egress contains only the runtime bearer and protocol-safe fields;
- normal contexts still resolve to Cloud;
- one real shared-runtime E2E returned exactly `42` through installed Codex 0.151.0 and Claude Code 2.1.251 without persistent config or cloud model use.

Acceptance:

- real installed Codex and Claude Code each complete the same harmless local turn;
- llama.cpp receives no native credential, caller token, or provider-only header;
- ordinary requests still resolve to native cloud and preserve the proven native pass-through;
- cancellation stops local generation and no retry crosses a side-effect boundary.

The canary E2E disables client retries. The existing relay cancellation proof applies unchanged to Local because both routes consume the same frozen Hyper body path. Explicit runtime stop and `Drop` are proven here; the S1-05 restart run also proved bounded parent-death cleanup.

### S1-04 — Transactional `install` and `off`

Status: complete (2026-08-30).

Manage only the exact supported Codex and Claude Code settings.

- Bind and verify the launchd-owned listener before changing a client.
- Generate separate caller tokens for Codex and Claude.
- Never open either harness's auth store.
- Detect conflicting provider/auth configuration and fail without writes.
- Download and verify the pinned local runtime behind `lao install`; require no separately installed runtime package.
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

Current evidence:

- a clean `lao install` downloaded the official runtime archive, verified its exact size, SHA-256, binary build, and Metal visibility, then completed both installed local canaries;
- the Codex adapter performs a structural TOML edit and the Claude adapter preserves unrelated JSON settings;
- the CLI stores byte-exact before/after files with original modes, while its crash record contains paths and phase but no caller key;
- launchd bootstrap must produce a fresh 0600 adoption file and pass the exact inert hello before the first client write;
- one main transaction test proves exact off, permission restoration, lock exclusion, and user-edit refusal;
- one fault test induces failure at each of the two client write boundaries and proves both originals remain exact.

### S1-05 — Real installed acceptance

Status: complete (2026-08-30).

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

Current evidence:

- Codex 0.151.0 and Claude Code 2.1.251 completed fixed saved-login cloud outcomes through the installed gate; no local worker started, proving Cloud remained the default;
- `lao smoke` returned exactly `42` through both real harnesses: Codex cold local took 23.5 seconds and Claude warm local took 1.6 seconds;
- after a forced daemon restart, both local outcomes passed again at 23.5 seconds and 1.6 seconds;
- the restart-run worker peaked at 2,146,768 KiB RSS, about 2.05 GiB, with the verified 32K artifact under the 6 GiB Light ceiling;
- caller tokens, runtime keys, provider credentials, and raw client output were absent from product logs and reported evidence; the daemon error file stayed empty and owner-only;
- no repeated permission prompt was observed; `lao off` restored both original settings byte-for-byte with their original modes and left no daemon, worker, listener, plist, runtime key, or log.

### S1-06 — Adversarial review

Status: complete (2026-08-30).

Review only the Stage 1 trust boundaries:

- credential destination and caller confusion;
- listener ownership before configuration;
- unsupported input reaching local inference;
- memory/pressure escape;
- worker and socket cleanup;
- crash-safe install/off rollback.

Fix blockers. Do not expand the review into deferred products.

Current evidence:

- client callers use separate 256-bit capabilities and constant-time comparison; the gate still strips caller, selector, and native credentials before local egress;
- launchd activation now precedes adoption proof, the installed daemon lives in owner-only product state rather than a protected development folder, and settings change only after the exact hello succeeds;
- unsupported methods, paths, bodies, callers, selectors, authentication classes, redirects, and non-public native destinations fail closed before local or cloud connection;
- fresh fit and measured child RSS both enforce the Light ceiling, while cloud requests leave the runtime unloaded;
- launchd restart and `lao off` reap the worker and listener within the bounded cleanup check;
- induced failures at both settings writes and real readiness failures restored both originals exactly; launch artifacts and owner-only logs are cleaned transactionally.

### S1-07 — Simplify and hand off

Status: complete (2026-08-30).

- Read the manifesto again.
- Remove unused types, options, tests, dependencies, prose, and indirection.
- Keep only evidence that protects a Stage 1 requirement.
- Run formatting, focused and workspace tests, Clippy with warnings denied, `cargo xtask check`, `cargo xtask extract`, and `git diff --check`.
- Update all four living documents together.
- Commit as `YavorGIvanov <yavorgenadiev@gmail.com>` with no other author or coauthor.

Current evidence:

- the final pass removed the unimplemented `doctor` claim and retained only the four exercised CLI operations;
- formatting, all workspace tests, workspace Clippy with warnings denied, the 29-package architecture check, extraction/conformance, and diff hygiene pass;
- README status, the visual architecture map, the product architecture, and this plan describe the same Stage 1 boundary and evidence.

## Stage 1 exit gate

Stage 1 is complete only when a new user on the supported Mac can:

```text
install once → keep using Codex or Claude → use cloud normally → request one local canary → turn LAO off cleanly
```

The result must use saved harness authentication without LAO reading the real token, keep local inference within Light mode, and restore both clients exactly.

## R1 — Safe runtime residency

Status: complete (2026-08-30).

This is the first measured post-Stage 1 slice. The installed local worker used about 2.05 GiB RSS and previously remained resident until daemon shutdown, while the measured cold start was 23.5 seconds. Release memory safely before considering preload or wider local routing.

- Acquire one response-held runtime reference only after a Local route decision.
- Hold the lease through the complete response stream, including cancellation.
- Begin the idle window only after the last concurrent local lease ends.
- Stop the worker at the first five-second check after five observed idle minutes, or as soon as an idle check observes macOS pressure.
- Treat a failed pressure probe as pressure; never interrupt an active local stream.
- Let the next Local request perform the existing fresh fit check and cold start.

Acceptance:

- cloud requests never start or retain the runtime;
- an active or partially streamed local response cannot be evicted;
- idle timeout and memory pressure both select eviction only with zero active leases;
- eviction uses the proven owned stop path, which releases the child, key, and listener;
- no preload, user setting, status surface, new model, or automatic routing is added.

Current evidence:

- the gate fixture proves Cloud acquires no lease, Local acquires exactly one, the lease remains held after response headers, and it releases after the response completes;
- the daemon residency test proves pressure and the five-minute boundary select eviction;
- the existing real runtime lifecycle proof covers owned stop, process cleanup, key removal, and port release;
- focused tests, workspace checks, extraction, Clippy, and diff hygiene pass.

## Deferred backlog

After the exit gate, choose the next measured bottleneck. The long-term contracts and constraints remain in the product architecture.

Deferred product work:

- automatic difficulty routing, task stickiness, repair, escalation, and circuit breakers;
- model catalog signatures, multiple models, preferences, recommendations, and llama-swap;
- remaining runtime residency in this order: background preload only for installs with useful local routing enabled; only then parallel verification and start if measured cold latency still warrants it;
- Ollama, LM Studio, ShoeHorn, FreeToken, NVIDIA, Linux, and Windows;
- hooks and task-boundary tracking;
- consented capture, scrub, snapshots, encrypted vault, retention, export, and deletion;
- personal replay evals, proprietary-model campaigns, reports, and promotion workflow;
- explicit training consent, dataset lineage, adapters, tuning, and rollback;
- background controls, observability, support bundles, updater, packaging, and release hardening;
- product-scale metrics and design-partner rollout.

These components stay independently drafted. They are not prerequisites for proving the core product loop.
