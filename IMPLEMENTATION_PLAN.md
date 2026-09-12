# Implementation plan

## Outcome

Retain the working proof and earn the next claim: useful local work with independently verified outcomes and a measured total cost in time and resources.

```text
Codex / Claude Code → LAO gate → conservative router → llama.cpp or native cloud

Codex / Claude planner → one MCP work packet → semantic router → Cloud or OpenCode → local runtime
```

The user keeps each coding harness and its existing login. Cloud remains the default. Stage 1 proved one explicit canary; R2 permits one narrow text request to route automatically; R4 lets the cloud harness delegate one bounded implementation packet to a real local agent. Each packet is routed independently. Unsupported, risky, or ambiguous work stays Cloud. `lao off` restores the original client configuration even if the daemon is unavailable.

Stage 1 targeted this 24 GiB Apple M4 Mac and is complete. The historical S1/R1–R7 ledger below records that proof, not the final router, catalog, evaluator, or cross-platform release. The next steps are ordered below; a deferred contract is not current functionality.

## Next steps

The next release is a limited Apple Silicon Mac beta with a measured useful task slice. Cloud remains the default; llama.cpp/Qwen3 and the user's native harness stay in place. The existing proof and R8 checker do not establish net benefit. Complete these gates in order; packaging preparation may overlap measurement, but cannot replace it.

| Order | Slice | Owner / files | Exit gate |
|---|---|---|---|
| R8 | Offline paired evidence contract — complete | `xtask`, `docs/benchmarks` | Strict complete pairs and independent verdicts; synthetic example only. |
| R9 | Prove useful local work | `app/cli/tests`, public fixtures, `docs/benchmarks` | Realistic tasks, independent verification, matched native Cloud/hybrid trials; include planning, handoff, review, repair, failures and resource costs. |
| R10 | Admit only task types that earn Local | `svc/route`, worker and parent handoff owners | Report false Local and unnecessary Cloud decisions; held-out task outcomes justify the eligible slice, with safe partial-change return. |
| R11 | Remove installation barriers | CLI/client owners, release artifacts | Signed/notarized prebuilt binaries; Codex-only, Claude-only or both; clean install, upgrade, rollback and removal without source-build prerequisites. |
| R12 | Certify the advertised Mac support range | Client/gate/run/worker owners | Published hardware/OS/client/auth matrix with fresh lifecycle, sandbox, resource and native-path evidence for every advertised combination. |
| Beta / R13 | Small consent-based user pilot | Release owner, `docs/benchmarks` | 5–10 users; count activation, verified outcomes, repairs, resource use and observed cloud usage; retain failures and decide whether to widen release. |
| After first release | Optional engines and platforms | Runtime/CLI and platform owners | One admitted combination at a time behind existing APIs; llama.cpp stays default. Linux/Windows and NVIDIA/AMD are expansion targets, not Mac beta prerequisites. |

**R9a is complete**: the fixture suite and one local-only diagnostic pass. R9b now has an offline-verified corpus and a registered subscription workflow collector. The user authorized the Codex comparison and a separate two-task Claude smoke, with a $5 additional-spend ceiling. The complete cold comparison verified 36/36 native objectives and 22/36 hybrid objectives, with all delegations returning Cloud. Warm/Claude collection is paused at the user’s request. Do not mark R9 or user value proven from fixture checks or local diagnostics. Personal capture/vault/eval/training stay disabled.

### R8 — Evidence before new performance claims

Status: complete (2026-09-06), offline tooling and synthetic evidence only.

The contributor-only `xtask evidence` command validates sanitized paired reports offline; it does not activate the daemon or evaluation services.

Acceptance:

