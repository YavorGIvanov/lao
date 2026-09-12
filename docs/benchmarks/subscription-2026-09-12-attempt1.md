# Subscription workflow report

Status: **refused**. 10/148 planned arm executions completed.

Primary time runs from native parent dispatch through independent verification, including routing, local failures, review and repair. Setup and cleanup are recorded separately in the JSON. All completed attempts, including failures, enter the medians.

| Harness | Local cache | Arm | Verified / planned | Parent completed | Verified Local only | Median primary seconds | Median total seconds |
|---|---|---|---:|---:|---:|---:|---:|
| codex | cold | baseline | 5/36 | 5 | 0 | 40.051 | 42.245 |
| codex | cold | candidate | 0/36 | 5 | 0 | 25.024 | 27.279 |
| codex | warm | baseline | 0/36 | 0 | 0 | — | — |
| codex | warm | candidate | 0/36 | 0 | 0 | — | — |
| claude | warm | baseline | 0/2 | 0 | 0 | — | — |
| claude | warm | candidate | 0/2 | 0 | 0 | — | — |

## Outcomes needing review

| Harness | Cache | Task | Round | Arm | Parent | Local worker | Verified | Scope | Reason |
|---|---|---|---:|---|---|---|---|---|---|
| codex | cold | web-dev-port | 1 | candidate | complete | cloud | False | True | — |
| codex | cold | web-node-version | 1 | candidate | complete | cloud | False | True | — |
| codex | cold | service-timeout-units | 1 | candidate | complete | cloud | False | True | — |
| codex | cold | service-retry-limit | 1 | candidate | complete | cloud | False | True | — |
| codex | cold | catalog-numeric-price | 1 | candidate | complete | cloud | False | True | — |

## Limits

- Codex is the main 12-task comparison. Claude is a separate two-task, single-round warm smoke.
- The corpus contains JSON manifest excerpts, not full-project builds. Express is a two-task repository holdout, not an unseen task family.
- Subscription authentication was required; no API fallback or credit purchase was enabled. The authorized additional-spend ceiling was $5. Native token observations are in the JSON; subscription tokens are not converted to dollar savings.
- Codex account usage percentages are shared with concurrent account activity. They cannot attribute quota consumption or savings to this campaign alone.
- Native CLIs report tokens at completion. This collector does not enforce the draft's per-request token reservation or scan the native encrypted provider traffic. It constrains source access and records the resulting scope independently.
- Cold resets apply to the isolated local model, not provider-side caches. Warmup uses a fixed one-token local request. Resource and battery observations accompany every completed arm.
- Failed workers can leave correct edits; parent completion is not the task verdict. No promotion, tail-percentile or general savings claim follows from this small pilot.

Refusal: `interrupted`. Unexecuted rows remain planned in the JSON; they were not silently dropped.
