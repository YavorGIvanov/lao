# Paired task evidence

Validate an already collected, sanitized pilot report:

```sh
cargo xtask evidence docs/benchmarks/example.json
```

The example is **synthetic**, including every identity and timing. It exercises the checker and is not a LAO benchmark. The command reads at most 1 MiB, prints aggregate JSON, and returns nonzero for unusable evidence. It runs no model, subprocess benchmark, verifier, capture, or network request. This is contributor tooling; `svc/eval` stays disabled.

The release sequence and acceptance gates live in [Mac beta milestones](../../IMPLEMENTATION_PLAN.md#next-steps); this offline checker is preparation, not completed task-benefit evidence.

The contract requires explicit measurement boundaries, pinned configurations, LAO's independent task verdict and content-free output.

## Public task suite (R9a)

[Tasks](tasks.json) declare six objectives across three small repository-owned JSON fixture projects: web manifest settings, service settings and catalog data. These are authored examples, not a sample of three real user repositories. The exact task corpus and [verifier/runner](../../app/cli/tests/worker.rs) are pinned by [tasks.sha256](tasks.sha256).

Run the offline fixture acceptance checks from the repository root:

```sh
shasum -a 256 -c docs/benchmarks/tasks.sha256
cargo test -p lao-cli --test worker -j 2
```

Every task starts in a fresh owner-only Git repository containing its input and an unchanged README guard. Git setup clears inherited environment and configuration. The independently declared JSON-pointer replacement defines the complete expected parsed document; all other values must remain identical. JSON whitespace and object-key order may change. Expectations remain in the parent test process, outside the worker workspace. No generated code or dependency is executed. Scope verification checks the whole fixture tree, including Git metadata, file modes, added/deleted paths and link/special-file rejection, with a 256-entry/1 MiB bound. Offline tests establish failing starting inputs, passing references and rejection of extra files or link replacements.

With an active default semantic LAO installation and its local runtime, explicitly run the diagnostic:

```sh
cargo test -p lao-cli --test worker -j 2 -- --ignored --nocapture
```

It sends each of the six fixed packets once, serially, then the broad Cloud control. It never invokes a cloud harness or continues a Cloud return. Each MCP subprocess has a 605-second outer deadline around the worker's existing 600-second limit, a 64 KiB output bound, and cleanup on normal return/error. An outer timeout is infrastructure-invalid, not an inferred Local result. Fixture directories are disposable; the production worker retains its existing parent-death cleanup. Abrupt termination is not a secure-erasure guarantee.

Output contains only public task IDs, route/status enums, independent verdicts, elapsed time and aggregate counts. A successful diagnostic exit requires usable infrastructure, intact file scope, at least one independently verified Local result and the unchanged Cloud control; it does **not** require or imply all tasks completed locally. Failed workers, timeouts and deferrals remain visible. Timing is `mcp_dispatch_through_verification` with uncontrolled cache state; it excludes parent model planning, review, repair and Cloud continuation. This is deliberately not schema-1 paired evidence and cannot support savings or promotion claims.

The [first local run](local-2026-09-06.md) verified two Local tasks and retained four Cloud deferrals. R9b still needs representative projects/holdouts, consented matched cloud/hybrid arms, controlled cohorts, actual artifact/resource observations and complete failure accounting. Reuse the fixtures and independent verifier there; do not tune these prompts to obtain more Local routes. If the fixture or verifier changes, review it and regenerate the hash manifest before a new run, retaining the prior results and their identities.

## Schema 1

Use [example.json](example.json) as the complete input shape. Unknown fields and enum values are rejected. Field names are case-sensitive. All fields are required.

| Field | Meaning |
|---|---|
| `schema` | Exactly `1`. |
| `evidence` | `synthetic` or `measured`; always preserved in output. The checker cannot authenticate this declaration. |
| `status` | `complete`, `incomplete`, or `refused`. Only complete evidence can produce a comparison. |
| `host` | Content-free hardware/OS/resource-cohort identifier, such as `m4-24g-macos26-light`. No username, hostname, serial, or repository path. Run separate reports on different machines. |
| `fixture_sha256`, `verifier_sha256` | SHA-256 of the exact public fixture manifest and independent verifier manifest, respectively. These manifests name all task IDs and hash all required files. Never supply private task content in this report. |
| `boundary` | Exactly `dispatch_through_verification`: parent dispatch through final independent verification, including review, repair, and Cloud continuation where present. Engine-only and gateway timings need a separate contract. |
| `cache` | `cold` or `warm`; use separate reports and matched starting conditions. Worker state is fresh in both. |
| `baseline`, `candidate` | Declared configurations; details below. |
| `tasks` | Unique public fixture IDs, 1–100. This is a pilot limit, not a statistically sufficient sample definition. |
| `rounds` | Equal repetitions, 1–5. One round is a smoke check; planned R9 pilots start with three. No sample size enables promotion through this tool. |
| `trials` | Exactly one row for every task × round × arm. Row order is execution order; alternate arms in collection. The checker validates membership/completeness, not scheduling or health. |

Identifiers use 1–128 ASCII letters, digits, dots, hyphens, or underscores. Hashes are hex strings of the declared length. They are syntactically checked; referenced files are not opened or hashed by this command.

Each configuration contains:

- `source_revision`: 40-character Git commit. The producer must also record source dirtiness in the artifact manifest and refuse dirty measured runs.
- `artifacts_sha256`: SHA-256 of the exact bytes of a producer-owned manifest covering source/lockfile, runtime binary, model weights/revision/quantization, tokenizer and template, harness binary/version, router policy, resource/context settings, and trial limits. For a cloud arm, include provider-reported model identity and relevant settings; do not imply remote model weights are hashable. This digest binds the other measured setup details without putting raw content or paths in output.
- `runtime`, `model`, `harness`: explicit versioned identifiers. Keep the actual cloud model/runtime identities in the manifest for hybrid arms. A version change requires a new declared configuration and report.
- `context`: declared common task context budget for that arm, 1–262,144. This syntax bound does not certify engine capacity or available memory. Equal workload/trial limits and effective context belong to trusted collection.

Each trial contains:

- `task`, one-based `round`, and `arm` (`baseline` or `candidate`).
- `observed`: the configuration independently observed for this arm; it must exactly match the declared configuration. Copying a declaration without checking actual versions defeats this protection. This catches a reported runtime version that differs from the installed dependency pins when truthfully collected.
- `route`: `cloud`, `local`, or `local_then_cloud`. The last value accounts for failed local work followed by parent Cloud continuation; the checker performs no fallback.
- `worker`: `complete`, `failed`, `timeout`, or `infrastructure_invalid`. Failed/time-out rows remain in the comparison. Infrastructure-invalid rows refuse the whole comparison; preserve the original refused report and retry pairs symmetrically only within an approved budget.
- `verified` and `scope_ok`: independent final task-verifier and exact patch-scope verdicts. Both must be true for a verified success. They are not derived from worker status. A worker that stops after a correct edit can still yield a verified task; a completed worker with an incorrect edit cannot.
- `elapsed_ms`: positive integer milliseconds, capped at 24 hours per pilot arm. Include the entire failed attempt and any repair; never use only successful generation time. Zero, fractional, negative, non-finite, and excessive values fail.

## Reading the output

The checker reports per-arm trial count, verified count, worker completion/failure/timeouts, scope failures, local-only and local-then-Cloud counts, and median total elapsed milliseconds over **all** trials. The paired success delta is candidate minus baseline in percentage points, with equal task/round denominators. It is descriptive, not a confidence interval. Route counts expose Cloud involvement; these aggregate counts do not give success by route. A final success after Cloud continuation must not be claimed as locally completed work.

The synthetic example deliberately has a completed but incorrect candidate row and a failed worker with an independently verified correct result. Baseline verifies 2/2, candidate verifies 1/2, candidate median time is 1,500 ms, and the delta is −50 percentage points. These invented values demonstrate why execution metadata cannot score correctness.

No report emits p90/p95, statistical significance, a routing recommendation, or a cost/quota-saving estimate. Validation cannot establish source authenticity, actual health/cache state, consent, honest timing, verifier provenance, correct scope, safe replay, or absence of cherry-picking. A trusted producer must enforce these before declaring measured evidence complete. See [R9](../../IMPLEMENTATION_PLAN.md#r9--measure-the-whole-bounded-task) for the planned collection gate and [personal evaluation statistics](../../PRODUCT_VISION_AND_ARCHITECTURE.md#112-statistics) for later promotion criteria.

Collection stays opt-in. R9a adds an explicitly invoked local diagnostic on public fixtures; it does not authorize private capture or cloud model campaigns. Do not put raw harness output, prompts, patches, process command lines, local paths, credentials, or user identifiers into reports. Parse errors and summaries deliberately omit supplied strings.