- versioned strict input with declared fixture/verifier hashes, machine identity, cache condition, dispatch-through-verification timer boundary, source/runtime/model/harness configuration, and planned tasks/rounds;
- actual configuration in every arm must equal its declaration; missing/duplicate arms, invalid timing, unknown fields and infrastructure-invalid trials make the comparison unusable;
- synthetic and measured evidence remain visibly distinct; worker completion, verifier success and scope verdict remain independent;
- preserve failed/timed-out worker rows and total elapsed time; display counts, a descriptive success delta and medians, with no automatic model promotion, tail statistics, or inferred money/quota savings;
- bounded local read, sanitized errors and aggregate output; no raw task content, process arguments, credentials, or model traffic;
- one main outcome test and one essential integrity-failure test, focused strict Clippy, formatting and diff hygiene.

This is an internal consistency check, not proof of authentic measurements, correct verdicts, consent, or a secure replay environment. Collection and artifact hashing remain producer responsibilities. Full personal evaluation statistics remain governed by the product architecture.

Current evidence:

- `cargo xtask evidence docs/benchmarks/example.json` accepts the documented synthetic pairs and reports baseline 2/2, candidate 1/2 and a 1,500 ms candidate median. These are invented test values, not a performance result.
- The main case proves that worker completion cannot replace independent verifier/scope success; the integrity case rejects missing pairs and observed-version drift. A real CLI failure check also rejects drift without echoing supplied content.
- Focused integrity tests, strict workspace Clippy, architecture checks and extraction/conformance passed. No installed-client, inference, capture or cloud campaign ran for R8. Historical R1–R7 measurements retain their original configurations.

### R9 — Measure the whole bounded task

Status: R9a complete (2026-09-06); R9b collector and corpus verified offline (2026-09-12). Subscription execution is authorized and registered; the complete cold comparison is reported and its paired export passes the evidence checker. Warm/Claude collection is paused at the user’s request. R9 user-benefit acceptance is still open.

**R9a — Build the task suite.** Add six bounded structured-file edits across three small repository-owned fixture projects, beyond the existing typo canary, plus an essential broad/risky Cloud control. Declare task IDs, objectives, exact paths and independent expected behavior before running the worker. Fresh copies separate trials; verifier expectations stay outside worker access. Prove that every untouched task fails its verifier, its reference edit passes, and unrelated changes fail the scope check. Run the real MCP/local path serially with deadlines, bounded output and cleanup; record every route, worker status, content verdict, scope verdict and dispatch-through-verification time. A Cloud return is a deferral, never a local success. No cloud generation, private capture or router tuning occurs in this diagnostic. Hand-authored fixture packets are not evidence of natural harness behavior or broad coding quality.

R9a evidence:

- [Six task definitions and three input projects](docs/benchmarks/tasks.json) replace the typo-only worker integration fixture. The SHA-256 manifest pins the corpus and independent verifier before execution; no new service, framework, dependency or production behavior was added.
- Offline tests prove every starting fixture fails and its reference passes; a correct allowed edit cannot hide an added file or symlink replacement. Verification never executes model-written code and checks the entire fixture tree with bounded reads.
- The [6 September local diagnostic](docs/benchmarks/local-2026-09-06.md) ran all six packets once: two routed Local and independently passed (49,997 ms and 41,652 ms), four deferred to Cloud, and the separate broad control stayed Cloud unchanged. All scope checks passed. No prompt/router tuning, retries or cloud generation occurred.
- Final verification on 7 September: 87 workspace tests passed (11 opt-in tests skipped), strict workspace Clippy passed, and the final fixture changes passed focused tests/Clippy. Formatting, file hashes, document links and the 33-package architecture guard passed. The manifesto is unchanged.
- The recorded timer is MCP dispatch through independent verification, with uncontrolled cache state. It excludes parent planning/review/repair and is not schema-1 paired evidence. Neither savings nor unnecessary-Cloud error rates can be inferred.

**R9b — Compare the full workflow.** Use these tasks and extend to representative real/public projects with an independent holdout. Both native Cloud and LAO hybrid arms must complete the same objective and verifier. Keep Codex and Claude reports separate. This is the first user-benefit gate; R9a timings alone exclude parent planning/review/repair and cannot satisfy it.

