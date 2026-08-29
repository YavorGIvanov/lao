# Local Agent Optimizer: Decision-Complete Implementation Plan

Research snapshot: 23 August 2026

Target license: MIT

Primary implementation language: Rust

Default inference engine: llama.cpp in C/C++

Optional adapter language: Python, isolated out of process

> **North star for every agent:** choose the simplest elegant solution that works. Minimize code, concepts, dependencies, configuration, comments, and operational parts while meeting the required behavior, security, privacy, and component boundaries. Future-proof with clean seams and tests, never speculative machinery.

## 1. Outcome

The first proof of concept is a small Apple Silicon overlay that:

- installs with one command;
- does not replace the Codex or Claude Code UI, approval model, or agent loop, while applying reversible user-level integration settings;
- preserves native cloud authentication on explicitly supported, version-gated client builds;
- selects and benchmarks one local model under a quantified resource policy;
- routes only the easiest 5–15 percent of low-risk tasks locally;
- adds less than 50 milliseconds p95 to native-cloud requests;
- consumes less than 100 MiB RSS while idle, excluding the lazily loaded inference worker;
- unloads the local model under idle or memory pressure;
- records metadata-only route outcomes by default;
- optionally captures valuable tasks after explicit local consent; and
- can replay an approved task subset to produce one comparative report.

This plan separates research spikes from build tasks. No feature progresses past a phase gate when its security or compatibility evidence is incomplete.

### 1.1 Rules for every implementation task

These rules apply to prototypes, production code, tests, configuration, and documentation:

- Build the smallest working vertical slice. Minimize lines of code, dependencies, processes, configuration, comments, and concepts together; do not optimize one by making the others worse.
- Future-proof through versioned seams, private state, golden fixtures, and replaceable adapters. Do not build speculative framework code, a generic plugin system, or unused extension points.
- Reuse a maintained upstream tool for the PoC when it passes license, security, resource, and compatibility checks. Pin the exact version and keep all upstream types and assumptions inside one adapter.
- Every reused component has an ownership trigger: replace or fork only when measured gaps, security response, performance, release control, or maintenance risk justify it. The contract and fixtures remain stable during replacement.
- Prefer deletion, composition, and standard-library/platform features over a new dependency or abstraction. A new layer must enforce a named boundary or remove more complexity than it adds.
- Keep names short and scoped. Use one or two clear words; rely on the directory/module namespace and avoid repeated words such as manager, service, controller, implementation, component, or interface.
- Write few comments. Comment only a security/privacy invariant, a non-obvious correctness reason, or a pinned upstream/platform quirk. Do not restate names or control flow.
- Use one concise diagram when it replaces substantial relationship prose. Do not duplicate the same design in multiple documents.
- A change is incomplete if it works only by bypassing a contract, accessing another component's state, or adding an undocumented private protocol.

Code review must ask: can code, a dependency, a layer, a configuration switch, or a comment be removed while preserving the acceptance criteria? If yes, remove it before merge.

## 2. Agent execution model

The north star above applies to every agent, every task, and every review. When two designs satisfy the same acceptance and safety criteria, agents must choose the one with less code and fewer concepts.

Each work package has one primary agent. The primary agent owns its research note, code, tests, required canonical-document updates, and handoff. Review agents do not edit the same subsystem concurrently.

Every agent task follows this sequence:

1. Read this plan, the product blueprint, and upstream licenses.
2. Inspect current upstream source and official client documentation.
3. Update the relevant decision and acceptance criteria in one of the two canonical documents when the task changes architecture; do not create a parallel design document.
4. Implement only the assigned public contract.
5. Add unit, integration, failure-injection, and privacy tests appropriate to the task.
6. Run the subsystem test matrix.
7. Produce a handoff containing changed interfaces, known risks, measurements, and follow-up work.

An agent may not weaken credential, privacy, resource, or consent gates to make a test pass.

### 2.1 Agent charters

| Agent | Charter | Principal ownership |
|---|---|---|
| A00 Architecture integrator | Maintains boundaries, design consistency, phase gates, and release assembly | workspace, shared types, dependency policy |
| A01 Codex integration | Exact Codex Responses behavior, configuration, hooks, version compatibility | Codex adapter and fixtures |
| A02 Claude integration | Exact Messages/SSE behavior, gateway configuration, hooks, compatibility | Claude adapter and fixtures |
| A03 Gateway security | Capability auth, credential firewall, upstream origin policy, adversarial tests | gateway auth and transport security |
| A04 Installer and lifecycle | Transactional install, backup, doctor, repair, rollback, pause, uninstall | config transaction engine |
| A05 Local runtime | llama.cpp supervision, cancellation, health, pressure eviction, backend contract | runtime supervisor |
| A06 Hardware and resources | hardware probing, memory topology, Light/Auto/Maximum admission | hardware and fit planner |
| A07 Catalog and artifacts | signed catalog, model choice, user preference, download, verification, rollback | catalog and artifact cache |
| A08 Routing | features, policy, stickiness, risk floors, escalation, outcomes | router |
| A09 Capture and privacy | hook correlation, repository checkpoint, redaction, importance, retention | capture pipeline |
| A10 Vault security | encrypted metadata/blob store, keys, export, delete, corruption recovery | local vault |
| A11 Evaluation harness | isolated task replay, verifiers, limits, campaign execution | eval runner |
| A12 Statistics and reports | paired analysis, evidence grades, recommendations, user reports | reports and metrics |
| A13 Platform and release | packaging, signing, services, compatibility CI, SBOM, updates | release engineering |
| A14 Training adapters | provenance eligibility, dataset renderer, MLX/Axolotl adapters, adapter rollback | post-v1 training |
| A15 Independent assurance | threat model, protocol fuzzing, privacy red team, release sign-off | cross-cutting review only |

### 2.2 Completed research tracks

The plan already incorporates four independent deep dives:

- gateway, Codex/Claude integration, auth preservation, and alternatives;
- local inference, hardware fitting, routing research, and lifecycle alternatives;
- capture, encrypted storage, personal evals, training, and provider-policy constraints;
- product differentiation, resource UX, ShoeHorn, Magnitude, and FreeToken.

Implementation agents must validate volatile upstream facts again before coding.

## 3. Repository structure

Use a contract-first modular monorepo. P0-00 creates every strategic boundary as a buildable package. Deferred components expose only draft status; operational methods appear during the spike that proves them. Backends and internal stages remain modules or adapters until independent state, trust, release, or ownership makes another package boundary useful.

Target layout:

    api/
      core/                    status and typed errors
      wire/                    bounded local-RPC projection
      client/                  client configuration boundary
      gate/                    gateway and credential state machine boundary
      route/                   routing boundary
      run/                     hardware, fit, and runtime boundary
      model/                   catalog and artifact boundary
      capture/                 classified capture boundary
      vault/                   encrypted storage boundary
      eval/                    replay, statistics, and report boundary
      train/                   eligibility, training, and promotion boundary
    svc/
      gate/                    gateway plus private credential state
      codex/                   Codex client adapter
      claude/                  Claude Code client adapter
      route/                   routing implementation
      run/                     hardware, fit, runtime, and backend adapters
      model/                   signed catalog and artifact lifecycle
      capture/                 ingress, private scrub stage, and snapshot
      vault/                   encrypted store
      eval/                    replay and reporting
      train/                   optional training adapters
    app/
      daemon/                  small hot-path composition root
      cli/                     installer and local control client
      capture/                 least-authority capture worker
      vault/                   least-authority vault worker
      eval/                    least-authority evaluation worker
      train/                   least-authority training worker
    test/
      kit/                     public contract conformance suite
    xtask/                     architecture, extraction, and release checks

Rules:

- Paths are short because the namespace carries meaning. Packages use the shortest unambiguous `lao-*` name; contract crates add `-api` only when needed.
- `api/*` may depend only on other APIs and reviewed external libraries. They never depend on a service or application.
- `svc/*` may depend on APIs, never a sibling service implementation. Concrete selection happens only in `app/*` composition roots.
- Cycles are forbidden. `cargo xtask check` reads Cargo metadata and fails CI on a forbidden edge.
- `[package.metadata.lao]` is the only component registry. It records kind, owner, API, private state owner, isolation class, and status. `[workspace.metadata.lao]` records restricted API consumers. `cargo metadata --format-version 1` drives enforcement and graph generation.
- Every stateful component exclusively owns its schemas and migrations. Shared databases, cross-component SQL, unversioned filesystem reads, and environment variables used as private inter-component APIs are forbidden.
- In-process calls use transport-neutral traits and immutable owned DTOs. The draft local RPC v0 uses a four-byte length prefix, typed JSON, a 1 MiB cap, service/version/capability negotiation, and one call per connection. Deadlines, idempotency, and semantic cancellation are defined only after a later spike proves them. No domain API mentions its wire format.
- Unsupported major versions fail before body decode or work. Additive optional fields are tolerated within a major; new required semantics use a capability or new major.
- Contract conformance suites run against every implementation. Linked and RPC adapters for the same contract must produce equivalent outcomes and typed errors.
- Private affine values represent credentials and raw capture only inside their owning service. Scoped serializable capabilities and typed non-authorizing IDs are distinct concepts; one generic handle is forbidden.
- Gateway code cannot access vault decryption keys.
- Credential ingress, route commitment, and egress materialization form one ordered private state machine in `svc/gate`; router sees only sanitized context and an immutable decision.
- Raw ingress, scrub, and snapshot are private modules in `svc/capture`; raw types never enter an API. Vault receives only classified/redacted artifacts or scoped references.
- The always-on daemon may link hot-path packages for latency and memory efficiency, but this does not relax dependency rules. llama.cpp is a separate process. Capture, vault, eval, and train each have a tiny least-authority worker launched only on demand.
- Shared foundation crates are not created speculatively. A utility becomes shared only after real duplication and at least two current consumers justify it.
- Python adapters cannot listen on external interfaces and are never resident by default.
- No crate logs request bodies, paths, secrets, capabilities, diffs, or decrypted artifacts.

Start in one repository. Extract a component only after the architecture and implementation plans record at least one of: independent release cadence, separate security/operations ownership, multiple external consumers, or a contributor community that needs autonomous governance. Before extraction, its contract must pass compatibility tests in both repositories, its state must already be private, and no application behavior may depend on workspace-relative paths or atomic cross-component database changes.

Pin the Rust compiler, Cargo.lock, llama.cpp build, optional sidecars, and release toolchain at the first implementation commit. Use rustls-based TLS and disable upstream redirects.

### 3.1 Package and process boundary matrix

| Capability | Package boundary | Default process | Owned durable state | Allowed communication |
|---|---|---|---|---|
| gateway/auth state machine and router | two independent components | `lao-daemon` | router policy/cache only | contract traits; supported HTTP at client edge |
| Codex/Claude adapters | one component per client | short-lived `lao` command | config transaction journal added in P0-05 | client contract only; no request bytes |
| hardware, fit, runtimes | run component with private backend adapters | `lao-daemon` plus supervised runtime | runtime state owned by run | Run contract plus loopback HTTP to inference worker |
| catalog and artifacts | model component | `lao-daemon` | catalog, artifact cache, and promotion state owned by model | Model contract only |
| capture, private scrub, snapshot | one ordered component | lazy `lao-capture-worker` | encrypted staging spool owned by capture | authenticated local RPC and classified outputs only |
| vault | independent component | lazy `lao-vault-worker` | exclusive ownership of vault DB, blobs, keys, migrations | authenticated Vault RPC only |
| eval and report | one component | on-demand `lao-eval-worker` | campaign work directory and signed results | Eval contract; vault access only through Vault API |
| optimization and training | model/train adapters behind separate APIs | on-demand `lao-train-worker` | candidate artifacts and provenance owned by model/train | approved references and versioned RPC/JSONL only |

The table is an initial deployment profile, not permission to merge ownership. A linked call may be replaced with RPC, or a package extracted to another repository, without changing the domain contract.

## 4. Contracts and private boundaries

### 4.1 ClientAdapter

Required operations:

- detect client installation and version;
- calculate an installation preview;
- apply managed configuration transactionally;
- install asynchronous hooks;
- verify local and native paths;
- pause/resume without removing backups;
- restore or uninstall conflict-safely.

Implementations: Codex and ClaudeCode.

During P0-02/P0-03, version policy, precedence, conflicts, and previews remain private to each client service; `api/client` stays minimal. Add shared semantic doctor records or the full trait only when a real second consumer needs them. Client services never handle request bytes or provider credentials. In P0-05, each adapter may receive only its own caller token through a restricted transient install input, write it directly into managed settings, and never return, log, or retain another copy. Gate owns generation, runtime validation, and rotation; the app only coordinates the transaction.

### 4.2 Gate boundary

Credential sealing and egress materialization are private ordered stages inside `svc/gate`; their secret-bearing types never cross a package boundary. The public gate contract exposes only sanitized routing context and the immutable route decision boundary. The router sees header classes and client metadata but never values.

Private egress-stage inputs:

- client kind and version;
- immutable final route class;
- request origin/path, sanitized non-secret headers, and an opaque SealedNativeCredentials handle;
- provider-policy snapshot.

Private egress-stage outputs:

- exact upstream origin;
- allowed headers;
- stripped-header audit classes without values;
- redirect policy;
- injected explicit-provider credential handle when applicable.

Native credentials are legal only for a matching native route and exact official origin. The per-client `X-LAO-Key` is always consumed locally. The Anthropic protocol/OAuth capability in `anthropic-beta` is preserved only for the exact native Anthropic route. Local and third-party routes receive neither. A route cannot change after an egress-auth action is created.

### 4.3 ManagedRuntime and ExternalEndpoint

ManagedRuntime operations:

- probe;
- prepare a verified model and FitPlan;
- start;
- health;
- benchmark;
- cancel request;
- stop;
- report pressure and runtime metrics.

ExternalEndpoint operations are probe, immutable fingerprint, health, benchmark, and request. It has no prepare/start/stop/delete/reconfigure authority.

The PoC managed implementation is DirectLlamaCpp. V1 adds LlamaSwap as managed lifecycle plus non-owning ExistingOllama and ExistingLmStudio endpoints. FreeToken is post-v1 and Maximum-only.

### 4.4 RouterPolicy

Input TaskContext includes:

- client/session/turn/repository fingerprints;
- task type and cognitive-shape scores;
- risk class;
- prompt and estimated context size;
- required protocol/tool/media capabilities;
- local benchmark and circuit state;
- repository and user override;
- personal evidence summary.

Output RouteDecision includes:

- Local or NativeCloud;
- selected model;
- confidence;
- difficulty and risk;
- reason codes;
- policy version;
- sticky boundary;
- permitted recovery.

### 4.5 Capture boundary and ArtifactStore

Raw ingress, scrub, snapshot, and commit are private ordered stages inside `svc/capture`. Unscanned bytes exist only in memory or the encrypted spool. The scrub stage is the sole raw-data inspector. The public capture contract exposes only classified/redacted artifacts and opaque references.

ArtifactStore rejects unclassified plaintext and exposes opaque handles. It supports inspect-with-local-auth, retention, pin, encrypted-by-default export, delete, wipe, integrity verification, key rotation, and migration.

### 4.6 EvalRunner

EvalRunner receives an immutable approved EvalCampaign and returns signed EvalTrials. It may not expand tasks, models, data categories, permitted file manifest, tool/network scope, repetitions, limits, or budget. It scans every transformed cloud-bound request and tool result at runtime.

## 5. Phase 0: risk-reduction spikes

No production feature code begins until the corresponding spike passes.

### P0-00 — Boundary and extraction proof

Owner: A00 with A15 review

Dependencies: none

Implementation: 29-package Rust 1.98 workspace, `lao-wire` draft JSON v0 projection, `lao-test-kit`, and Cargo-metadata `xtask` enforcement.

Create the minimal workspace, empty strategic packages, contract-only dependency graph, one linked fake implementation, and one RPC fake implementation. Confirm the modular-monorepo decision in the two canonical documents before any protocol or runtime spike establishes an accidental private interface.

Deliverables:

- package-local Cargo classification and workspace policy for allowed dependency edges;
- skeleton contract, component, application, and lazy-worker packages;
- transport-neutral semantic contract plus version/capability handshake;
- `cargo xtask check` using Cargo metadata;
- a temporary extraction drill that builds one fake component outside the workspace against the synthetic public conformance contract.

Acceptance:

- every strategic component named in Section 3 builds independently, including deferred components as honest draft stubs;
- injected service-to-service, restricted-API, dependency-cycle, duplicate/foreign-state, escaped-migration-root, and ambient workspace-path violations fail automated checks;
- linked and local-RPC fake implementations pass the same conformance suite;
- previous/current additive fields interoperate and unsupported majors, missing capabilities, malformed frames, oversized frames, and unknown operations fail before dispatch;
- macOS local RPC verifies the peer user before decoding the handshake;
- the active self-contained core and wire packages pass Cargo's pristine package build;
- an external fake builds from an unrelated temporary directory against pristine-packaged core/wire crates and the same public conformance source, then passes linked/RPC parity.

