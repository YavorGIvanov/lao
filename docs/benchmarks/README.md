# Paired task evidence

Validate an already collected, sanitized pilot report:

```sh
cargo xtask evidence docs/benchmarks/example.json
```

The example is **synthetic**, including every identity and timing. It exercises the checker and is not a LAO benchmark. The command reads at most 1 MiB, prints aggregate JSON, and returns nonzero for unusable evidence. It runs no model, subprocess benchmark, verifier, capture, or network request. This is contributor tooling; `svc/eval` stays disabled.

The contract requires explicit measurement boundaries, pinned configurations, LAO's independent task verdict and content-free output.

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
| `rounds` | Equal repetitions, 1–5. One round is a smoke check; planned R8 pilots start with three. No sample size enables promotion through this tool. |
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

No report emits p90/p95, statistical significance, a routing recommendation, or a cost/quota-saving estimate. Validation cannot establish source authenticity, actual health/cache state, consent, honest timing, verifier provenance, correct scope, safe replay, or absence of cherry-picking. A trusted producer must enforce these before declaring measured evidence complete. See [R8](../../IMPLEMENTATION_PLAN.md#r8--measure-the-whole-bounded-task) for the planned collection gate and [personal evaluation statistics](../../PRODUCT_VISION_AND_ARCHITECTURE.md#112-statistics) for later promotion criteria.

Collection stays opt-in. This task authorized offline tooling and synthetic data, not private capture or paid/unattended model campaigns. Do not put raw harness output, prompts, patches, process command lines, local paths, credentials, or user identifiers into reports. Parse errors and summaries deliberately omit supplied strings.