R9b partial evidence (2026-09-12): the [registration](docs/benchmarks/workflow.md) pins twelve narrow JSON tasks, including Vite/Fastify public excerpts and an Express repository holdout. The complete cold Codex comparison contains 36 matched pairs across three repetitions: native Cloud verified 36/36 objectives, hybrid 22/36. All hybrid calls returned Cloud; all parents reported completion. Median primary times were 42.582 and 45.483 seconds respectively, including failed objectives. All cold scope, cleanup and pre/post resource checks passed. The Express holdout verified 6/6 native and 4/6 hybrid objectives. The [cold-only schema-1 export](docs/benchmarks/subscription-2026-09-12-codex-cold.json) passes `cargo xtask evidence`. No Local coverage or user benefit was demonstrated.

Warm collection retained three Codex executions, including a resource-invalid pair. One Claude baseline then completed; its candidate was interrupted when the user requested postponement while using Blender and other programs. Final accounting is 76 finished executions, one interrupted execution and 71 unexecuted rows. All observations and earlier refused checkpoints are retained, with no completed-arm replacement. Six focused collector tests, formatting and diff checks pass; earlier workspace tests, strict Clippy and architecture/extraction checks cover the unchanged Rust implementation. The user's subscription-only scope and $5 additional-spend ceiling remain in force.

R9b remains partial. Finish deferred collection only when requested, retaining invalid/interrupted pairs and the frozen workload; disclose that the cold holdout was reviewed for this authorized partial report. The next measured product issue is unreliable native parent continuation after Cloud handoff, to be reproduced on already-observed tasks before any routing change. Subscription counters do not implement per-request token reservation, encrypted native-traffic inspection or invoice accounting. Public excerpts and repeated objectives do not establish broad coding quality, quota savings or promotion eligibility.

Pre-register before execution:

1. Pin the source and lockfile, actual runtime/model/tokenizer/template, harness binary/version, fixture and verifier. Record a content-free host/OS/resource configuration. Keep Codex and Claude reports separate.
2. Compare the native Cloud baseline with the LAO hybrid path on identical tasks. Alternate arm order, run serially, begin with three equal repetitions, and declare wall-time/token limits and a maximum campaign budget. Cloud evaluation/spend needs its own explicit scope and consent; saved subscription login is not unattended-evaluation authorization.
3. Separate cold and warm cohorts. Define cold as no loaded inference/prefix state; define warm by completed fixed warmup and observed readiness. Fresh worker state is required in both. Check available memory, pressure, power and thermal conditions before/after each arm. Refuse rather than publish unstable comparisons; bound every subprocess and clean up on interruption.
4. Primary time starts at parent dispatch and ends after independent verification, including route time, local failure, review, repair and Cloud continuation if any. Record requested/actual route and backend, worker execution status, independent verifier/scope outcome, and resource observations. Keep generator prefill/decode and gateway microbenchmarks separate.
5. Hash collection artifacts without copying credentials or raw output into reports. Atomically checkpoint planned/completed/refused state, include every trial, and retry infrastructure-invalid pairs symmetrically only inside the approved budget. Failed models and timeouts stay in the denominator.

Acceptance: all planned pairs accounted for, untouched-baseline verifier behavior established, exact edit scope checked independently, cleanup passes, and a report clearly identifies successful/failed attempts and overhead. Worker `complete` alone cannot pass a task; an edited file after worker failure still requires review. If only synthetic/offline checks ran, report exactly that.

Do not calculate p90/p95 or claim general savings from this tiny pilot. Require the product architecture's larger held-out task and uncertainty gates before quality/promotion claims. Actual token/quota/currency savings need measured provider usage and an explicit baseline; latency is not a proxy for spend.

### R10 — Admit only useful task types

Status: conditional on R9 paired evidence; no automatic promotion.

Use R9 outcomes to define a narrow eligible task slice. Report false Local decisions (unsafe/unsupported work or independently unsuccessful local work) separately from unnecessary Cloud decisions on independently demonstrated eligible work. Do not label every Cloud deferral wrong. Use an untouched holdout and matched hardware, budgets and verifier definitions; include failed attempts, repair and parent overhead in the decision.

