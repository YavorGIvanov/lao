# Local Agent Optimizer

Local Agent Optimizer is a working title for an open-source, nearly invisible layer between coding-agent clients and local or cloud models.

The product is intended to let people continue using Codex and Claude Code normally while it:

- discovers what local model can run comfortably on their hardware;
- routes a conservative subset of bounded tasks to that model;
- preserves the native cloud path for difficult or risky work;
- captures valuable, reproducible task evidence locally with consent;
- evaluates new models against the user's own work; and
- eventually supports explicitly authorized local-model personalization.

This repository contains the research-backed product specification, implementation plan, architecture skeleton, complete Stage 1 installed proof, and the first real automatic-routing slice. It does not yet contain production routing certification or release packaging.

## Install

This is a research proof for supported Apple Silicon Macs. It transactionally changes the user settings for both Codex and Claude Code. It does not read or copy either harness's credential store. `lao off` restores unchanged settings exactly and preserves unrelated settings the clients add while LAO is installed.

Clone the project, install the command, and let LAO finish setup:

```sh
git clone https://github.com/YavorGIvanov/lao.git && cd lao && ./install.sh && lao install
```

That is the whole setup. `lao install` detects the clients and machine, downloads and verifies the supported local runtime and model, applies the client settings transactionally, starts the service, and warms the local path in the background. There are no separate runtime packages, model servers, versions, or prerequisite checks to manage. Unsupported configurations stop safely without overwriting existing settings, and a partial install rolls back automatically.

Setup stops with a specific error when a prerequisite or existing configuration is unsupported. It verifies or downloads the immutable Qwen inference model and MiniLM router model (1,208,656,003 bytes total), so allow about 1.21 GB of network traffic and cache space on the first run.

Running the clone command and `lao install` again is safe. A healthy existing setup is verified and reused without downloading again, replacing its keys, or rewriting client settings.

At any time, check the whole installed path without consuming a model request:

```sh
lao status
```

`LAO: ready` means the service is running and both clients are routed through LAO. `local cache: warming` becomes `local cache: ready` after the background canaries finish. Ordinary work is available immediately on the cloud-safe path while warming continues. No credential or configuration value is printed.

## Test the experience

Open a new terminal in the repository where you already work and start a new Codex process:

```sh
cd /absolute/path/to/your/existing-project
codex
```

Use Codex normally. A separate local MiniLM classifier sends only a narrow, bounded first-turn request to local inference; uncertainty, complex work, unsupported shapes, and every classifier failure stay on the saved-login cloud path. For a visible automatic example, ask Codex: `Correct the spelling error in this one word: teh. Reply with only the corrected word.` The tested answer is `the` from the local Qwen model.

To prove the fixed local path through both real installed harnesses with sanitized output, run:

```sh
lao smoke
```

It prints only pass/fail and elapsed time. A warmed local response completes in roughly one to two seconds on the tested Mac. Run `lao status` first if you want to see whether background warming has finished; it does not consume a model request.

When finished—or immediately if a later check fails—restore both clients and stop LAO from any directory:

```sh
lao off
```

Codex and Claude may update their own unrelated settings while LAO is installed; `lao smoke` accepts those updates and `lao off` preserves them. LAO still refuses changes to the routing fields it owns. A successful `off` leaves no daemon, worker, listener, plist, runtime key, or log.

## Manifesto

### Product

- Keep the user's harness.
- Stay invisible by default.
- Keep setup to one command.
- Ask only at real trust boundaries.
- Never ask twice for the same consent.
- Improve a little at first.
- Learn from measured outcomes.
- Push the practical boundary of speed and efficiency.
- Treat mediocre performance as unfinished work.
- Move reusable work off the user's critical path.
- Never make the user wait for work that can happen safely in the background.
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
- Measure and optimize the hot path before accepting it.

### Architecture

- One monorepo is not one monolith.
- Every component owns its state.
- No shared database.
- Components communicate through versioned APIs only.
- Components never import sibling implementations.
- Keep optimization policy and state in its own component.
- Only apps wire concrete components together.
- Link the hot path when it saves resources.
- Isolate secrets, data, runtimes, eval, and training.
- Scaffold every strategic boundary early.
- Implement deferred behavior only when needed.
- Future-proof with seams and fixtures, not extra machinery.
- Keep extraction possible, not mandatory.
- Keep the visual architecture map current with the code.

## Contributor workflow

1. Read the product architecture and implementation plan below.
2. Run `cargo xtask check`, `cargo test --workspace`, and `cargo xtask extract` to verify the boundary baseline.
3. Keep cloud as the default and preserve the exact installed proof above.
4. Leave capture, eval, training, and extra backends disabled behind their APIs until the plan activates them.

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

The cloud-safe baseline is complete. The gate authenticates the caller before reading a body and retains headers, credentials, and targets. For automatic routing it extracts only bounded current-user text and asks the selected router for Local or Cloud. Cloud keeps the original body. A final Local decision creates a tool-free body containing only the final user text and model name `lao-local`, strips native secrets, and binds it to the runtime's protected loopback endpoint. For automatic routing, any unsupported body, router error, timeout, busy classifier, or unknown answer remains Cloud.

Stage 1 is complete on the supported test Mac. The pinned local runtime serves native Responses and Messages HTTP/SSE, so this slice passes request bodies and response streams without a translation layer and exposes the model only as `lao-local`. The supported installed Codex and Claude Code clients each completed saved-login cloud requests and the same real local canary through one gate and router, including after a daemon restart.

The clean automatic-route proof sent the no-canary spelling request through both harnesses: Codex and Claude each returned `the`. Measured daemon RSS was about 141 MiB with MiniLM loaded. These are proof measurements, not benchmarks.

`lao install` generates separate 256-bit caller keys, verifies launchd before either client write, and applies settings under one owner-only transaction. The separate optimizer then warms fixed local Claude and Codex canaries in the background. With both harness prefixes cached, the worker peaked at about 2.31 GiB under the 6 GiB Light ceiling. `lao off` restores client settings and removes the daemon, worker, listener, plist, optimizer state, runtime key, and log.

Each local response holds a runtime lease until its stream completes or is dropped. The healthy worker and both harness prefixes remain in a bounded RAM cache; a five-second watcher unloads them on idle memory pressure. Repeated `lao install`, `lao status`, and same-revision source setup are measured below 100 ms; gateway p95 overhead is below 3.5 ms. Model generation is reported separately.

The default `lao install` selection is `--router semantic --runtime llama-cpp`. `--router safe` keeps automatic work in Cloud, while `--router vllm-semantic` uses a user-managed vLLM Semantic Router decision endpoint. `--runtime external` only connects to a pre-existing protected IPv4-loopback endpoint. vLLM and SGLang are candidate implementations behind that API, not certified integrations: LAO does not install, probe, start, stop, or E2E-certify them yet.

To select a running vLLM or SGLang server without giving LAO ownership of it:

```sh
LAO_EXTERNAL_ADDR=127.0.0.1:8000 \
LAO_EXTERNAL_KEY_FILE=/absolute/path/to/owner-only/runtime.key \
lao install --runtime external
```