Research basis: [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html), [versioned Cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html), [pristine package verification](https://doc.rust-lang.org/cargo/commands/cargo-package.html), [Rust dyn compatibility](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility), [Serde evolution rules](https://serde.rs/container-attrs.html), and [tonic's transport/codegen surface](https://docs.rs/tonic/latest/tonic/). P0-00 deliberately does not adopt tonic or protobuf; Section 3 records the later adoption triggers.

### P0-01 — Rust protocol proxy feasibility

Owner: A00

Dependencies: P0-00

Build a throwaway loopback Rust proxy that streams one sanitized Responses fixture and one Messages fixture in both directions without buffering the full body.

Deliverables:

- a concise transport decision added to this plan;
- streaming and cancellation measurement;
- list of fields that require transformation for local llama.cpp;
- decision on whether a temporary CCR sidecar is needed.

Acceptance:

- first byte is forwarded without full-response buffering;
- cancellation closes upstream and downstream;
- tool-call IDs and event order remain byte-equivalent on pass-through;
- no content logging;
- native-cloud overhead below 20 milliseconds locally for the fixture.

Result (2026-08-24): passed. The dependency-free `std::net` spike in `svc/gate/tests/proxy.rs` streams complete HTTP/1.1 requests and chunked SSE responses for sanitized Responses and Messages fixtures. It preserves every byte, header, event, and tool identifier; carries two exchanges over one persistent connection; and closes the opposite side after either peer disconnects. A paired 21-trial full-fixture measurement on an Apple M4 MacBook Air in the Rust debug test profile measured 118.291 microseconds median and 150.167 microseconds p95 first-byte overhead. The full proxy suite then passed 100 consecutive runs.

Transport decision:

- Keep the native-cloud path as a raw streaming bridge. Do not parse, buffer, or transform pass-through bodies.
- Put local llama.cpp translation behind the gate boundary. Responses translation must handle model/input/instructions, tools and tool choice, streaming event types, response/item IDs, function-call IDs and argument deltas, status, errors, and usage. Messages translation must handle model/system/messages content blocks, tools and input schemas, tool choice, max tokens, thinking, streaming event types, message/content/tool-use IDs, JSON deltas, stop reasons, errors, and usage.
- Preserve unknown fields only on native pass-through. Local translation must reject unsupported required capabilities rather than silently drop them.
- A Claude Code Router sidecar is not needed for transport. P0-02 and P0-03 may still use a time-boxed compatibility sidecar only if current client behavior cannot be reproduced narrowly in Rust.

### P0-02 — Codex subscription compatibility spike

Owner: A01

Dependencies: P0-01

Using current official Codex documentation and open-source implementation, verify a managed custom provider against a fake upstream. Keep the real-subscription smoke as a separate, explicitly authorized final gate.

Acceptance:

- LAO never opens or copies real Codex auth storage;
- ChatGPT-subscription versus API-key mode is determined through supported client status/behavior, never auth-file inspection, before subscription preservation is claimed;
- the spike writes only to a disposable Codex home; P0-05 owns real configuration and rollback;
- fake upstreams receive synthetic sentinel credentials only; a real credential is sent only in a manual smoke test to the exact official origin;
- Codex 0.146.0 uses a custom `lao` provider with `requires_openai_auth = true`, `supports_websockets = false`, a non-secret `/oai` prefix, and a separate `X-LAO-Key` header;
- synthetic native auth and caller auth coexist, but neither appears in complete client stdout or stderr;
- the shared user setting's possible IDE-extension impact is reported before any future write;
- any test-only origin injection exists only in test builds and cannot be configured in a release binary;
- unsupported versions fail closed with a clear doctor result;
- behavior is documented as compatibility, not a permanent provider guarantee.

Synthetic result (2026-08-25): passed for installed `codex exec` 0.146.0. An ignored opt-in test under `svc/codex/tests` uses a disposable Codex home, synthetic native auth, and a distinct caller token. It observed an HTTP/SSE `POST /oai/responses` through the custom provider, proved both header classes arrived, completed a streamed response, and found neither token in stdout or stderr. It touched no real config or auth. Run with `cargo test -p lao-codex --test installed -- --ignored`. The later native-login E2E is recorded under P0-04. Tools, compact/models endpoints, errors, cancellation, and configuration writes remain separate gates.

### P0-03 — Claude subscription gateway spike

Owner: A02

Dependencies: P0-01

Verify base URL plus custom caller-header configuration, effective credential precedence, beta capability forwarding, SSE pings, errors, and session identifiers using official gateway behavior.

Acceptance:

- no API key helper or imported credential;
- saved account remains active only when no other credential/provider wins effective precedence;
- the spike writes only to a disposable Claude home; P0-05 owns real configuration and rollback;
- Claude Code 2.1.223 preserves a non-secret `/ant` prefix and proves its unauthenticated bodyless hello probe and caller-authenticated Messages SSE request shape;
- synthetic native auth and caller auth coexist on payload requests, but neither appears in complete client stdout or stderr;
- because current official documentation declares `ANTHROPIC_CUSTOM_HEADERS` supported from 2.1.227, production support remains closed until that or a later version passes the same installed probe;
- existing ANTHROPIC_API_KEY, ANTHROPIC_AUTH_TOKEN, apiKeyHelper, workload identity, provider settings, and shell base-URL overrides are detected and presented as conflicts rather than deleted;
- effective-configuration smoke tests prove which credential/base URL wins before claiming subscription preservation;
- Remote Control and unproven IDE/Desktop/background surfaces are reported unsupported; auxiliary traffic outside the model gateway is not described as intercepted.

Synthetic result (2026-08-25): passed for installed Claude Code 2.1.223 in `--bare --print` mode. An ignored opt-in test under `svc/claude/tests` uses disposable homes, synthetic native auth, and a distinct caller token. It observed the unauthenticated bodyless `HEAD /ant/api/hello`, then a caller-authenticated Messages request with the native bearer still present, settings-over-shell base-URL precedence, protocol beta/version/session headers, SSE ping, and streamed text. It found neither token in stdout or stderr and touched no real config or auth. Run with `cargo test -p lao-claude --test installed -- --ignored`. The later saved-login E2E is recorded under P0-04. Errors, cancellation, configuration writes, and effective-config resolution remain separate gates.

### P0-04 — Credential firewall proof

Owner: A03

Dependencies: P0-02, P0-03

Implement an isolated prototype with exact route/origin/path allowlists, per-client caller-header validation, header classification, redirect rejection, and fake adversarial upstreams.

Acceptance:

- exhaustive fixture matrix proves no native secret reaches local or third-party routes;
- scheme, host, port, path, DNS rebinding, redirect, and case/encoding confusion fail closed;
- missing, wrong, duplicate, conflicting, and cross-client caller tokens fail before a payload body is read;
- the exact Claude hello probe is the only unauthenticated path and cannot carry a body or trigger state;
- `Host` is validated, CORS is absent, and caller tokens never appear in product-controlled logs, errors, telemetry, crash reports, or support bundles;
- a hostile pre-bound port fails setup before configuration changes; a supervisor-held socket remains owned across daemon crashes and restarts;
- native pass-through preserves unknown fields, error bodies, and retry-relevant headers;
- all routes consume `X-LAO-Key`; local routes rebuild a clean request without native auth, Anthropic protocol/OAuth capability data, or client identifiers;
- induced failures scan complete supported-client stdout/stderr; any echo is documented and triggers rotation guidance;
- route is immutable after egress auth is created and router code cannot inspect/clone/serialize sealed credentials;
- firewall runs before protocol rewriting.

Network decision (2026-08-28): use narrowly featured [Hyper HTTP/1](https://docs.rs/hyper/latest/hyper/server/conn/http1/) for both client ingress and upstream egress, direct [tokio-rustls](https://docs.rs/tokio-rustls/latest/tokio_rustls/) with the [platform verifier](https://docs.rs/rustls-platform-verifier/latest/rustls_platform_verifier/) for native HTTPS, and no general HTTP connector. Resolve once under a deadline, reject any non-public answer, connect the concrete address, and complete certificate and hostname verification before Hyper can send native authentication. Build the exact URI only after route freeze and add no redirect middleware.

Integrated synthetic result (2026-08-27): `svc/gate/src/policy.rs` is the single private policy implementation and `svc/gate/src/net.rs` consumes only its frozen request. Admission validates exact Host, methods, paths, framing metadata, CORS absence, and separate caller capabilities. Freeze validates the client/native-auth matrix, strips hop-by-hop data, rebuilds local headers from an allowlist, and binds the route, semantic target, Host, and path before the first upstream connection. Synthetic Codex-cloud and Claude-local exchanges carry Hyper `Incoming` bodies without application buffering, return SSE, strip request and response hop-by-hop fields plus reflected credential headers, preserve permitted unknown native headers, and prove local bytes contain none of the credential sentinels. Missing, wrong, duplicate, cross-client, or auth-confused inputs create no upstream connection, including a rejected request with a declared but unsent body. A 3xx response is not relayed. Ten focused tests replace the duplicate test-only firewall implementation.

Native E2E result (2026-08-28): opt-in tests ran the installed harnesses against this same gate. Codex 0.146.0 reused its saved ChatGPT login, and Claude Code 2.1.223 reused its saved claude.ai login. Each completed one fixed, tool-free prompt over platform-verified TLS. LAO did not open either credential store, inject a provider key, persist client configuration, or print request headers. Only the non-secret caller-token sentinel was scanned in complete client output. These are compatibility observations, not a permanent provider guarantee; the Claude 2.1.227-or-later floor still needs the same test.

Supervisor E2E result (2026-08-29): the real `lao-daemon` adopts one launchd socket named `gate`, requires exactly one nonzero IPv4 loopback listener, and has no production self-bind fallback. An explicit opt-in test uses a temporary 0700 directory and 0600 plist for an active-session LaunchAgent. It proved a hostile pre-bound port never produces an adoption signal, the unconfigured daemon serves only the inert Claude hello, payloads fail closed, launchd retains the port after `SIGKILL`, and a later hello starts a replacement daemon. The fresh 0600 file proves socket adoption only; P0-05 must also pass the hello before changing client configuration. Bootstrap status alone is insufficient. Cleanup uses `bootout` and preserves diagnostics if that fails. Ad-hoc development binaries may trigger macOS approval; release installation requires one stable signed and notarized daemon identity and must prove upgrades do not create repeated prompts.

P0-04 remains open. Native TLS, bounded DNS, active-session supervisor ownership, and the closed daemon are implemented. Test code can still map a frozen target to a loopback fixture. Sealed router handoff, complete errors/retries/cancellation, client-output failure scanning, boot-wide ownership, and real configuration remain unimplemented. A per-user LaunchAgent does not own the socket across logout or reboot; production persistence requires a privileged system LaunchDaemon or explicit rollback/re-activation. Hyper 1.11.0, hyper-util 0.1.20, Tokio 1.53.1, rustls 0.23.43, tokio-rustls 0.26.4, and rustls-platform-verifier 0.7.0 remain narrowly pinned.

### P0-05 — Transactional configuration proof

Owner: A04

Dependencies: P0-00

Prototype byte-preserving backup and managed user-settings edits for Codex TOML and Claude Code CLI configuration. Do not use a launcher shim as the primary path.

Acceptance:

- failure injection after every write stage restores original bytes and permissions;
- two independent 256-bit caller tokens are generated, stored under owner-only directory/file permissions, and rotated atomically;
- a transaction lock and crash journal make repeated apply, repair, rollback, and interrupted replacement deterministic;
- managed client config becomes active only after the OS supervisor owns and verifies the loopback socket, and rollback completes before that ownership is released;
- platforms without equivalent continuous socket ownership cannot enable persistent interception;
- concurrent installers serialize;
- changed user files are never overwritten silently;
- the PoC support claim is limited to Codex CLI/TUI and Claude Code CLI; IDE, desktop, and background surfaces remain unsupported until separate adapters pass;
- dry-run shows exact managed keys;
- uninstall restores byte-identical files in the no-conflict case.

### P0-06 — Direct llama.cpp proof

Owner: A05

Dependencies: P0-00

Launch a pinned Apple Silicon llama-server from Rust, verify health, Responses, Messages, cancellation, tool fixtures, lazy unload, crash cleanup, and port security.

Acceptance:

- loopback-only ephemeral port;
- random runtime capability;
- no orphan process or listener after crash/cancel/uninstall;
- one local task completes through each client protocol fixture;
- measured startup and memory are recorded.

### P0-07 — Resource admission proof

Owner: A06

Dependencies: P0-06

Implement Apple unified-memory detection, Metal working-set visibility, non-negative per-memory-pool Light/Auto/Maximum formulas, load-time reassessment, and pressure eviction.

Acceptance:

- unified memory is never counted twice;
- discrete VRAM and host RAM remain separate constraints and A below reserve resolves to zero;
- fixtures cover 16/24-GiB unified systems, 8/12-GiB display GPUs plus host RAM, cgroups, and pressure;
- resolved absolute budget is displayed before download;
- simulated pressure reduces admission or evicts safely;
- Light uses at most its 25-percent ceiling, Auto 45 percent, Maximum 70 percent;
- daemon remains below 100 MiB idle excluding worker.

### P0-08 — Catalog and preferred-model proof

Owner: A07

Dependencies: P0-06, P0-07

Define a minimal signed manifest with two fixture models and one production candidate. Implement immutable revision, expected length/hash, license, template, context, capability, and benchmark fields.

Acceptance:

- corrupt, expired, rolled-back, revoked, oversized, or wrong-hash data is rejected;
- preferred model is never silently substituted;
- preview shows model, quantization, download, total working set, context, and expected speed;
- interrupted download resumes and promotes atomically.

### P0-09 — Routing corpus and baseline

Owner: A08

Dependencies: P0-00, product blueprint

Create a labeled, non-sensitive corpus of task categories, risk floors, context/capability cases, and expected reason codes. Implement the static scoring baseline as a library.

Acceptance:

- all security/destructive cases force cloud in Hybrid;
- Local only never invokes cloud;
- task difficulty and risk remain separate;
- the easiest fixture set yields an initial 5–15 percent local rate;
- decisions are deterministic and versioned.

### P0-10 — Repository snapshot proof

Owner: A09

Dependencies: P0-00

Create exact sanitized snapshots for clean, staged, unstaged, renamed, deleted, symlinked, binary, LFS, submodule, sparse, and untracked fixtures.

Acceptance:

- replay reconstructs allowed bytes and modes exactly;
- ignored, credential, build, and oversized paths are absent by default;
- unstable concurrent writes are detected;
- original final patch is not present in the pre-task replay;
- no repository code executes during capture.

### P0-11 — Vault cryptography proof

Owner: A10

Dependencies: P0-00

Prototype SQLCipher metadata and authenticated streaming artifact encryption with OS Keychain integration.

Acceptance:

- strings cannot recover prompts, repository names, paths, diffs, or metadata;
- reordered, truncated, modified, or substituted ciphertext fails authentication;
- failed writes leave the old vault consistent;
- key loss and backup/deletion limitations are documented;
- fuzz the minimal unsafe/FFI boundary.

### P0-12 — Eval sandbox proof

Owner: A11

Dependencies: P0-10

Implement disposable Apple Silicon replay using Apple Container CLI as the preferred PoC sandbox and a Docker-compatible engine as fallback. Container support is optional to install and is not required for normal routing; if neither backend passes isolation checks, evaluation is disabled.

Acceptance:

- candidate sees only the approved pre-task source and instruction;
- oracle patch, trajectory, rating, and hidden verifier are inaccessible;
- time, CPU, memory, process, tool, and network limits are enforced;
- the run produces an environment digest and final patch;
- absence of a sandbox disables eval rather than degrading silently.

### P0-13 — Statistical report proof

Owner: A12

Dependencies: P0-12

Generate a paired two-model report from deterministic fixture trials.

Acceptance:

- task-level results, pass rates, paired deltas, confidence intervals, median/p90 latency and cost, hard regressions, evidence grade, and limitations appear;
- a one-run campaign is labeled insufficient;
- critical regression blocks general promotion;
- output cannot claim hidden quantization.

### P0-14 — Independent threat review

Owner: A15

Dependencies: P0-00 through P0-13

Review route confusion, credential leakage, local capability exposure, config rollback, malicious repository/model/verifier data, compromised clients/hooks, same-user limits, signed-policy/catalog rollback and stale-clock attacks, vault corruption/exhaustion/backups/key rotation, decompression bombs, cross-project correlation, export boundaries, eval escape, brokered provider keys, and cost caps.

Exit gate P0:

- P0-00 must prove dependency enforcement and repository extraction without a private-state dependency.
- P0-02 through P0-04 must have no unresolved credential-severity finding.
- P0-05 must restore every failure fixture.
- P0-06 and P0-07 must meet idle/resource targets.
- P0-10 and P0-11 must pass privacy/integrity fixtures.
- Any critical finding returns the affected spike to its owner.

## 6. Phase 1: Rust foundation

### F-01 — Workspace and dependency policy

Owner: A00

Dependencies: P0 gate

Harden the P0 workspace with license and advisory policy, deny/audit configuration, shared CI, release profiles, and feature policy. Promote the P0 boundary and extraction checks into required CI. Do not recreate or expand the topology without a proven boundary.

Acceptance:

- clean checkout builds with one documented command;
- dependency licenses are compatible or explicitly reviewed;
- unsafe code is denied except named audited modules;
- Cargo.lock is committed;
- every strategic component has package-local `metadata.lao`, a package manifest, public contract reference, test target, and draft status where functionality is deferred;
- every package builds and tests independently with `cargo test -p`, and change-scoped CI can select it without building unrelated optional workers;
- boundary, cycle, state-ownership, and workspace-relative-path checks are mandatory and cannot be skipped by feature selection;
- release profile strips symbols into separate debug artifacts and uses reproducible settings.

### F-02 — Shared schemas and migrations

Owner: A00

Dependencies: F-01

Implement schema-versioned ModelManifest with backend-specific artifact format, HardwareSnapshot, per-pool FitPlan, BenchmarkResult, TaskContext, RouteDecision, RouteOutcome, artifact-level ProvenanceNode/DAG, TaskBundle metadata, EvalCampaign approval digest/scope, EvalTrial, EvalReport, ConsentPolicy, and typed errors. Keep domain DTOs wire-independent. Add bounded JSON projections only for real worker calls; adopt Prost only if the documented cross-language, independent-release, remote-RPC, or measured-performance trigger is met.

Acceptance:

- JSON golden fixtures round-trip;
- unknown additive fields are tolerated where compatibility requires;
- breaking schema changes require migrations;
- previous/current contract fixtures pass in both directions and unsupported major versions fail before work or state mutation;
- Rust linked calls, local RPC, and Python JSON/JSONL projections have documented semantic mappings for the fields they support;
- identifiers containing provider/user data are keyed or encrypted.

### F-03 — Content-free observability

Owner: A00 with A15 review

Dependencies: F-01

Implement structured local logs and metrics with a compile-time prohibited-field list and redaction wrappers.

Acceptance:

- fixture secrets, paths, prompts, capabilities, diffs, and headers never appear;
- request IDs are local keyed identifiers;
- debug mode still does not log bodies;
- support bundle is locally previewable.

### F-04 — Local control API

Owner: A00

Dependencies: F-02, F-03

Expose authenticated local operations for status, pause, resource mode, model preview, doctor, reports, and shutdown. Prefer a Unix-domain socket on macOS/Linux and named pipe on Windows; use loopback only where the platform adapter requires it.

Acceptance:

- same-user access only;
- no remote/LAN access;
- capability rotation works;
- CLI commands are idempotent.

### F-05 — Component conformance and state isolation

Owner: A00 with A03, A10, and A15 review

Dependencies: F-02, F-03, F-04

Build reusable conformance suites for every public contract and certify each reference, disabled, linked, and RPC implementation. Add failure injection at process, protocol, and persistence boundaries.

Acceptance:

- the same behavioral fixtures and typed-error assertions run against linked and RPC implementations;
- cancellation, deadline, backpressure, duplicate request, worker crash, worker restart, and version mismatch have deterministic behavior;
- no component can query or migrate another component's state, and a schema migration cannot require an atomic write across owners;
- no raw credential, plaintext task content, vault key, raw repository path, or mutable shared reference crosses an interface not explicitly authorized for that data class;
- lazy workers can be absent or stopped while normal cloud routing continues;
- replacing one reference component with the extracted fake from P0-00 requires only composition-root configuration.

Exit gate F:

- P0-00 architecture invariants are enforced in required CI.
- All strategic packages exist and pass their independent build, documentation, and contract checks.
- No Phase 2 feature begins until its contracts and conformance suite are stable at v1-alpha.
- The always-on empty daemon meets the idle target with lazy workers stopped.

## 7. Phase 2: invisible routing vertical slice

### G-01 — Gateway core

Owner: A03

Dependencies: F-02, F-03, P0-01

Implement streaming ingress, per-client caller-header authentication, request limits, cancellation, timeout, and route dispatch for the exact required Responses and Messages endpoints.

### G-02 — Codex client adapter

Owner: A01

Dependencies: G-01, F-04, P0-02, P0-05

Implement detection, managed config, hooks, smoke test, pause, repair, and restoration.

### G-03 — Claude client adapter

Owner: A02

Dependencies: G-01, F-04, P0-03, P0-05

Implement base URL plus managed caller header, hooks, protocol headers, smoke test, pause, repair, and restoration.

### G-04 — Production credential firewall

Owner: A03

Dependencies: G-01, G-02, G-03

Harden and wire the proven private firewall, with exact official origin policy, no redirects, minimal native forwarding, and total local/third-party stripping.

### G-05 — Protocol conformance corpus

Owner: A01 and A02, with A15 as reviewer

Dependencies: G-02, G-03, G-04

Cover streaming text, reasoning/encrypted state and continuation fields, parallel tools, IDs/order, count/compact endpoints used by clients, exact anthropic-version and full anthropic-beta, x-claude-code-session-id, SSE pings, unknown additive headers/body/events, cancellation, malformed events, context overflow, 401/403/429/5xx, request/response backpressure, and disconnects before/after output.

Acceptance for G-01 through G-05:

- native behavior is byte-preserving where no transformation is required;
- tool IDs/order are retained;
- native error bodies and retry-relevant headers survive unchanged;
- no full response-body buffering;
- local routes remove all native auth/account/attestation/protocol-capability data;
- no reroute after output or possible side effects;
- cloud proxy overhead below 50 milliseconds p95;
- compatibility matrix covers current and previous supported client versions.

### G-06 — Content-free task boundary tracker

Owner: A01 and A02 with A08

Dependencies: G-02, G-03, F-04

Install a same-user-authenticated hook endpoint that creates persisted task_run_id values from client/session/turn/repository metadata while capture remains off.

Acceptance:

- concurrent sessions and turns never cross-assign;
- duplicate and out-of-order start/stop/incomplete events are idempotent;
- crashed sessions close as incomplete;
- hooks use exec-form arguments with no shell interpolation;
- endpoint rejects other users;
- no prompt, transcript, file, or tool-result body is retained before capture consent;
- no unstable transcript parser is used.

### I-01 — Hardware snapshot

Owner: A06

Dependencies: F-02, P0-07

Implement normalized Apple hardware, memory, pressure, power, and llama.cpp-visible device data.

### I-02 — Resource profiles

Owner: A06

Dependencies: I-01

Implement B-p = max(0, min(profile fraction × T-p, A-p − R-mode,p)) independently for each memory pool; CPU/priority/context/disk/residency policies; pre-load reassessment; battery/thermal downgrade; pressure eviction. Unified memory is one pool; discrete VRAM and host RAM are separate FitPlan constraints.

### I-03 — Direct llama.cpp backend

Owner: A05

Dependencies: F-02, I-02, P0-06

Implement pinned binary discovery, generated flags, capability auth, startup parsing, health, authoritative normalized hardware/runtime metrics, cancellation, graceful stop, crash recovery, and worker TTL. Any llama-swap telemetry added later is supplemental.

### I-04 — Signed model catalog

Owner: A07

Dependencies: F-02, P0-08

Implement offline root trust, timestamp/snapshot/targets metadata, expiry, key rotation, revocation, last-known-good behavior, license display, and compatibility constraints.

### I-05 — Artifact manager

Owner: A07

Dependencies: I-04

Implement preview, disk check, immutable download, partial resume, maximum size, streaming SHA-256, atomic cache promotion, dedupe, LRU removal, and rollback.

### I-06 — Candidate recommendation and preference

Owner: A07 with A06

Dependencies: I-02, I-04, I-05

Rank by license, backend, full working-set fit, context, verified tools/protocol, measured speed, curated prior, and user preference. llmfit may advise but cannot override real fit. Existing endpoints require a certified artifact/runtime fingerprint; any change, revocation, expiry, or incompatibility invalidates automatic routing.

Acceptance:

- one candidate is downloaded, not a speculative set;
- preference is honored when certified;
- any quant/context/profile change is explicit;
- failed candidate falls to a smaller verified option only after preview;
- LAO never pulls, deletes, stops, or reconfigures a user-owned endpoint;
- no permanent best model is hard-coded.

### I-07 — Onboarding benchmark

Owner: A05 with A07

Dependencies: I-03, I-06

Run cold load, one discarded warm-up, 20 timed prompt/decode repetitions, selected configured-context allocation with 2K output reserve, long-context retrieval/tool behavior at the maximum auto-routable context, tool fixtures, Responses/Messages fixtures, and peak memory capture for every selected mode/configuration.

Acceptance:

- Quality/Balanced/Fast thresholds and p10 gate are calculated from the required sample;
- configuration fails closed to Hybrid/Cloud only when context, tool, speed, or memory gates fail;
- Local only requires a warning and explicit one-task override;
- Auto never silently lowers its 64K selection target;
- the router never sends more than the context proven by retrieval/tool fixtures.

### R-01 — Feature extractor

Owner: A08

Dependencies: F-02, G-06

Implement task category, cognitive shapes, risk, context, capability, and confidence from prompt plus hook context without cloud calls.

### R-02 — Static policy

Owner: A08

Dependencies: R-01, I-07, P0-09

Implement thresholds, risk floors, modes, per-repository rules, model pins, reason codes, and policy snapshots.

### R-03 — Task-run stickiness

Owner: A08

Dependencies: R-02, G-06

Persist the route state machine across gateway restarts using keyed client/session/turn/repository identifiers.

### R-04 — Pre-output recovery and circuit breaking

Owner: A08 with A03 and A05

Dependencies: R-03

Implement typed runtime failures, at most one retry of a complete replayable request before the first output byte, no post-output retry, next-task conservative routing, explicit cloud-retry guidance, and the three-failures/five-minutes/ten-minute circuit breaker keyed by exact model/artifact/quantization/backend/engine/flags/device.

Automatic semantic repair or task restart is excluded from Phase 2. It requires C-02/E-01 plus a separate client-control feasibility task and explicit replay consent.

### R-05 — Outcome metadata

Owner: A08

Dependencies: R-03, G-05

Record route, transport result, verifier summary, acceptance when available, loops, tool errors, latency, tokens, and cost without bodies.

### U-01 — One-command installer

Owner: A04

Dependencies: G-02, G-03, I-07, R-04, F-04

Implement install, dry-run, resource/model selection, signed payload verification, service setup, smoke tests, transaction commit, doctor, pause, repair, rollback, standalone native bypass, and uninstall.

Acceptance:

- Cloud only uses the healthy gateway and native upstream;
- lao bypass restores/removes managed base URLs and hooks without contacting the daemon;
- daemon-absent launch, crash mid-request, corrupt compatibility metadata, and transactional bypass are tested;
- no request is replayed after output or a possible tool side effect.

### U-02 — Silent status and weekly report

Owner: A12

Dependencies: R-05, F-04

Report local attempts, verified successes, fallbacks, quota/API tokens avoided, latency, resource pressure, override rate, and learned evidence. Do not insert normal-route messages into Codex or Claude.

Exit gate P2:

- easy fixture and design-partner tasks route through both clients to llama.cpp;
- hard/risky tasks preserve native cloud behavior;
- all credential-isolation fixtures pass, and any suspected or confirmed misroute blocks the gate until resolved;
- daemon and resource targets pass;
- installer rolls back every injected failure;
- local route remains capped to the conservative policy;
- a design partner can work through the original client without learning a new harness.

## 8. Phase 3: capture and personal evaluation

### C-01 — Hook event ingestion

Owner: A09

Dependencies: G-06, F-04

Extend the content-free boundary tracker with a consent-gated private capture ingress. Raw material may exist only in memory or an encrypted crash-safe spool.

Acceptance:

- hook overhead below 50 milliseconds p95;
- hook failure never blocks the client;
- no transcript parser is required;
- raw provider IDs are not stored;
- duplicate/out-of-order events remain idempotent under concurrent sessions;
- no body is retained before consent;
- spool files, SQL journals/WAL, crash dumps, process arguments, and sidecar input reveal no plaintext.

### C-02 — Bounded working-tree snapshotter

Owner: A09

Dependencies: P0-10, C-03

Stream bounded repository files through the private scrub stage to produce approved encrypted/redacted archives and binary-safe patches that reconstruct base tree, index, worktree, and untracked layers separately; include recursive initialized submodules and safe already-present LFS bytes; detect sparse/partial/history boundaries and unstable files.

Acceptance:

- clean, staged, unstaged, renamed, deleted, executable-bit, newline, symlink, binary, and untracked fixtures reconstruct byte-for-byte;
- recursive submodule, nested worktree/repository, case-collision, Unicode-normalization, partial-clone, missing-LFS, and sparse fixtures either reconstruct or receive an explicit non-replayable reason;
- symlinks cannot escape the snapshot;
- capture never fetches LFS/partial objects or executes code;
- unrelated concurrent edits require review;
- replay initializes a synthetic Git repository while preserving index/worktree distinction.

### C-03 — Privacy scanner

Owner: A09 with A15 review

Dependencies: C-01

Implement path policies, pinned Gitleaks sidecar, provider/key/entropy/PII rules, stable task-local placeholders, whole-file exclusion, transitive provenance classification, and fail-closed durable-content eligibility.

Acceptance:

- fixture tokens, keys, connection strings, env files, encoded secrets, and PII never reach durable plaintext;
- scanner failure discards full content and retains at most content-free metrics;
- repeated values use stable task-local placeholders;
- the private commit stage and ArtifactStore reject unclassified plaintext;
- local-model artifacts receive artifact-level provenance; proprietary/unknown ancestors taint descendants;
- exact runtime campaign requests are scanned separately under E-04.

### C-04 — Consent and importance

Owner: A09

Dependencies: C-03

Implement capture-off default, one-time local consent, per-project exclusions, deterministic importance, dedupe, diversity, pin, and retention class.

Acceptance:

- routing and content-free boundary tracking work with capture off;
- consent is purpose-specific, inspectable, revocable, and versioned;
- accepted/verifier-passing meaningful local tasks become candidates while no-op/duplicate/privacy-ineligible tasks do not;
- valuable failures can be retained without becoming training eligible;
- provider-policy capture/retention gates keep proprietary trajectories and cloud-influenced patches out of reusable pools until legal eligibility is reviewed.

### V-01 — Encrypted vault

Owner: A10

Dependencies: F-02, P0-11

Implement SQLCipher metadata, authenticated streaming blobs, OS keystore, vault KEK plus random per-artifact wrapped DEKs, keyed dedupe IDs, atomic writes, integrity verification, key rotation, and migrations.

Acceptance:

- at-rest files, WAL, journals, temporary files, arguments, and environment reveal no content or keys;
- modified/reordered/truncated/substituted ciphertext fails authentication;
- deleting a wrapped DEK makes the artifact practically unrecoverable from the active vault;
- locked/unavailable keystore fails closed without data loss;
- failed write or rotation leaves a consistent recoverable vault;
- sensitive buffers are zeroized where practical and the audited cryptographic/FFI boundary is fuzzed.

### V-02 — Retention and user control

Owner: A10

Dependencies: V-01, C-04

Implement seven-day low-value metadata, 90-day candidates, 10-GiB default cap, pinned retention, inspect, encrypted-by-default export, delete, wipe, derivation tracking, backup expiry, and cryptographic-erasure documentation.

Acceptance:

- expiry and size GC preserve pinned artifacts;
- export requires destination/data preview and creates a new explicit key boundary;
- deleting a source enumerates dependent datasets/adapters and requires retire/retrain or influence disclosure;
- disk exhaustion and decompression-bomb fixtures fail atomically;
- backup, SSD, snapshot, key-loss, and trained-weight deletion limits are documented.

### E-01 — Native replay

Owner: A11

Dependencies: C-02, V-01, P0-12

Reconstruct a clean pre-task environment, hide oracle data, prepare dependencies in a separate pinned phase, launch a pinned client/harness, broker provider access without exposing raw keys, enforce limits, collect patch and verifiers, and destroy the sandbox.

Acceptance:

- candidate sees only approved instruction, pre-task source, tools, and network scope;
- oracle patch/trajectory/rating/hidden verifier is inaccessible;
- agent and verifier run default-deny network;
- time, memory, CPU, process, tool, token, and cost limits hold;
- environment and input digests are stable;
- absent sandbox disables eval rather than weakening isolation.

### E-02 — Verifier hierarchy

Owner: A11

Dependencies: E-01

Implement immutable-hash/provenance verifiers, pre-task baseline execution, deterministic task checks, existing tests, build/type/lint, patch/scope rules, tool/loop/timeout metrics, and optional secondary judge.

Acceptance:

- pre-existing failures are distinguished from candidate regressions;
- original acceptance selects a task but never scores a new candidate;
- original-assistant tests require user review before becoming hidden ground truth;
- model failure/timeout counts as failure;
- infrastructure-invalid trials are labeled and rescheduled symmetrically within budget.

### E-03 — Local campaign

Owner: A11

Dependencies: E-02, I-07

Run paired local candidates with equal limits, randomized order, pinned environment, and reproducibility digest.

Acceptance:

- baseline, endpoints, task weights, critical tasks, category claims, repetitions, and analysis-plan version are frozen before dispatch;
- candidates receive equal limits and repetitions in randomized interleaved order;
- rerun from the manifest reproduces environment and input digests;
- no candidate sees another candidate's output.

### E-04 — Proprietary approval and provider policy

Owner: A11 with A03 and A15

Dependencies: E-02, V-02

Implement signed provider-policy registry, exact initial-payload preview plus maximum disclosure scope, API/commercial credential requirement, authenticated campaign digest, approval expiry, timestamped pricing, runtime request scanning, reservation, and hard token/currency caps.

Acceptance:

- no external request before approval;
- dispatch recomputes the digest over task/snapshot IDs, provider/models, harness, scan policy, disclosure scope, repetitions, limits, price version, and expiry;
- changes or stale/unknown pricing invalidate approval;
- every transformed cloud-bound request and tool result is scanned before any byte leaves the device;
- a turn-two secret or data outside the approved manifest/scope aborts and requires renewed approval;
- consumer OAuth is rejected for unattended campaigns unless explicitly authorized by current policy;
- worst-case cost is reserved before each trial, actual use is reconciled, and retries cannot exceed token or currency caps;
- provider authorization and legal retention/training eligibility remain separate decisions.

### E-05 — Comparative statistics

Owner: A12

Dependencies: E-03, E-04

Implement pre-registered paired counts, task outcome as mean of equal repetitions, two-sided 95-percent task-cluster percentile bootstrap with 10,000 resamples and recorded seed, optional McNemar for one-trial tasks, median/p90 cost and latency, critical regression, multiplicity labeling, evidence grade, and promotion rules.

Acceptance:

- general promotion requires no critical regression, success-delta lower bound at least minus five points, and either the success-delta or utility-delta lower bound above zero;
- category promotion uses the same rule only for a pre-registered category;
- multiple-candidate claims use multiplicity control or are exploratory;
- fewer than 20 distinct tasks suppress p90 and strong recommendations;
- binary failures/timeouts and infrastructure-invalid trials follow E-02 semantics.

### E-06 — Report UI

Owner: A12

Dependencies: E-05, F-04

Produce terminal and machine-readable reports with task categories, uncertainty, examples, limitations, and reproduce command. Label behavioral change without causal claims.

Acceptance:

- report contains task-level evidence, aggregate deltas/intervals, hard regressions, latency/cost/tool failures, evidence grade, manifest, and limitations;
- insufficient campaigns explicitly decline a strong recommendation;
- regression alert requires a ten-point significant drop plus independent confirmation; latency alert requires a confirmed 25-percent paired-median increase;
- wording remains “behavioral regression suspected” without hidden-model causal claims.

### E-07 — Semantic retry feasibility

Owner: A01, A02, A08, and A11

Dependencies: E-01, G-06

Determine whether each supported client can create a new explicit cloud task run from an approved pre-task checkpoint without duplicating side effects or mutating an opaque live conversation.

Acceptance:

- no automatic semantic retry ships unless the adapter proves a new task-run boundary, exact input reconstruction, user-visible lineage, and side-effect isolation;
- otherwise the product offers explicit lao retry --cloud and next-task cloud routing only.

Exit gate P3:

- no secret fixture enters durable plaintext or an approved cloud payload;
- repository fixture replay is exact within its declared sanitized boundary, and non-replayable cases are explicit;
- sandbox oracle isolation passes;
- a two-model fixture report is reproducible;
- small samples are not overclaimed;
- deletion and export behave as documented.

## 9. Phase 4: first supported release

### X-01 — macOS production packaging

Owner: A13

Dependencies: P2 and P3 gates

Universal packaging where supported, Apple signing/notarization, LaunchAgent service, Keychain, atomic updater, SBOM, provenance, rollback.

### X-02 — Linux platform

Owner: A13 with A05/A06/A10

Dependencies: X-01 architecture

systemd user service, Secret Service, CPU/CUDA/HIP/Vulkan detection, cgroups, package formats, and sandbox adapter.

### X-03 — Windows platform

Owner: A13 with A05/A06/A10

Dependencies: X-01 architecture

Windows service or user scheduled background process, Credential Manager, DXGI/WMI/NVIDIA/AMD probes, named pipes, config ACLs, installer/uninstaller, and sandbox adapter.

### X-04 — Runtime adapters

Owner: A05

Dependencies: I-03

Add llama-swap as the default managed multi-model lifecycle adapter; add non-owning Ollama and LM Studio ExternalEndpoint adapters. LAO may fingerprint, health-check, benchmark, and request them but may not pull, delete, stop, or reconfigure their models/global settings. llama-swap telemetry supplements rather than replaces HardwareProbe admission.

### X-05 — Full hardware matrix

Owner: A06 and A13

Dependencies: X-02, X-03, X-04

Test macOS Apple Silicon and Intel where supported, Windows/Linux CPU, NVIDIA, AMD HIP/Vulkan, WSL detection, containers, multiple GPUs, display GPU, unified memory, and pressure.

### X-06 — Compatibility CI

Owner: A01, A02, A13

Dependencies: G-05

Continuously test current and previous supported Codex/Claude versions, pinned llama.cpp builds, catalog recipes, hook schemas, and native/cloud fake upstreams.

### X-07 — Compatibility kill switch

Owner: A00 and A13

Dependencies: X-06

Signed compatibility metadata can disable an unsafe local route and choose Cloud only while the gateway is healthy. For actual Native bypass it instructs the standalone transactional lao bypass path; it cannot repair configuration through a dead daemon. It cannot enable capture, upload, training, or spending.

Acceptance:

- corrupt, replayed, downgraded, expired, or stale-clock metadata fails closed;
- a kill switch cannot add an upstream origin or relax credential/privacy policy;
- Cloud only and Native bypass are tested as distinct states.

### X-08 — Pressure-aware Auto

Owner: A06

Dependencies: X-05

Use observed foreground workload, battery, thermal, free memory, and eviction history to recommend a lower or higher profile. Changes require user acceptance and never exceed the selected ceiling.

### X-09 — Model recipe certification

Owner: A07

Dependencies: X-05, X-06

Certify each exact artifact, quantization, template, engine build, backend, context, and tool protocol. Publish signed evidence and revocations.

Release gate:

- all hard security/reliability metrics in the blueprint pass;
- supported-platform install/uninstall and pressure matrices pass;
- no unsigned catalog or update path;
- privacy red team and independent assurance approve;
- legal review covers proprietary evaluation policy before public enablement.

## 10. Phase 5: personal intelligence

### L-01 — Personal evidence aggregates

Owner: A08

Dependencies: P3 gate

Maintain Beta-Binomial posteriors by model, task family, shape, language, and repository with hierarchical priors, decay, deletion rebuild, and minimum samples.

### L-02 — Lower-bound routing

Owner: A08

Dependencies: L-01

Permit medium tasks locally only when the tenth-percentile success bound clears the user's quality floor. A few successes cannot promote a weak model.

### L-03 — Router replay benchmark

Owner: A12

Dependencies: L-01, E-05

Compare Cloud only, Local only, static hybrid, CodeRouter-inspired policy, ACRouter baseline, and personalized policy on personal holdouts. Report regret, quality, cost, latency, and escalation.

### L-04 — Contextual ranker

Owner: A08 with A15/legal review

Dependencies: L-03, signed provider-policy eligibility

Train locally only on policy-eligible outcome features; proprietary-service outcomes are excluded by default until explicitly reviewed. Run shadow predictions, require held-out improvement, record calibration, and support automatic rollback.

### L-05 — Constrained exploration

Owner: A08 with A15

Dependencies: L-04

At most 5 percent Thompson-style exploration for low-risk reversible tasks within an explicit exploration budget. Never explore security/destructive tasks.

### L-06 — Regression monitor

Owner: A12

Dependencies: E-05

Use a stable sentinel set, provider-reported identifiers, and pinned harness/environment. Trigger a success alert only for an absolute drop of at least ten points whose 95-percent interval excludes zero and an independently scheduled confirmation reproduces it. Trigger latency only for a confirmed 25-percent paired-median increase with its interval above zero. Wording remains behavioral regression suspected.

## 11. Phase 6: advanced inference and fine-tuning

### A-01 — ShoeHorn artifact optimizer

Owner: A05 and A07

Dependencies: mature catalog and Maximum mode

Define an ArtifactOptimizer contract and expose an Advanced exact-fit workflow using a machine-readable ShoeHorn plan. Require explicit full-precision download, disk/time preview, imatrix provenance, held-out perplexity/quality check, and standard GGUF output for DirectLlamaCpp or llama-swap. Never treat ShoeHorn as a runtime backend or run it during default onboarding.

### A-02 — FreeToken adapter

Owner: A05

Dependencies: stable FreeToken API and release review

Maximum-mode only for supported NVIDIA Windows/Linux systems. Launch an isolated Python/CUDA environment through the ManagedRuntime JSON contract. Do not install on macOS, CPU-only, or unsupported GPUs.

### T-01 — Training eligibility

Owner: A14 with A15/legal review

Dependencies: V-02, E-05

Default-deny proprietary, mixed, unknown, holdout, unauthorized, secret, PII, and restricted-license data. Generate a readiness report only after 200 eligible diverse examples and 50 disjoint holdouts.

Acceptance:

- artifact-level provenance DAG records producer, provider/model, ancestors, policy revision, and repository/license basis;
- proprietary or unknown taint propagates transitively;
- holdouts cannot enter generator training;
- deleting a source identifies all derived datasets/adapters;
- readiness remains blocked by unresolved rights/privacy/policy findings regardless of count.

### T-02 — Dataset renderer

Owner: A14

Dependencies: T-01

Freeze source manifests, rescan, dedupe, split by repo/family/time, render model-specific tool/chat templates, and create sample-review reports.

Acceptance:

- frozen manifest and every rendered example trace to eligible source artifacts;
- semantic/exact duplicates and cross-repository/task-family leakage are detected;
- chronological holdout remains untouched;
- random sample plus every high-risk example is reviewable before training.

### T-03 — MLX-LM adapter

Owner: A14

Dependencies: T-02

First training backend for Apple Silicon. Version-pin environment, enforce resource/time limits, emit adapter and training manifest through JSONL.

Acceptance:

- optional Python environment is isolated, version-pinned, and absent from the resident daemon;
- resource, time, cancellation, and disk limits hold;
- output includes base hash, dataset manifest, config, seed, metrics, and adapter hash;
- no training starts without an approved frozen dataset.

### T-04 — Axolotl and optional Unsloth adapters

Owner: A14

Dependencies: T-02

Axolotl is the reproducible NVIDIA path; Unsloth is an optional fast path. Neither is a default daemon dependency.

Acceptance:

- both obey the same JSON/JSONL and provenance contracts as MLX;
- unsupported CPU/GPU environments fail before downloading training dependencies;
- output manifests are backend-comparable;
- no adapter is installed or promoted by the training process.

### T-05 — Adapter evaluation and promotion

Owner: A14 with A11/A12

Dependencies: T-03 or T-04

Evaluate untouched base and adapter on personal holdouts plus tool/general guardrails. Sign comparison. Require explicit approval for every promotion and retain base/prior adapter rollback.

Acceptance:

- identical pinned harness and limits compare base and adapter;
- critical personal, tool-call, format, and general-capability regressions block promotion;
- every promotion records explicit approval and can roll back atomically;
- standing consent may schedule training/evaluation but never promotion;
- source deletion reports whether the promoted adapter must be retired/retrained because selective unlearning is unavailable.

## 12. Cross-cutting test matrix

### Protocol

- Responses and Messages streaming.
- Parallel tools, ordering, IDs, unknown additive fields/events, encrypted reasoning/state and continuation fields.
- Exact anthropic-version, full anthropic-beta, x-claude-code-session-id, and keepalive behavior.
- Count/compact endpoints used by clients.
- Keepalive, cancellation, malformed SSE, slow clients.
- 401/403/429/5xx and retry headers.
- Disconnect before and after first output.

### Security

- Native credentials never reach local/third-party endpoints.
- Local caller tokens never appear in product-controlled output or cross origin; supported-client stdout/stderr exposure is tested and rotation is atomic.
- Anthropic native capability reaches only the exact native Anthropic origin.
- Real credentials never enter fake-upstream tests.
- Redirect, DNS, host, path, encoding, and case confusion.
- LAN access and CORS rejected.
- Malicious GGUF metadata, oversized artifacts, wrong hashes, expired signatures.
- Support bundle and debug log leakage.

### Configuration

- Clean install, existing custom settings, concurrent install.
- Failure at every transaction stage.
- User edits after install.
- Pause, upgrade, repair, rollback, dry-run, uninstall.
- Byte/permission restoration.

### Hardware

- Unified memory not double-counted.
- CPU, Metal, CUDA, HIP, Vulkan, multi-GPU, display GPU.
- Driver missing/mismatch and backend initialization failure.
- Pressure, battery, thermal, cgroups, WSL, container.
- OOM, crash, disk full, interrupted download, model revocation.

### Routing

- High risk forces cloud.
- Local only never leaks to cloud.
- Cloud never downgrades mid-task.
- Stickiness survives restart.
- One repair/one escalation and no thrash.
- Circuit breaker and user/repository/model overrides.
- Learned shrinkage, biased observations, holdout separation, exploration limits.

### Capture and vault

- Clean/staged/unstaged/renamed/deleted/symlink/untracked reconstruction.
- Recursive submodules, LFS bytes/missing objects, sparse/partial clones, nested worktrees, binaries, case/Unicode collisions, concurrent/unrelated writes.
- Tokens, PEM, connection strings, env files, encoded findings, scanner errors.
- Spool/temp/WAL/journal/crash-dump/process-argument leakage.
- Ciphertext modification, truncation, reorder, substitution, key rotation, locked keystore.
- Crash consistency, retention, pin, export/import, delete, key loss.

### Evaluation

- Oracle patch and trajectory hidden.
- Untouched baseline verifier before each candidate and immutable verifier provenance.
- Network/resource/tool/time/cost limits.
- Runtime scan of every cloud-bound turn/tool result and turn-two secret abort.
- Paired order randomization and equal repetitions.
- Small sample evidence grade.
- Critical regressions and non-inferiority.
- Approval invalidation and cap enforcement.

## 13. Phase metrics

### System metrics

- Idle daemon below 100 MiB RSS and no sustained CPU.
- Cloud proxy overhead below 50 milliseconds p95.
- Hook overhead below 50 milliseconds p95.
- Model unload: 2 minutes Light, 5 minutes Auto, opt-in residency Maximum.
- No local route exceeds the maximum context proven by long-context retrieval/tool fixtures with 2K output reserve.

### Product metrics

Evaluate over at least 300 real tasks, 10 repositories, and 10 design partners:

- 10 percent of eligible bounded sessions complete locally.
- 5 percent overall cloud token/quota reduction.
- no more than two points overall success decline, or at least 95 percent matched-cloud success for smaller samples;
- fewer than 20 percent manual route overrides;
- 80 percent install without manual repair;
- activation below 10 minutes excluding download;
- half of partners retain weekly use for four weeks.

### Security gates

- all credential-isolation fixtures pass, and any suspected or confirmed credential misroute blocks release until resolved;
- zero known secret-fixture cloud leaks;
- all config restoration fixtures pass;
- no supported-machine pressure crash;
- independent assurance has no open critical/high issue.

## 14. Decisions that are intentionally fixed

- The normal user continues to invoke Codex or Claude Code, not a replacement agent.
- The project begins as a contract-first modular monorepo with all strategic component boundaries scaffolded in P0-00.
- Services depend only on versioned APIs; only application composition roots select concrete implementations.
- Stateful components own private schemas and migrations; no shared database or cross-component SQL is permitted.
- Rust owns the trusted core.
- llama.cpp is the default inference engine.
- Python is optional and out of process.
- Mac Apple Silicon is the PoC; macOS/Windows/Linux and CPU/NVIDIA/AMD are v1.
- Cloud is the conservative default.
- Local PoC routing is intentionally small.
- Auto is the default resource mode and is quantitatively capped.
- User model preference is never silently replaced.
- One prequantized artifact is downloaded during onboarding.
- Exact-fit local quantization is advanced, not default.
- Capture is off until consent.
- Proprietary campaigns require separate approval and authorized credentials.
- Proprietary outputs are training-ineligible by default.
- Evaluation holdouts never train the generator.
- Automatic model promotion is out of scope.
- The repository uses MIT, while preserving all third-party notices.

## 15. Immediate execution order

The architecture prerequisite runs first:

1. A00 with A15 review: P0-00 boundary and extraction proof.

Only after P0-00 passes may the first implementation wave run these agents in parallel:

1. A00: P0-01 Rust protocol proxy.
2. A01: P0-02 Codex compatibility after the proxy contract exists.
3. A02: P0-03 Claude compatibility after the proxy contract exists.
4. A05: P0-06 direct llama.cpp proof.
5. A04: P0-05 transactional config proof.
6. A09: P0-10 repository snapshot proof.
7. A10: P0-11 vault cryptography proof.

The second wave begins when inputs are available:

1. A03: P0-04 credential firewall.
2. A06: P0-07 resource admission.
3. A07: P0-08 catalog/preference.
4. A08: P0-09 routing corpus.
5. A11: P0-12 eval sandbox.
6. A12: P0-13 report.

A15 performs P0-14 only after the prototypes and adversarial fixtures are available. A00 then records the gate decision and opens Phase 1. Phase 1 creates and certifies the entire strategic package topology before the Phase 2 vertical slice begins.

This ordering validates the highest-risk assumptions before investing in broad platform support, UI, learned routing, or training.