- Keep risky, broad, ambiguous and unsupported work Cloud. Preserve one-call packet routing, exact paths and parent verification.
- Return sanitized partial-change metadata after failure or timeout; the parent reviews actual changes before continuing. Never automatically replay possible side effects or silently bypass the sandbox.
- Improve the measured blocker only: handoff/review overhead, worker reliability, routing, prefill or decode. Retain resource bounds and fresh worker state. Add no router framework or engine switch without evidence.
- Publish success by task type, Local coverage, route errors, whole-task time and observed cloud usage with denominators. Compare on held-out tasks before widening eligibility; latency is not a proxy for cost.

Exit: the admitted slice offers measured user benefit without material correctness, resource or trust regression. Preserve or narrow the current policy when the result is negative; a canary or synthetic report cannot promote it.

### R11 — Distribute without setup barriers

Status: packaging preparation in progress; release remains gated on useful R9/R10 evidence.

R11a (2026-09-07): `package.sh` builds a self-contained Apple Silicon archive with the CLI, daemon, shared installer, source identity, MIT license and fixed SHA-256 manifest. It checks architecture and rejects dependencies on non-system dynamic libraries. The installer validates the archive before writes and can reuse identical installed content without invoking source tools. `sh test/install.sh` exercises actual archive installation, repeat installation and CLI startup with only stock OS tools on PATH, plus corruption rejection without modifying installed binaries. No client/model activation occurs in this packaging test. This is unsigned contributor preparation; signing/notarization, publication and upgrade/rollback certification remain open.

R11 dependency/model notices (2026-09-12): [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt) audits the 228 normal/build Cargo dependencies of the Apple Silicon CLI/daemon, Rust standard-library/runtime notices, nested native-code licenses, and the pinned separately downloaded Qwen3, MiniLM, llama.cpp and OpenCode artifacts. It retains upstream license and attribution text and includes the unmodified MPL-2.0 option-ext source. Models and external executables are not bundled in this archive. Packaging includes and checksums the notices and refuses a stale lockfile/compiler audit; the installer requires a regular notice file and verifies it before writes. Signing, publication and R9b benchmark/campaign files are outside this change. Verification: `sh package.sh` and `sh test/install.sh` passed on Apple Silicon, including byte-for-byte notice contents, manifest checks, missing/corrupt/symlink notice rejection before writes, repeat installation, CLI startup, single-client discovery and binary-corruption rejection. Separate audit checks verified all 228 license references and stale lockfile/compiler rejection before build. Shell syntax, `cargo fmt --all -- --check` and `git diff --check` passed. No Rust implementation or dependency changes, client/model activation or cloud generation were needed.

Ship signed and notarized prebuilt Apple Silicon CLI/daemon artifacts with a stable identity, integrity verification and dependency/model license notices. Users must not need Rust, Cargo or a source checkout. Keep setup one command and measure download, activation and resource requirements honestly.

R11b implementation (2026-09-07): automatic discovery admits Codex-only, Claude-only or both; `--client codex|claude|both` selects explicitly. Transactions snapshot, validate and restore only selected files; legacy two-client records retain their JSON representation. Missing snapshots stop before writes, and failed recovery retains the record. The daemon emits no caller capability or warming probe for an unselected client; status and smoke follow the persisted selection. Selection changes require `off` first; upgrades retain the current selection.

Admission uses minimum versions (Codex 0.151.0, Claude Code 2.1.251) plus read-only checks for required CLI flags. Newer versions do not need allowlist edits. Old or unrecognized version formats and missing capabilities fail before downloads/settings changes. Help checks are a compatibility signal, not proof of native auth, config semantics or every future version. Keep local smoke evidence separate from the R12 compatibility matrix.

R11b verification:

| Installed client | Isolated selected-client lifecycle | Warm local smoke |
|---|---|---|
| Codex 0.153.4 | Preflight, generated launchd job, warming, disabled-client denial, settings restoration and listener cleanup passed | 4,064 ms |
| Claude Code 2.1.251 | Same path passed with Codex unselected | 2,120 ms |

The E2E uses the daemon extracted from the unsigned archive, real installed harness binaries and one bounded cached Qwen/llama.cpp runtime through the existing external-runtime seam. It runs production transaction/probe code against temporary settings and unique launchd labels; it does not replace the user's active installation. These are local canaries, not user-benefit benchmarks or complete fresh-Mac/upgrade acceptance. One startup was refused by the memory-fit guard; the later serial run passed without changing the guard. Both current clients also passed the existing loopback protocol/credential-separation E2Es with synthetic credentials; no native cloud generation or R9b campaign ran.

Offline evidence includes single-client restore/rollback with unreadable unselected files, legacy two-client record loading, missing-snapshot refusal before writes, invalid-record rejection, explicit selection, CLI capability parsing and minimum-version classification. Workspace tests, strict workspace Clippy, all 33 architecture checks and extraction/conformance passed. The archive installer passes repeat installation, missing-other-client discovery and corruption rejection. Builds use at most two jobs.

Reproduce the local lifecycle after building `lao-daemon`: `cargo test -p lao-cli --lib each_harness_warms_smokes_and_restores_independently --jobs 2 -- --ignored --nocapture`. Set `LAO_TEST_DAEMON` to an extracted archive's daemon to test that artifact. Cached model/runtime, supported CLI capabilities and the public Codex model catalog are prerequisites. The test consumes local resources; it does not read harness credential stores or alter the active service.

R11c binary upgrade preparation (2026-09-12): the archive installer stages both binaries and their source identity before replacing installed files. A private `.install-pending` directory serializes binary installers and retains hard-linked original files until both replacements and command links succeed. Command errors and catchable HUP/INT/TERM interruptions restore prior files and remove newly created command links. Existing non-regular destination files are refused. Failed rollback retains snapshots and blocks another installer from overwriting recovery state.

Verification: `sh package.sh` and `sh test/install.sh` passed with two build jobs. The archive upgrades synthetic prior binaries, preserves unrelated files, and reuses the resulting installation on repeat. Fault injection through a test-only `mv` wrapper exercises failure and TERM after CLI replacement, checks byte-for-byte restoration of both binaries and source identity plus unchanged links/inode, and verifies a clean retry. Failed-rollback and missing-snapshot cases retain recovery state and refuse a subsequent install. A separate fresh-install link-failure check removes newly installed binaries/links and verifies a successful retry. Existing notice, corruption, CLI startup and single-client checks still pass; shell syntax, formatting and diff checks pass. These tests do not activate clients/models or alter the active service. They establish binary installer recovery, not historical-version, service/settings upgrade or fresh-Mac certification. R11c did not yet recover automatically after SIGKILL; R11d below adds process-crash recovery. R9b files are unchanged.

R11d process-crash recovery (2026-09-12): stock macOS `lockf` holds an exclusive lock through an inherited shell descriptor; process death releases it after surviving installer children exit. Before replacement, the installer publishes a complete recovery record with fixed checksums for old/new files, explicit absence markers, link ownership and the original command directory. The next verified installer validates the entire record and every destination before restoring anything. Interrupted recovery is repeatable. Conflicting content, missing snapshots, symlinks and a different command directory are refused with snapshots retained. Renaming the pending directory back to staging commits completed work; staging cleanup cannot roll back that commit. Older incomplete R11c records still require inspection/manual recovery.

Verification: the prebuilt archive passes `sh test/install.sh`, including real SIGKILL after CLI replacement, another SIGKILL during rollback, successful retry, kernel-lock contention, user-replacement conflicts and retry after a transient rollback failure. Separate isolated checks killed preparation, committed cleanup and a fresh installation; retries passed, a committed installation retained its inode, a changed command directory was refused, and legacy incomplete snapshots were preserved. Existing archive/notice/upgrade checks, shell syntax, formatting and diff checks pass. This is process-termination evidence on the test Mac, not physical power-loss or filesystem durability certification. No active service, client/model activation or R9b campaign was touched. R11e below adds removal; full service/settings upgrade, signing/notarization and publication remain open.

