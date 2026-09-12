# R9b subscription workflow registration

Status (2026-09-12): the user authorized end-to-end subscription trials and a report, with a $5 ceiling on additional spend. Codex is the primary comparison; two Claude Code tasks form a separate smoke comparison. The [collector](collect.py) and independent fixture oracle are implemented. Completion and limitations belong to the resulting report, not this registration.

## Amendment before the completed campaign

The first attempt at revision `cc3bb100` was interrupted after ten completed arms and one dispatched arm, before new upstream or holdout tasks ran. Its [JSON](subscription-2026-09-12-attempt1.json) and [report](subscription-2026-09-12-attempt1.md) remain unchanged: five baselines passed, five hybrid Cloud deferrals left the file unchanged. The interrupted row lacks a final verdict and primary timing; it must not enter complete-pair estimates.

Fresh diagnostics used only the already-observed `service-timeout-units` task. Native event classifications showed reads and a Cloud return, then no editing operation; the parent described a sandbox block. A real native sandbox probe allowed the exact file write and refused guard/outside access. An equivalent workspace-relative permission profile did not fix the behavior. A factual permission/handoff clarification allowed a fresh hybrid execution to pass the independent verifier, with one failed parent tool retained in the diagnostic count. The precise client/model cause is not established; this is a collector instruction correction, not evidence of a worker sandbox failure.

Both Codex arms now append this identical clarification to the common instruction, substituting the disposable absolute file path:

> The campaign sandbox grants read access to this workspace and write access to FILE. A Cloud tool result is a routing decision, not a sandbox refusal; complete the edit using this harness within that exact file permission.

The original exact filesystem permissions, model, objectives, oracle, router, worker and schedule are unchanged. The collector now retains interrupted parent timing/verdict before stopping, counts failed tools separately from configuration notices, and whitelists numeric token counters. Three additional native diagnostics during this correction (two unsuccessful, one successful) are excluded from task estimates; earlier preparatory diagnostics were not fully metered, so whole-session diagnostic usage is unknown. No holdout task was used in diagnosis.

The user's instruction to continue end to end covers this corrected run within the same subscription and $5 additional-spend bounds. The new run starts from a new clean source revision and is reported separately, without pooling the original ten completed arms or substituting successful retries into the old attempt. There are no within-run replacement pairs and no further prompt/router tuning. Preserve any new failures as outcomes.

## Resource-pause continuation

The corrected run at product revision `7e59d94` stopped after 69 completed executions, before dispatching row 70, with `resource_guard_refused`. The [original checkpoint](subscription-2026-09-12.json) and [refused report](subscription-2026-09-12.md) are retained unchanged. A subsequent content-free health check observed elevated memory pressure (level 2); the campaign helpers had exited. No holdout outcomes were reviewed during this pause.

The collector now supports `--resume ORIGINAL --output NEW` solely for a pre-dispatch resource refusal with an untouched planned suffix, within the original registration expiry. It refuses dispatched rows, changed schedule order, noncontiguous completed rows, failed cleanup/scope/resource checks, and changed product binaries, oracle, local artifacts, clients, lockfile or host. It keeps completed rows unchanged, including unsuccessful task outcomes; no arm is retried or replaced. The same pair power-source check applies across the pause.

This orchestration-only correction does not alter model instructions, tool permissions, tasks, verifier, runtime settings or routing. The new report records the original report hash and each continuation's collector revision, hashes, resources and starting index. Its original metadata continues to identify the pinned product binaries; collector revisions are separate provenance. Each continuation requires normal resource readings and a clean source revision. Preserve every refused checkpoint. A completed report must disclose the pause and the split collector provenance; do not describe it as one uninterrupted run.

The resource continuation completed all 72 cold arms, then warm setup failed before any warm task dispatch. A process-group termination `PermissionError` masked the original setup error and prevented terminal checkpoint finalization. The [abandoned checkpoint](subscription-2026-09-12-continued.json) remains unchanged; a separate [recovered refusal](subscription-2026-09-12-setup-refused.json) records exactly 72 unchanged completed rows, 76 untouched planned rows, zero remaining campaign helpers/workspaces, and the unknown original setup cause. A fresh one-token warmup diagnostic passed startup, warmup, stop and cleanup without running a task.

The fixture oracle now receives EOF and a bounded graceful exit before forced termination. Terminal report writing occurs even if final oracle cleanup fails. Resume also accepts this explicitly reviewed pre-dispatch setup refusal, with the same untouched-suffix, schedule and artifact checks. It does not accept an arbitrary abandoned planned report or any previously dispatched pending row. These changes affect setup/error accounting only; native task instructions, permissions and execution functions remain unchanged. The recovered refusal and both prior checkpoints stay separate from the final continued report.

