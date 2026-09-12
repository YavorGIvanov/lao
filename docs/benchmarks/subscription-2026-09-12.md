# Subscription workflow report

Status: **refused**. 69/148 planned arm executions completed.

Primary time runs from native parent dispatch through independent verification, including routing, local failures, review and repair. Setup and cleanup are recorded separately in the JSON. All completed attempts, including failures, enter the medians.

| Harness | Local cache | Arm | Verified / planned | Parent completed | Verified Local only | Median primary seconds | Median total seconds |
|---|---|---|---:|---:|---:|---:|---:|
| codex | cold | baseline | 35/36 | 35 | 0 | 42.807 | 44.632 |
| codex | cold | candidate | 21/36 | 34 | 0 | 45.483 | 47.434 |
| codex | warm | baseline | 0/36 | 0 | 0 | — | — |
| codex | warm | candidate | 0/36 | 0 | 0 | — | — |
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

## Limits

- Codex is the main 12-task comparison. Claude is a separate two-task, single-round warm smoke.
- The corpus contains JSON manifest excerpts, not full-project builds. Express is a two-task repository holdout, not an unseen task family.
- Subscription authentication was required; no API fallback or credit purchase was enabled. The authorized additional-spend ceiling was $5. Native token observations are in the JSON; subscription tokens are not converted to dollar savings.
- Codex account usage percentages are shared with concurrent account activity. They cannot attribute quota consumption or savings to this campaign alone.
- Native CLIs report tokens at completion. This collector does not enforce the draft's per-request token reservation or scan the native encrypted provider traffic. It constrains source access and records the resulting scope independently.
- Cold resets apply to the isolated local model, not provider-side caches. Warmup uses a fixed one-token local request. Resource and battery observations accompany every completed arm.
- Failed workers can leave correct edits; parent completion is not the task verdict. No promotion, tail-percentile or general savings claim follows from this small pilot.

Refusal: `resource_guard_refused`. Unexecuted rows remain planned in the JSON; they were not silently dropped.