R11e removal (2026-09-12): `lao uninstall` shares `off`'s settings restoration and shutdown path, then removes `~/Library/Application Support/lao`, `~/Library/Caches/lao`, the installed binary pair and source receipt, and only command links pointing to that installation. External runtimes, client homes, unrelated binary-directory entries and the source checkout/archive remain. `off` retains installed binaries and cached models/runtimes/router/worker artifacts. Custom binary locations require the same `LAO_PREFIX`/`LAO_BIN_DIR` as installation. Source builds now retain binary ownership checksums even with an unversioned working tree; older installs without receipts require rerunning `install.sh` before removal.

Removal holds the binary and settings installation locks, rejects pending binary recovery, changed binary contents, conflicting settings, overlapping deletion roots and symlinked removal paths, and retains recovery state if shutdown fails. It releases each lock pathname only after finishing that namespace's writes. Missing already-removed binaries are allowed so the archive's `bin/lao uninstall` can finish an interrupted removal; native settings restoration must finish before deleting caches or binaries.

Verification: focused tests cover selected settings restoration, shutdown ordering/failure, conflict preservation, unrelated files and external symlink targets, binary receipts, repeat removal and command parsing. The real prebuilt archive installs and uninstalls in a temporary home, preserves a client-settings fixture, then passes repeat removal and reinstallation with stock OS tools. Service shutdown uses the existing `off` implementation; the new settings test injects shutdown and does not certify a live-service removal matrix. 91 workspace tests passed (12 opt-in tests ignored); strict workspace Clippy, final focused CLI Clippy, all 33 architecture checks, extraction/conformance, formatting, shell syntax and diff checks passed. Builds used at most two jobs. No active installation, installed-client/model tests or R9b files are touched. Full service/settings upgrade, physical power-loss certification, signing/notarization and publication remain open.

Existing supported settings must survive installation and upgrade. Conflicts in owned settings must stop safely and explain recovery without printing secrets.

Acceptance: fresh-machine install, interrupted download/setup, repeat install, version upgrade, failed-upgrade rollback, `off` and full removal all work. Native harness use and unrelated settings survive failures; no orphan process/listener or repeated permission prompts remain. Document what off retains in cache and what full removal deletes. Keep status and troubleshooting content-free; add no telemetry or auto-updater framework merely to pass this milestone.

### R12 — Certify the supported Mac release

Status: planned. R9–R12 must pass before advertising the supported beta; retain research-preview status otherwise.

Publish the minimum-version/capability admission policy alongside an evidence matrix of exact exercised versions. Newer client releases may pass preflight without a repository update; do not label that as full compatibility certification. Start with a small set of Apple Silicon RAM/OS configurations, one or both harnesses, and the authentication modes actually validated. Expand one combination at a time.

For every advertised combination, record source/artifact/client/OS identity and pass:

- clean activation and ordinary native Cloud use, natural bounded delegation, streaming/tools/errors and cancellation;
- existing user settings, single-client operation, update/rollback/off, login/logout and reboot readiness;
- sleep/wake, offline startup and runtime/network failure; unavailable local execution must leave or restore the supported native workflow, without replaying partial edits;
- cold/warm memory fit, pressure during active and idle work, bounded CPU/RAM, cleanup and absence of pressure crashes;
- OS sandbox startup and actual file/link/network denial, credential/origin isolation and partial-failure reporting on each supported macOS version.

Resolve failures before claiming support. Apple's deprecated sandbox interface remains a release risk to test explicitly. Unsupported configurations must leave the native harness usable and explain the support limit without consuming a model request. No cross-platform claim follows from the Mac matrix.

### R13 — Learn from a small Mac beta