A second warm preparation attempt also stopped before dispatch and is retained in [its refusal report](subscription-2026-09-12-final.json). Its last setup metadata showed 55% available memory. The pinned model declares a 5 GiB working set; production Light mode reserves max(8 GiB, 35% of RAM), so this 24 GiB host needs at least 56% available memory before loading. The earlier successful warm probe ran at 58%. The collector now checks that same allowance before warm setup, while leaving the production fit guard authoritative. It also reaps an exited oracle and preserves its setup failure instead of masking it with broken-pipe cleanup. No warm task was dispatched in either failed setup attempt, and no task arm was retried. Subsequent collection resumes the same 72-row completed prefix and 76-row untouched suffix; the extra setup refusal remains separate evidence.

## Authorized partial finish

After the postflight resource refusal, the user explicitly authorized proceeding with partial results and reported closing apps. The cold Codex cohort already contains all 72 scheduled executions. Close the unfinished Codex warm cohort, retaining its three completed rows and infrastructure-invalid pair without replacement. Run only the four untouched Claude warm smoke executions, under the same model, task, permission, subscription and resource settings. No completed task is replayed. The remaining 69 Codex warm rows stay recorded as planned but closed from further execution.

`--finish-partial` requires a refused checkpoint, a completed cold cohort, an untouched Claude smoke, unchanged execution artifacts and intact completed-row scope/cleanup. It records invalid resource pairs separately, preserves the original report, and finishes with status `partial`. A cold-only paired export may include all 36 stable Codex pairs; the warm subset cannot support the registered full comparison. Publish this final partial report and the separate Claude smoke without routing promotion or inferred savings. If the smoke itself is refused, retain that refusal and finish the report with the evidence available. This amendment closes collection before holdout analysis; do not tune the workload after reviewing the closed campaign.

## User-requested pause and partial analysis

The user subsequently instructed postponement of the remaining runs while using Blender and other applications. Stop dispatch and preserve the active execution. One Claude baseline finished; the candidate was interrupted, independently checked and cleaned. The resulting checkpoint contains 76 finished executions, one interrupted execution and 71 unexecuted rows. The later pause supersedes the earlier warm-cohort closure: remaining work is deferred for possible later continuation, without silent retries.

The user authorized finishing the report from existing evidence. Analyze the complete stable cold Codex cohort, including its Express holdout, now; report the warm and Claude subsets separately as incomplete. A later continuation must keep the frozen workload and disclose this interim holdout review. Resource-invalid and interrupted pairs remain ineligible for valid paired estimates. The original checkpoints and their metadata are unchanged.

## Corpus and provenance

Retain the six authored tasks in [tasks.json](tasks.json), and add the six manifest edits in [workflow-tasks.json](workflow-tasks.json). The latter use byte-for-byte public upstream files at immutable commits:

| Project | Revision | Input | License |
|---|---|---|---|
| Vite 7.1.3 | `e090b7d1e55f59722f5a312067242e96bb8d8994` | [Vanilla starter manifest](https://github.com/vitejs/vite/blob/e090b7d1e55f59722f5a312067242e96bb8d8994/packages/create-vite/template-vanilla/package.json) → [local copy](fixtures/vite/package.json) | [MIT notice](fixtures/vite/LICENSE) |
| Fastify 5.5.0 | `b84733e997340076240d93db5bbeef716df58756` | [Package manifest](https://github.com/fastify/fastify/blob/b84733e997340076240d93db5bbeef716df58756/package.json) → [local copy](fixtures/fastify/package.json) | [MIT notice](fixtures/fastify/LICENSE) |
| Express 5.1.0 | `cd7d4397c398a3f3ecadeaf9ef6ac1377bd414c4` | [Package manifest](https://github.com/expressjs/express/blob/cd7d4397c398a3f3ecadeaf9ef6ac1377bd414c4/package.json) → [local copy](fixtures/express/package.json) | [MIT notice](fixtures/express/LICENSE) |

These are proposed maintenance edits to public project excerpts, not upstream bug reports or full repository builds. Do not install dependencies, execute their scripts, or claim the applications were tested. Public author/contact metadata remains in the original manifests; disclose it in the campaign payload preview. License notices are retained outside the task workspace.

| Cohort | Task IDs | Use |
|---|---|---|
| Previously observed | `web-dev-port`, `web-node-version`, `service-timeout-units`, `service-retry-limit`, `catalog-numeric-price`, `catalog-unique-tags` | Keep R9a observations separate from new full-workflow results. |
| New development | `vite-dev-strict-port`, `vite-preview-port`, `fastify-ci-lint`, `fastify-markdown-lint` | Public manifest maintenance. No router/prompt tuning during this campaign. |
| Repository holdout | `express-node-floor`, `express-coverage-lcov` | Freeze before any model execution; inspect outcomes only after all scheduled trials. Never use the local diagnostic to try these tasks first. |

Express is a repository holdout, not an unseen task-family or pretraining holdout. All tasks are narrow JSON edits. Public visibility and only two held-out tasks limit any generalization; this corpus cannot establish broad coding quality or meet the later promotion sample gate.

Each task uses a fresh copy containing only its declared input at the task's `file` path, the fixed README guard from the verifier, and freshly initialized Git metadata. Only the declared input is writable. The parent receives the exact `objective`, file scope and common workflow instruction below. Never expose `pointer`, `value`, the full task manifests, reference edits, competing results or verifier implementation to a generating harness/worker. Construct the request outside the disposable workspace; do not mount this checkout into it.

The same independent [verifier](../../app/cli/tests/support/tasks.rs) establishes that all 12 starting inputs fail and reference edits pass. It compares the whole parsed document and checks the entire workspace scope. No generated code executes. The original opt-in diagnostic still selects only the six R9a tasks; adding these fixtures does not run or authorize a new diagnostic.

## Matched workflow

Codex 0.154.0 uses the user's configured `gpt-6-astra` model with medium effort and a declared 32,768-token context. Claude Code 2.1.251 uses `claude-opus-5` with high effort, resolved from the user's configured Opus alias during connection preflight. Both clients reported subscription authentication. Baseline and hybrid retain the same model and settings within each harness. Claude runs only `web-dev-port` and `service-timeout-units`, one pair each in the warm cohort: four executions, reported separately from Codex's 144.

The user's explicit subscription authorization supersedes the earlier draft's request for an API-priced campaign. [Official Codex documentation](https://developers.openai.com/codex/noninteractive/) documents non-interactive use of saved CLI authentication; [pricing](https://developers.openai.com/codex/pricing/) distinguishes included usage limits, purchased credits and API billing. No provider credential store is read or copied by the collector. This is not a provider-policy certification for retention or training; raw trajectories and generated edits are discarded.

Common instruction, followed verbatim by the task's objective and declared file:

> Complete the objective within the declared writable file. Review the resulting diff and check that all other settings are preserved. Stop when the objective is satisfied. Do not install dependencies, run project scripts, access the network, or edit any other file.

Native baseline has no LAO gateway or MCP tool enabled. Hybrid additionally has the pinned LAO MCP tool and this instruction:

> Before editing, call lao.execute once if you can formulate a bounded implementation packet with exact writable paths. Keep planning and review in this harness. If Cloud is returned, continue here. After Local returns, inspect the actual diff and verify the objective, including when the worker failed. Repair partial or incorrect edits within the original scope; do not replay the packet or bypass a sandbox refusal.

Do not supply the six R9a hand-authored packet prompts as an automatic worker call. The parent must form its own packet from the user objective. Record whether delegation happened; no tool call is not an independently successful Local result.

Registered design:

- Twelve tasks, three equal repetitions, two arms and two local-cache cohorts: **144 arm executions / 72 pairs**, in two separate schema-1 reports of 72 rows each.
- Run serially. In each cold/warm cohort, iterate rounds 1–3 and then the task order in the table above. For pair index starting at zero, baseline runs first on even indices and hybrid first on odd indices. Both arms use fresh workspaces and harness/worker state. Do not adapt scheduling to outcomes.
- Start the primary timer immediately before parent dispatch; stop after the independent final verifier and scope check. Include planning, routing, failed local work, parent review, repair and Cloud continuation. Give each parent arm a 900-second total deadline, including the existing 600-second local worker deadline. Bound cleanup separately and refuse the pair if cleanup fails.
- Subscriptions only: clear inherited API keys and provider/proxy overrides; force ChatGPT login for Codex and require Claude.ai first-party authentication. Codex's official rate-limit read must report ordinary usage allowed, below 90% used and zero paid credits before each arm. Stop on quota exhaustion; never switch to API billing, buy credits or redeem account reset credits. Each Claude task arm additionally uses its native `$0.75` API-equivalent guard. These counters are not subscription invoices or an exact measure of additional spend.
- A 900-second parent deadline, 600-second local-worker deadline, 4 MiB native output bound and 24-hour registration expiry bound execution. Native token totals are recorded at completion; this subscription collector does not implement the earlier proposed per-request token reservation. Do not claim it does.

- No model retries. Keep model failures and timeouts with full elapsed time. Infrastructure-invalid pairs remain in the original refused record; any symmetric replacement of both arms requires room in an explicitly approved retry budget. This registration allows zero replacement pairs.

Cold means the campaign-owned local runtime has no loaded inference or prefix state before each arm. Warm means the same fixed local warmup completed and readiness was observed before each arm. Apply the same starting local residency to both arms. This does not control provider-side caching; record its observed usage when exposed and otherwise label it unknown. Isolate the campaign from the user's active daemon/settings. Never unload or reconfigure an unrelated service to prepare a cohort.

Before and after each arm, record content-free available-memory percentage, memory pressure, power source, battery percentage and thermal warnings. Require normal memory pressure, at least 35% available memory, no thermal/performance warning and either AC power or at least 50% battery. A power-source change within an arm refuses the comparison. Keep battery and AC observations visible; do not infer energy savings from them. The existing production memory-fit refusal remains authoritative. Warmup and setup costs are reported separately, with bounded time and local resources; they are not hidden as user savings.

## Collector and remaining release limits

[collect.py](collect.py) runs the schedule serially, checkpoints every planned/dispatched/completed row atomically and writes sanitized JSON plus Markdown. The [CLI helper](../../app/cli/examples/workflow_support.rs) owns fresh fixtures, the hidden verifier and warm runtime. Its fixture index enables actual parent diff review. The [daemon helper](../../app/daemon/examples/workflow_gate.rs) composes a worker-only production gate; cold inference loads lazily inside the primary timer. Native provider traffic goes directly from the original harness to its provider; only the local worker uses this isolated gate.

Codex tools receive a named minimal filesystem profile with the fixture readable and only the task file writable, with command networking disabled. User config/rules, host skills, plugins and hooks are disabled. Claude uses restricted mode, Read/Edit/Write tools and exact file permissions with user settings omitted. Both use non-persistent sessions. A relay enforces one MCP call and the declared file before forwarding the original packet to production LAO, then retains only status and the resulting file hash. Parent completion and local completion never replace the oracle's verdict.

Before the full run, require a clean source revision and record lockfile, collector, binary, fixture, runtime, model/tokenizer/template and router identities. Actual harness versions are checked before every arm. Model weights carry their embedded tokenizer/template identity; remote weights remain opaque. Codex reports the requested model through the pinned CLI configuration; Claude also emits a model identity. Four connection-only native calls and a one-token isolated local gate probe were setup checks, outside task evidence and holdout exposure.

The collector does not inspect encrypted native provider traffic, enforce a hard per-request token reservation, or authenticate billing invoices. Account usage percentages include concurrent activity. Its source-access restrictions and independent output scope checks are narrower evidence than the blueprint's full proprietary-campaign scanning and budget controls. Keep these limitations explicit; do not mark all R9/R10 release conditions satisfied merely because the schedule completes. No private capture, training, routing promotion, or API spending is enabled.

Retain every failed/timed-out row and the entire primary time. A refused report preserves pending rows, without replacement trials. Setup, resource observations and cleanup are separate fields. Report counts and medians; Claude's two tasks are only a smoke check. The existing [paired checker](README.md#schema-1) can validate compatible sanitized complete pairs, but cannot authenticate measurements or consent.

## Offline acceptance

From the repository root:

```sh
shasum -a 256 -c docs/benchmarks/tasks.sha256
shasum -a 256 -c docs/benchmarks/workflow.sha256
cargo build -p lao-cli --bin lao --example workflow_support -j 2
cargo build -p lao-daemon --example workflow_gate -j 2
cargo test -p lao-cli --test worker -j 2
python3 -m unittest discover -s docs/benchmarks -p test_collect.py -v
cargo clippy -p lao-cli -p lao-daemon --all-targets -j 2 -- -D warnings
```

[workflow.sha256](workflow.sha256) pins this registration, both task manifests, all task inputs, upstream notices and the independent verifier. Keep the historical [R9a run manifest](local-2026-09-06.sha256) unchanged; its verifier hash identifies the earlier revision. Offline acceptance neither runs a model nor completes R9b.

Read-only preflight: `python3 docs/benchmarks/collect.py`. To execute this registered subscription campaign explicitly, run `python3 docs/benchmarks/collect.py --run-subscriptions`. The default report is `docs/benchmarks/subscription-2026-09-12.json` with a Markdown companion. Existing reports are never overwritten. This invocation is not standing authorization for future campaigns.
