# Local Agent Optimizer

Local Agent Optimizer is a working title for an open-source, nearly invisible layer between coding-agent clients and local or cloud models.

The product is intended to let people continue using Codex and Claude Code normally while it:

- discovers what local model can run comfortably on their hardware;
- routes a conservative subset of bounded tasks to that model;
- preserves the native cloud path for difficult or risky work;
- captures valuable, reproducible task evidence locally with consent;
- evaluates new models against the user's own work; and
- eventually supports explicitly authorized local-model personalization.

This repository contains the research-backed product specification, implementation plan, architecture skeleton, streaming proof, and installed-client compatibility probes. It does not yet contain production local routing.

## Manifesto

### Product

- Keep the user's harness.
- Stay invisible by default.
- Keep setup to one command.
- Ask only at real trust boundaries.
- Never ask twice for the same consent.
- Improve a little at first.
- Learn from measured outcomes.
- Use local only when it helps.
- Keep cloud as the safe path.
- Never consume the whole machine.
- Measure fit before download.
- Measure quality before promotion.
- Keep user data local and encrypted.
- Ask before capture, cloud eval, spend, or training.
- Make every recommendation explainable.
- Prefer evidence to claims.

### Code

- Choose the simplest elegant solution that works.
- Write less code.
- Keep names short.
- Keep comments rare.
- Comment why, not what.
- Reuse before rebuilding.
- Pin what we reuse.
- Add code only for a proven need.
- Hide upstream details behind our API.
- Own a component only when evidence justifies it.
- Build one narrow vertical slice at a time.
- Delete before abstracting.
- End every change with a careful simplification pass.
- Avoid speculative frameworks.
- Optimize for a human reading the code tomorrow.
- Make invalid states hard to express.
- Fail closed at trust boundaries.
- Test contracts and outcomes.
- Test real paths end to end when cheap.
- Keep the hot path small.

### Architecture

- One monorepo is not one monolith.
- Every component owns its state.
- No shared database.
- Components communicate through versioned APIs only.
- Components never import sibling implementations.
- Only apps wire concrete components together.
- Link the hot path when it saves resources.
- Isolate secrets, data, runtimes, eval, and training.
- Scaffold every strategic boundary early.
- Implement deferred behavior only when needed.
- Future-proof with seams and fixtures, not extra machinery.
- Keep extraction possible, not mandatory.
- Keep the visual architecture map current with the code.

## How to begin

1. Read the product architecture and implementation plan below.
2. Run `cargo xtask check`, `cargo test --workspace`, and `cargo xtask extract` to verify the boundary baseline.
3. Follow the seven Stage 1 tasks in the implementation plan.
4. Build only the real Codex + Claude → router → llama.cpp/cloud vertical slice.
5. Keep cloud as default and use one explicit bounded local canary.
6. Leave capture, eval, optimization, training, and extra backends disabled behind their APIs.
7. Measure fit, protocol behavior, credential isolation, cleanup, and rollback.

Every agent starts here. If two solutions are equally safe and functional, choose the smaller one.

## Documents

- [Visual architecture map](architecture.html)
- [Product vision and system architecture](PRODUCT_VISION_AND_ARCHITECTURE.md)
- [Decision-complete implementation plan](IMPLEMENTATION_PLAN.md)

## Current architectural decision

The target trusted core is a small Rust daemon and CLI. llama.cpp remains the default C/C++ inference engine. Python is restricted to optional, isolated evaluation and training adapters.

The project starts as a contract-first modular monorepo, not a monolith. Every strategic component is an independently buildable package with a versioned interface and private state; concrete components cannot import one another and are wired only by application composition roots. Hot-path packages may share the daemon process, while data-sensitive and lifecycle-heavy workers remain lazy and out of process. Components can move to separate repositories later without redesign when their release or ownership needs justify it.

Implementation follows the manifesto above.

The proof of concept is Apple Silicon-first, preserves the original Codex and Claude Code harnesses, defaults to cloud, and attempts only a small number of strongly bounded local tasks while it gathers verified evidence.

## Status

The cloud-safe baseline is complete: the private gate retains request bodies, headers, credentials, and targets, and gives the router only `Context(client, operation, canary)`. Normal contexts route to native cloud. Only an exact, non-secret canary selector may produce Local; the gate consumes it, removes native secrets, and binds the request to the runtime's loopback endpoint and private bearer.

S1-03 and S1-04 are complete. Pinned llama.cpp 10280 serves native Responses and Messages HTTP/SSE, so this slice passes request bodies and response streams without a translation layer and exposes the model only as `lao-local`. Installed Codex 0.146.0 and Claude Code 2.1.251 each completed the same real local canary through one gate, router, and model load; their saved-login cloud E2Es also pass.

`lao install` now generates separate caller keys, verifies launchd adoption and the exact inert hello before either client write, and applies the supported Codex and Claude settings under one owner-only lock and crash record. `lao off` detects post-install edits, restores original bytes and permissions, then releases the listener without reading provider credentials. Unit fault injection covers both client write boundaries. The clean real install/cloud/local/restart/off run remains S1-05, so this is not yet a supported release.