After R9–R12, invite 5–10 consenting users without automatic private capture. Record minimal verified outcomes, manual repairs, full elapsed time, resource use and provider-reported cloud usage where available. Count handoff/review overhead against savings; label unavailable quota accounting unknown. Retain failed installs and tasks. The larger value targets in the product vision govern wider rollout, not an invented claim from six fixtures. User outreach and any cloud campaign require their own authorization.

### After release — User-selectable engines

Status: deferred until the llama.cpp proof and first supported release are complete.

Add optional engines behind `api/run`, one admitted hardware/model combination at a time. Let the user explicitly choose a compatible engine; keep llama.cpp as the default. Before exposing a choice, prove hardware and memory fit, exact artifact/model identity, required protocols and tools, authenticated access, effective context, cancellation, cleanup and verified task outcomes. Preserve the native harnesses and existing Cloud path. Do not run multiple resident engines beyond the resource budget.

## Implemented components

Only these components are active in the current proof:

| Component | Responsibility |
|---|---|
| `svc/codex`, `svc/claude` | supported client detection and exact settings transaction |
| `svc/gate` | local caller check, credential isolation, protocol ingress and exact egress |
| `svc/route` | conservative semantic routing for narrow text and bounded packets; explicit canary |
| `svc/run` | Apple fit guard and one owned llama.cpp child |
| `svc/model` | one immutable artifact record and verified local file |
| `svc/optimize` | single-flight background harness warming and non-secret readiness state |
| `svc/opencode` | one pinned local agent worker with exact tools and a macOS process sandbox |
| `app/daemon` | compose the request path and adopt the launchd listener |
| `app/cli` | install, preview, status, MCP worker, smoke, and `off` |

The matching `api/*` packages remain the semantic boundaries. Services never import sibling implementations; applications wire them.

`capture`, `vault`, `eval`, `train`, their workers, and generic local RPC remain disabled drafts. Additional backends and personal evidence services follow the release gates above.

## Proof ledger

Historical Stage 1 baseline (2026-08-30); later routing and installation changes are recorded separately:

- the package-boundary skeleton and architecture checker;
- streaming/keep-alive transport prototype;
- isolated installed-client probes for Codex and Claude Code;
- saved-login native cloud E2Es through the private gate for both clients;
- caller-token, credential-origin, path, header, TLS, redirect, and cancellation gates;
- a separate router contract that chooses Local only for the explicit canary and Cloud otherwise;
- launchd listener adoption and opt-in crash/prebind lifecycle proof;
- pinned llama.cpp supervision with a private loopback bearer, real Qwen output, stop, and port reuse;
- one immutable Qwen artifact record, exact cached-file verification, and a read-only `lao preview`.

The clean installed local/cloud/restart/off acceptance run completed this baseline on the test Mac.

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

The final pass removed the unimplemented `doctor` claim. `lao status` exposed service and per-client readiness without configuration values, capabilities or credentials. Workspace tests, strict Clippy, architecture checks, extraction/conformance and formatting passed.

## Stage 1 exit gate

The completed Stage 1 acceptance path on the test Mac was:

```text
install once → keep using Codex or Claude → use cloud normally → request one local canary → turn LAO off cleanly
```

It used saved harness authentication without LAO reading the real token, kept local inference within Light mode, and restored both clients exactly. Fresh-Mac release certification remains R12 work.

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
- replace continuation evidence with two independent turns through one worker object, state cleanup on success and failure, stale MCP session rejection, and exact-path rejection.

Retained deliberately: disabled strategic stubs are already tiny; runtime leases, pressure checks, install rollback, credential isolation, and pinned support-tree checks protect exercised requirements. Removing them would reduce safety or erase useful architectural seams. No new framework, dependency, backend, or automation was added.

Evidence and limits:

