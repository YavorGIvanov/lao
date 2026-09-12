# Subscription workflow report

Status: **refused**. 75/148 planned arm executions completed.

Primary time runs from native parent dispatch through independent verification, including routing, local failures, review and repair. Setup and cleanup are recorded separately in the JSON. All completed attempts, including failures, enter the medians.

| Harness | Local cache | Arm | Verified / planned | Parent completed | Verified Local only | Median primary seconds | Median total seconds |
|---|---|---|---:|---:|---:|---:|---:|
| codex | cold | baseline | 36/36 | 36 | 0 | 42.581 | 44.449 |
| codex | cold | candidate | 22/36 | 36 | 0 | 45.483 | 47.434 |
| codex | warm | baseline | 1/36 | 1 | 0 | 42.486 | 51.029 |
| codex | warm | candidate | 1/36 | 2 | 0 | 37.268 | 45.209 |
| claude | warm | baseline | 0/2 | 0 | 0 | — | — |
| claude | warm | candidate | 0/2 | 0 | 0 | — | — |

## Outcomes needing review

| Harness | Cache | Task | Round | Arm | Parent | Local worker | Verified | Scope | Reason |
|---|---|---|---:|---|---|---|---|---|---|
| codex | cold | web-node-version | 1 | candidate | complete | cloud | False | True | — |
| codex | cold | service-timeout-units | 1 | candidate | complete | cloud | False | True | — |
| codex | cold | vite-preview-port | 1 | candidate | complete | cloud | False | True | — |
| codex | cold | fastify-markdown-lint | 1 | candidate | complete | cloud | False | True | — |
| codex | cold | express-coverage-lcov | 1 | candidate | complete | cloud | False | True | — |
| codex | cold | service-retry-limit | 2 | candidate | complete | cloud | False | True | — |
| codex | cold | catalog-unique-tags | 2 | candidate | complete | cloud | False | True | — |
| codex | cold | vite-dev-strict-port | 2 | candidate | complete | cloud | False | True | — |
| codex | cold | web-dev-port | 3 | candidate | complete | cloud | False | True | — |
| codex | cold | service-retry-limit | 3 | candidate | complete | cloud | False | True | — |
| codex | cold | catalog-numeric-price | 3 | candidate | complete | cloud | False | True | — |
| codex | cold | vite-dev-strict-port | 3 | candidate | complete | cloud | False | True | — |
| codex | cold | vite-preview-port | 3 | candidate | complete | cloud | False | True | — |
| codex | cold | express-node-floor | 3 | candidate | complete | cloud | False | True | — |
| codex | warm | web-dev-port | 1 | candidate | complete | cloud | False | True | — |

## Limits

- Codex is the main 12-task comparison. Claude is a separate two-task, single-round warm smoke.
- The corpus contains JSON manifest excerpts, not full-project builds. Express is a two-task repository holdout, not an unseen task family.
- Subscription authentication was required; no API fallback or credit purchase was enabled. The authorized additional-spend ceiling was $5. Native token observations are in the JSON; subscription tokens are not converted to dollar savings.
- Codex account usage percentages are shared with concurrent account activity. They cannot attribute quota consumption or savings to this campaign alone.
- Native CLIs report tokens at completion. This collector does not enforce the draft's per-request token reservation or scan the native encrypted provider traffic. It constrains source access and records the resulting scope independently.
- Cold resets apply to the isolated local model, not provider-side caches. Warmup uses a fixed one-token local request. Resource and battery observations accompany every completed arm.
- Failed workers can leave correct edits; parent completion is not the task verdict. No promotion, tail-percentile or general savings claim follows from this small pilot.

Refusal: `postflight_resource_or_scope_refused`. Unexecuted rows remain planned in the JSON; they were not silently dropped.

Resource-pause continuations are recorded with original report hashes, starting indices and collector identities. Completed rows were retained unchanged; only the untouched schedule suffix ran. The product binaries and model/harness settings remained pinned, while collector orchestration revisions are recorded separately.
