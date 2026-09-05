# Implementation plan

## Outcome

Ship the smallest real proof of this product:

```text
Codex + Claude Code → LAO gate → conservative router → llama.cpp or native cloud

Codex / Claude planner → one MCP work packet → semantic router → Cloud or OpenCode → local runtime
```

The user keeps each coding harness and its existing login. Cloud remains the default. Stage 1 proved one explicit canary; R2 permits one narrow text request to route automatically; R4 lets the cloud harness delegate one bounded implementation packet to a real local agent. Each packet is routed independently. Unsupported, risky, or ambiguous work stays Cloud. `lao off` restores the original client configuration even if the daemon is unavailable.

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

## Implemented components

Only these components are active in the current proof:

| Component | Responsibility |
|---|---|
| `svc/codex`, `svc/claude` | supported client detection and exact settings transaction |
| `svc/gate` | local caller check, credential isolation, protocol ingress and exact egress |
| `svc/route` | cloud-safe decision; one explicit local canary |
| `svc/run` | Apple fit guard and one owned llama.cpp child |
| `svc/model` | one immutable artifact record and verified local file |
| `svc/optimize` | single-flight background harness warming and non-secret readiness state |
| `svc/opencode` | one pinned, permission-bounded local agent worker |
| `app/daemon` | compose the request path and adopt the launchd listener |
| `app/cli` | install, preview, status, MCP worker, smoke, and `off` |

The matching `api/*` packages remain the semantic boundaries. Services never import sibling implementations; applications wire them.

`capture`, `vault`, `eval`, `train`, their workers, generic local RPC, and future backends remain untouched disabled drafts. Do not finalize or expand them in Stage 1.

## Proof ledger

Already proven and retained:

- the package-boundary skeleton and architecture checker;
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

Stage 1 uses Light. The current verified Qwen3 lifecycle fixture uses a 16K context. Current measured 24 GiB ceilings at 72% OS availability are approximately 6.0 GiB Light, 9.28 GiB Auto, and 11.28 GiB Maximum; these values change with live pressure.

Acceptance:

- budget is resolved before load and pressure denies a new load;
- the real 16K fixture stays below the 6 GiB Light ceiling;
- llama.cpp reports an effective 16K context rather than silently shrinking it;
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

- direct real probes show pinned llama.cpp 10280 serves valid Responses and Messages SSE at the configured context;
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
- a repeated `lao install` verifies and reuses a healthy installation without downloading, rotating caller keys, or rewriting client settings;
- the Codex adapter performs a structural TOML edit and the Claude adapter preserves unrelated JSON settings;
- the CLI stores byte-exact before/after files with original modes, accepts unrelated settings that a running client adds, and removes only LAO-owned fields when those additions must be preserved; its crash record contains paths and phase but no caller key;
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
- the historical Stage 1 restart-run worker peaked at 2,146,768 KiB RSS, about 2.05 GiB, with its then-current 32K artifact under the 6 GiB Light ceiling;
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

- the final pass removed the unimplemented `doctor` claim and retained only exercised CLI operations;
- `lao status` reports service and per-client readiness without exposing configuration values, caller capabilities, or credentials;
- formatting, all workspace tests, workspace Clippy with warnings denied, the architecture check, extraction/conformance, and diff hygiene pass;
- README status, the visual architecture map, the product architecture, and this plan describe the same Stage 1 boundary and evidence.

## Stage 1 exit gate

Stage 1 is complete only when a new user on the supported Mac can:

```text
install once → keep using Codex or Claude → use cloud normally → request one local canary → turn LAO off cleanly
```

The result must use saved harness authentication without LAO reading the real token, keep local inference within Light mode, and restore both clients exactly.

## R1 — Safe runtime residency

Status: complete (2026-08-30).

This was the first measured post-Stage 1 slice. The then-current 1.5B worker used about 2.05 GiB RSS and previously remained resident until daemon shutdown, while the measured cold start was 23.5 seconds. R2 keeps its response lease and pressure-safety behavior but supersedes the idle timeout so useful warmed state remains resident on a healthy machine.

- Acquire one response-held runtime reference only after a Local route decision.
- Hold the lease through the complete response stream, including cancellation.
- Begin the idle window only after the last concurrent local lease ends.
- Stop the worker as soon as a five-second idle check observes macOS pressure.
- Treat a failed pressure probe as pressure; never interrupt an active local stream.
- Let the next Local request perform the existing fresh fit check and cold start.