- focused tests passed before and after editing; 81 workspace tests passed with 11 opt-in tests skipped, strict workspace Clippy passed, all 33 packages passed the architecture guard, and extraction/conformance, formatting, and diff hygiene passed;
- the existing installed worker fixture passed in 18.89 seconds using the changed source: the broad control stayed Cloud, the Local packet changed only `word.txt`, and the independent verifier passed; historical Codex/Claude natural-handoff timings remain historical;
- a delegated README edit in this review changed only its allowed file but returned `agent_failed`; independent diff review retained the correct edit. This is not a new successful worker benchmark;
- at R6, OpenCode tool permissions were not an OS sandbox; R7 adds the supported-Mac boundary. Reported changed paths still cover the allowlist only, and parent verification remains required;
- no net cloud-cost or quota reduction, broad task success rate, or untested model/harness compatibility is claimed.

## R7 — Worker OS boundary and terminal completion

Status: complete (2026-09-06).

The next blockers were trust in tool-level permissions alone and treating any valid JSON output as completion. This slice adds an OS boundary and makes execution status reflect the pinned worker protocol. It preserves routing, model selection, resource limits, native harness settings, and the deferred product boundaries.

Changes:

- wrap OpenCode and its descendants in macOS `sandbox-exec`; permit exact packet file contents, disposable state, verified support files, standard system resources/IPC, and TCP to the gate port;
- retain filesystem metadata and top-level repository listing for startup, while denying Git metadata and unlisted file contents; pass filesystem paths as sandbox parameters;
- use fresh writable XDG configuration state and keep the verified support tree read-only; express OpenCode permissions relative to `/` because hidden Git metadata makes that its worktree;
- reject preexisting hardlinks and special files as well as ambiguous repository-root permission patterns;
- require terminal stop, successful exit, bounded valid events, no reported tool/session errors, and an observed allowed-file change before returning `complete`;
- fix a parallel CLI fixture collision by adding a per-process sequence to temporary names.

Evidence and limits:

- direct subprocess probes exercise actual OS denial of unlisted/outside/Git contents, support-tree writes, symlink and hardlink escapes, and another loopback port; allowed-file writes, private state, quoted path parameters, and gate access succeed;
- 83 workspace tests passed with 11 opt-in tests skipped; focused worker/CLI checks, strict workspace Clippy, formatting, all 33 package architecture checks, and extraction/conformance passed;
- the final installed worker fixture passed in 18.71 seconds using the changed source and real local OpenCode/Qwen: broad work stayed Cloud, only `word.txt` changed, and the independent verifier passed. No cloud model request or new natural-harness benchmark was involved;
- no unsandboxed fallback exists. Apple's deprecated tool and private system profile limit this evidence to the tested Mac; portable release hardening remains deferred;
- metadata and top-level filenames are visible; this is a content boundary, not concealment of all host information. Existing directories are required for new files. Another unsandboxed same-user process racing filesystem changes is outside this proof;
- failed workers may leave partial edits. Parent review and verification remain mandatory; no automatic retry, cloud spend, new backend, capture, or training was added.

## Deferred backlog

The R8–R13 release gates above replace an unordered post-proof backlog. These remaining candidates have no implementation authorization from their presence here; select them only when the preceding evidence or a specific user task requires them. The long-term contracts and constraints remain in the product architecture.

Deferred product work:

- broad difficulty certification, cross-task stickiness and automatic repair/escalation beyond the R10 bounded slice;
- model catalog signatures, multiple models, preferences, recommendations, and llama-swap;
- broader cache policy for multiple models, machines, battery states, and thermal conditions;
- Ollama, LM Studio, ShoeHorn, FreeToken, NVIDIA, Linux, and Windows;
- hooks and task-boundary tracking;
- consented capture, scrub, snapshots, encrypted vault, retention, export, and deletion;
- personal replay evals, proprietary-model campaigns, reports, and promotion workflow;
- explicit training consent, dataset lineage, adapters, tuning, and rollback;
- richer background controls, telemetry, support bundles and automatic updating beyond the required R11/R12 release work;
- product-scale rollout beyond the R13 consent-based beta.

These components stay independently drafted. They are not prerequisites for proving the core product loop.
