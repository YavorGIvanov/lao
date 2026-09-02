# Local Agent Optimizer

Local Agent Optimizer is a working title for an open-source, nearly invisible layer between coding-agent clients and local or cloud models.

The product is intended to let people continue using Codex and Claude Code normally while it:

- discovers what local model can run comfortably on their hardware;
- routes a conservative subset of bounded tasks to that model;
- preserves the native cloud path for difficult or risky work;
- captures valuable, reproducible task evidence locally with consent;
- evaluates new models against the user's own work; and
- eventually supports explicitly authorized local-model personalization.

This repository contains the research-backed product specification, implementation plan, architecture skeleton, and a working Apple Silicon proof. Codex or Claude can delegate a bounded work packet to LAO; the real semantic router kept the tested broad packet in Cloud and sent the tested narrow packet to OpenCode running Qwen3 locally. It does not yet contain production routing certification or release packaging.

## Install

This is a research proof for supported Apple Silicon Macs. It transactionally changes the user settings for both Codex and Claude Code. It does not read or copy either harness's credential store. `lao off` restores unchanged settings exactly and preserves unrelated settings the clients add while LAO is installed; from Claude's mutable global state, it removes only LAO's entry.

Clone the project, install the command, and let LAO finish setup:

```sh
git clone https://github.com/YavorGIvanov/lao.git && cd lao && ./install.sh && lao install
```

That is the whole setup. `lao install` detects the clients and machine, downloads and verifies the supported runtime and models, applies the client settings transactionally, starts the service, and warms the local path in the background. There are no separate runtime packages, model servers, versions, or prerequisite checks to manage. Unsupported configurations stop safely without overwriting existing settings, and a partial install rolls back automatically.

Setup stops with a specific error when a prerequisite or existing configuration is unsupported. The verified Qwen3 model, MiniLM router, llama.cpp runtime, and OpenCode archive total 2,645,805,392 bytes, so allow about 2.7 GB on the first run. OpenCode's pinned support tree is capped at 80 MiB and accepted only when its lockfile and complete tree match the compiled SHA-256 digests.

## Normal use

Keep using `codex` or `claude`. The cloud harness remains the planner and can call LAO's `execute` tool for a bounded implementation packet. On the tested Codex and Claude Code versions, an ordinary eligible edit is delegated without mentioning LAO. LAO routes each packet independently:

```text
Codex / Claude planner
        ↓ one bounded packet
LAO semantic router
   ├─ Cloud → current harness continues
   └─ Local → OpenCode → Qwen3 / llama.cpp
```

The default is conservative: planning, broad changes, and uncertain work stay Cloud. A Local packet may read and edit only the paths named by the planner. OpenCode keeps the local tool loop coherent; the cloud harness reviews the result and runs verification. The installed settings auto-approve only `lao.execute`, so this path does not ask for repeated MCP confirmations or grant general command access.

Running the clone command and `lao install` again is safe. A healthy existing setup is verified and reused without downloading again, replacing its keys, or rewriting client settings.

At any time, check the whole installed path without consuming a model request:

```sh
lao status
```

`LAO: ready` means the service is running and both clients are routed through LAO. `local cache: warming` becomes `local cache: ready` after the background canaries finish. Ordinary work is available immediately on the cloud-safe path while warming continues. No credential or configuration value is printed.

## Test the experience

Open a new terminal in the repository where you already work and start Codex or Claude Code:

```sh
cd /absolute/path/to/your/existing-project
codex
# or: claude
```

Use either harness normally. In both tested clients, a broad planning request stayed Cloud and an ordinary one-file correction routed through OpenCode and Qwen3. Uncertainty, unsupported work, and classifier failures stay Cloud; routing quality beyond these conservative cases is not yet certified.

To check both installed harnesses and the local model with sanitized pass/fail output:

```sh
lao smoke
```

It prints only pass/fail and elapsed time. A warmed local canary completes in roughly two to four seconds on the tested Mac. Run `lao status` first if you want to see whether background warming has finished; it does not consume a model request.

When finished—or immediately if a later check fails—restore both clients and stop LAO from any directory:

```sh
lao off
```

Codex and Claude may update their own unrelated settings while LAO is installed; `lao smoke` accepts those updates and `lao off` preserves them. LAO still refuses changes to the routing entries it owns. A successful `off` removes LAO's client changes and leaves no daemon, runtime process, listener, plist, optimizer state, runtime key, or log.

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

The current routed-worker proof used real Codex and Claude Code cloud turns, not synthetic planners. Given only a normal spelling-fix request, each harness called `lao.execute` once without an approval prompt; MiniLM selected Local; OpenCode 1.18.25 used Qwen3 4B through llama.cpp to change only `word.txt`; and the parent harness independently verified the result. Broad planning controls remained in their cloud harnesses. The measured delegated runs took about 35 seconds in Codex and 26 seconds in Claude Code; the loaded llama.cpp worker measured about 4.70 GiB RSS. These are single proof measurements, not benchmarks.

`lao install` generates separate caller keys, verifies launchd before either client write, and applies both client settings as one recoverable transaction. The optimizer then warms fixed local Claude and Codex paths in the background. The current Qwen3 runtime remains below the 6 GiB Light ceiling at its 16K context. `lao off` removes the managed changes and service state while preserving unrelated client state. This remains a research proof rather than a supported release: signed packaging, interactive harness surfaces, API-key E2Es, and broader adapters remain future work.

Each local response holds a runtime lease until its stream completes or is dropped. The healthy worker and both harness prefixes remain in a bounded RAM cache; a five-second watcher unloads them on idle memory pressure. Repeated `lao install`, `lao status`, and same-revision source setup are measured below 100 ms; gateway p95 overhead is below 3.5 ms. Model generation is reported separately.

The default `lao install` selection is `--router semantic --runtime llama-cpp`. `--router safe` keeps automatic work in Cloud, while `--router vllm-semantic` uses a user-managed vLLM Semantic Router decision endpoint. `--runtime external` only connects to a pre-existing protected IPv4-loopback endpoint. vLLM and SGLang are candidate implementations behind that API, not certified integrations: LAO does not install, probe, start, stop, or E2E-certify them yet.

To select a running vLLM or SGLang server without giving LAO ownership of it:

```sh
LAO_EXTERNAL_ADDR=127.0.0.1:8000 \
LAO_EXTERNAL_KEY_FILE=/absolute/path/to/owner-only/runtime.key \
lao install --runtime external
```