Acceptance:

- cloud requests never start or retain the runtime;
- an active or partially streamed local response cannot be evicted;
- memory pressure selects eviction only with zero active leases;
- eviction uses the proven owned stop path, which releases the child, key, and listener;
- no user setting, new model, or automatic routing is added.

Current evidence:

- the gate fixture proves Cloud acquires no lease, Local acquires exactly one, the lease remains held after response headers, and it releases after the response completes;
- the daemon residency test proves pressure selects eviction only when no response lease is active;
- the existing real runtime lifecycle proof covers owned stop, process cleanup, key removal, and port release;
- focused tests, workspace checks, extraction, Clippy, and diff hygiene pass.

## R2 — Real automatic route

Status: complete (2026-08-30).

This is the smallest automatic path that exercises an independent real classifier and real inference without adding another daemon.

- Buffer only a validated, length-bounded Responses or Messages JSON body after caller authentication.
- Extract only the current user text for the router; never give it headers, targets, or provider credentials.
- Use vLLM Semantic Router's pinned Candle engine with a separate 22.7M-parameter MiniLM model and LAO's conservative easy/hard prototype policy. Any classifier error or unsupported body selects Cloud.
- After Local is final, build a tool-free Local body from only the final user text and model name `lao-local`; Cloud retains the exact original body.
- Keep the Stage 1 canary and the deterministic `safe` router available.
- Add a bounded `/api/v1/eval` adapter for user-managed vLLM Semantic Router.
- Keep inference behind `api/run::Local`: verified direct llama.cpp is the default and a protected user-managed IPv4-loopback endpoint can supply the same protocol. vLLM and SGLang are examples, not special cases in the API.

Acceptance:

- one eligible no-canary spelling fixture is classified Local and completes through real llama.cpp in real Codex and Claude Code processes;
- a tool-free `lao-local` body is created only after the Local decision;
- router failure and ambiguity remain Cloud;
- no native or caller credential reaches either classifier or local inference;
- direct llama.cpp remains the one-command default; external routers and engines require explicit configuration.

Current evidence:

- a clean default install adopted the launchd listener, then the eligible no-canary spelling fixture returned exactly `the` through both installed harness configurations; Codex took 3.79 seconds from cold semantic/runtime state and Claude took 3.85 seconds warm;
- the forced-Cloud installed Codex proof took 2.67 seconds with llama.cpp absent; daemon RSS measured about 8 MiB before MiniLM and 141 MiB after it, while llama.cpp measured about 2.02 GiB;
- the ignored E2E uses real installed Codex and Claude binaries with temporary endpoint settings, a self-bound gate, pinned MiniLM, pinned Qwen, and real llama.cpp; it records two Local decisions and gets exactly `the` twice without a canary;
- fixture tests prove the vLLM Semantic Router adapter is IPv4-loopback-only, deadline- and length-bounded, accepts exact `lao-local` or `lao-cloud`, handles normal and chunked JSON, and fails Cloud;
- unit tests prove the external runtime rejects non-loopback and invalid bearer configuration and never takes ownership of the user-managed engine; no vLLM or SGLang inference E2E is claimed.

## R3 — Background latency optimizer

Status: complete (2026-08-30).

Keep optimization outside the request components and move reusable cold work off the user's critical path.

- Give latency optimization its own API, implementation, private state, and package ownership.
- Run fixed loopback-only Claude and Codex warm canaries after daemon startup without blocking install or normal cloud work.
- Keep caller capabilities out of arguments, environment dumps, logs, and output; retain no raw harness output.
- Enforce one warm plan at a time and expose only `idle`, `warming`, `ready`, or `failed` through `lao status`.
- Retain both harness prompt prefixes in bounded RAM while the machine is healthy; pressure eviction still wins.
- Reuse integrity-checked binaries for the exact same clean source revision.
- Do not widen the R2 routing policy; the fixed warm canaries bypass semantic classification.

Acceptance:

- install returns while warming continues and Codex remains immediately usable on cloud;
- repeated install, status, and same-revision source setup complete below 100 ms on the test Mac;
- gateway p95 overhead remains below 20 ms;
- warmed real Codex and Claude canaries complete without a cold prefill;
- off and recovery stop the process tree and remove optimizer state;
- optimization imports no sibling service implementation and adds no policy to gate, route, or run.

Current evidence:

- direct pinned runtime inference measured 0.67–2.16 seconds while a cold Codex harness request spent about 46 seconds prefilling 10,550 uncached input tokens, identifying harness prefix preparation rather than the gateway as the dominant wait;
- a 384 MiB llama.cpp prompt cache retained both harness prefixes; alternating warmed Codex and Claude runs completed in 1.55–2.27 seconds with about 2.31 GiB peak RSS under the 6 GiB Light ceiling;
- the final installed smoke completed Codex in 1.43 seconds and Claude in 0.95 seconds after background warming;
- five paired gateway benchmarks measured 197–335 microsecond median overhead and at most 3.48 ms p95, so no gateway change was justified;
- repeated `lao install` measured 6.2 ms, `lao status` 5.6 ms, and integrity-checked same-revision source setup 52.5 ms;
- the optimizer owns bounded probes, loopback pinning, single-flight state, 0600 atomic readiness state, failure isolation, and retry; the applications only compose or inspect it through its API;
- focused tests, strict Clippy, and the 31-package architecture check pass.

## R4 — Routed OpenCode worker

Status: complete (2026-09-02).

This is the smallest real cloud-planner/local-worker path. Codex and Claude remain the user-facing harnesses. Their planner may call one MCP tool with a bounded objective and exact relative file paths. LAO routes that packet once; a Cloud result returns it to the current harness, while a Local result runs one pinned OpenCode agent loop entirely against local inference.

- Add `api/agent` as the worker contract and pin OpenCode 1.18.25 behind `svc/opencode`.
- Give OpenCode exact read/edit permissions for the named files. Deny Git metadata, shell, general network, search, subagents, and unlisted paths.
- Keep OpenCode's configuration isolated from user plugins and credentials. Bound and verify its pinned support tree before use. Keep runtime credentials ephemeral and local-only.
- Use the existing semantic router with worker-specific examples. Broad planning stays Cloud; the proven narrow one-file correction selects Local.
- Put OpenCode's Chat Completions traffic on a separate authenticated local gate path. It cannot fall back to a provider.
- Keep the runtime API interchangeable: verified llama.cpp/Qwen3 is the default; a user may select a protected external endpoint. vLLM and SGLang remain uncertified examples.
- Pre-approve only `lao.execute` in the managed Codex and Claude settings. Do not broaden shell, network, or harness permissions.

Acceptance:

- a real Codex cloud turn invokes `lao.execute` once without an approval prompt;
- the default semantic router selects Local for the bounded packet;
- real OpenCode and Qwen3 change only the permitted file;
- the parent Codex process independently verifies the change;
- a broad packet returns Cloud without starting OpenCode;
- installer rollback restores the original files; `lao off` restores unchanged managed files exactly and removes only LAO's entry from mutable Claude global state.

Current evidence:

- the installed Codex 0.151.0 E2E called `lao.execute` exactly once, changed `word.txt` from `teh` to `the`, and passed `verify.sh` in about 20 seconds warm without a permission prompt;
- the direct ignored MCP E2E proves the same Local result plus a Cloud control case and asserts the exact repository change set;
- the current Qwen3 4B Q4_K_M worker uses a 16K context and measured about 4.70 GiB RSS under the 6 GiB Light ceiling;
- focused tests cover routing, gate isolation, Git-metadata rejection, parent-death worker cleanup, merge-aware Claude MCP removal, exact permissions, install restoration, and MCP results. R6 subsequently removes session reuse; autonomous task splitting and production routing certification remain deferred. Real natural planner handoffs for both clients are recorded in R5.

## R5 — Natural harness handoff

Status: complete (2026-09-02).

Make the bounded worker part of normal Codex and Claude Code use instead of requiring the user to name LAO.

- Give both harnesses one explicit MCP tool contract for the eligible slice, exact repository-relative paths, Cloud handoff, and parent verification.
- Install one short supported Codex developer instruction that defines the eligible slice, one-call boundary, exact repository-relative paths, Cloud handoff, and parent verification.
- Refuse to overwrite an existing developer instruction and remove only the exact LAO-managed value during restoration.
- Warm the same Codex instruction prefix used during normal work.
- Use Claude Code's shared MCP contract without creating or changing a user instruction file.
- Keep broad, ambiguous, sensitive, and multi-area work in the cloud harness.

Acceptance and evidence:

- a fresh Codex 0.151.0 process received only a normal spelling-fix request, called `lao.execute` once with `word.txt`, and did not need an approval prompt;
- a fresh Claude Code 2.1.251 process received the same normal request and called `mcp__lao__execute` once without an approval prompt;
- for both clients, MiniLM selected Local, OpenCode/Qwen changed only `word.txt`, and the parent process reviewed the result and passed `verify.sh`;
- broad authentication-planning controls stayed in their cloud harnesses and did not call the worker;
- aligning the warm prefix reduced the post-warm Codex smoke from 25.4 seconds to 3.9 seconds; Claude completed in 2.1 seconds;
- focused configuration tests prove existing developer instructions conflict safely and uninstall removes LAO's instruction while preserving unrelated edits.

This proves both tested client paths, not universal planner behavior across future models or versions.

## R6 — Independent packets and adversarial simplification

Status: complete (2026-09-05).

The first-principles review keeps the product loop and removes state that the proof does not need. Each packet declares its own objective and file boundary. Resuming a prior worker session carried earlier file contents and instructions into a later packet, even if its allowed files changed. No acceptance evidence required that behavior.

Changes:

- remove the session input/output fields, resume flag, session-ID validation, and recursive output search;
- create owner-only temporary worker state per call, cleaned on normal success, failure, or timeout; retain binary/config verification and the serial turn guard;
- reject wildcard paths and backslashes: filenames must not expand into OpenCode permission patterns or be silently rewritten;
- replace continuation evidence with two independent turns through one worker object, state cleanup on success and failure, stale MCP session rejection, and exact-path rejection;
- rebuild the human architecture map around the two current flows, process/state ownership, failure behavior, and evidence limits;
- align agent instructions with OpenAI's Astra guidance while retaining the manifesto, consent rules, API boundaries, cloud default, and proportional testing.

Retained deliberately: disabled strategic stubs are already tiny; runtime leases, pressure checks, install rollback, credential isolation, and pinned support-tree checks protect exercised requirements. Removing them would reduce safety or erase useful architectural seams. No new framework, dependency, backend, or automation was added.

Evidence and limits:

- focused tests passed before and after editing; 81 workspace tests passed with 11 opt-in tests skipped, strict workspace Clippy passed, all 33 packages passed the architecture guard, and extraction/conformance, formatting, and diff hygiene passed;
- the existing installed worker fixture passed in 18.89 seconds using the changed source: the broad control stayed Cloud, the Local packet changed only `word.txt`, and the independent verifier passed; historical Codex/Claude natural-handoff timings remain historical;
- architecture link targets and section IDs passed validation; desktop and mobile renders were visually reviewed;
- a delegated README edit in this review changed only its allowed file but returned `agent_failed`; independent diff review retained the correct edit. This is not a new successful worker benchmark;
- OpenCode tool permissions are not an OS sandbox, and reported changed paths cover the allowlist only. Parent verification remains required;
- no net cloud-cost or quota reduction, broad task success rate, or Astra gateway compatibility is claimed.

## Deferred backlog

After the exit gate, choose the next measured bottleneck. The long-term contracts and constraints remain in the product architecture.

Deferred product work:

- certified independent difficulty routing, task stickiness, repair, escalation, and circuit breakers;
- model catalog signatures, multiple models, preferences, recommendations, and llama-swap;
- broader cache policy for multiple models, machines, battery states, and thermal conditions;
- Ollama, LM Studio, ShoeHorn, FreeToken, NVIDIA, Linux, and Windows;
- hooks and task-boundary tracking;
- consented capture, scrub, snapshots, encrypted vault, retention, export, and deletion;
- personal replay evals, proprietary-model campaigns, reports, and promotion workflow;
- explicit training consent, dataset lineage, adapters, tuning, and rollback;
- background controls, observability, support bundles, updater, packaging, and release hardening;
- product-scale metrics and design-partner rollout.

These components stay independently drafted. They are not prerequisites for proving the core product loop.
