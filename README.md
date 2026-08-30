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

This is a research proof for supported Apple Silicon Macs. It transactionally changes the user settings for both Codex and Claude Code. It does not read or copy either harness's credential store, and `lao off` restores the original settings exactly.

Clone the project and install the `lao` command:

```sh
git clone https://github.com/YavorGIvanov/lao.git && cd lao && ./install.sh
```

Then run the complete setup:

```sh
lao install
```

That command detects the installed clients and machine, downloads and verifies the supported local runtime and model, applies the client settings transactionally, and starts the service. There are no separate runtime packages or prerequisite checks to run. Unsupported configurations stop safely without overwriting existing settings, and a partial install rolls back automatically.

Setup stops with a specific error when a prerequisite or existing configuration is unsupported. It verifies or downloads the immutable Qwen inference model and MiniLM router model (1,208,656,003 bytes total), so allow about 1.21 GB of network traffic and cache space on the first run.

## Optional end-to-end test

Installation must end with `installed: Codex and Claude now use the launchd-owned LAO gate`. An already-running Codex process does not reload the new settings. Move to the existing work repository and launch a new process:

```sh
cd /absolute/path/to/your/existing-project
env -u LAO_LOCAL_SELECTOR codex
```

This is the intended cloud-safe experience: continue using Codex normally in the work repository. A separate local MiniLM model classifies bounded prompts; only a result that passes the conservative prototype threshold uses local inference, while uncertainty, failure, and complex work stay on the saved-login cloud path.

First prove that a request the policy must keep in Cloud still uses the saved-login path. This consumes one normal Codex turn:

```sh
env -u LAO_LOCAL_SELECTOR codex -c 'model_reasoning_effort="low"' \
  -c 'mcp_servers={}' -c 'web_search="disabled"' exec \
  --strict-config --ephemeral --skip-git-repo-check \
  --color never --sandbox read-only --model gpt-5.4 \
  'Review a complex production security migration without changing anything. Reply exactly CODEX_CLOUD_E2E_OK. Do not use tools.'
```

The final line must be `CODEX_CLOUD_E2E_OK`. The risk veto keeps both MiniLM and the llama.cpp/Qwen worker unloaded for this command.

Then run the explicit, bounded local canary through Codex:

```sh
LAO_LOCAL_SELECTOR=canary codex -c 'model_reasoning_effort="low"' \
  -c 'mcp_servers={}' -c 'web_search="disabled"' exec \
  --strict-config --ephemeral --skip-git-repo-check \
  --color never --sandbox read-only --model lao-local \
  'Reply exactly 42. Do not use tools.'
```

The final line must be `42`. Keep `LAO_LOCAL_SELECTOR=canary` inline exactly as shown; never `export` it. The first local turn takes roughly 24 seconds on the tested Mac and starts an approximately 2.05 GiB worker. The accepted Stage 1 check uses Codex's non-interactive `exec` mode; the interactive TUI is not yet claimed as tested.

The default router also handles one narrow eligible first-turn text request without the canary. This real E2E must return `the`; pinned MiniLM classifies the prompt and the verified Qwen model answers it:

```sh
env -u LAO_LOCAL_SELECTOR codex -c 'model_reasoning_effort="low"' \
  -c 'mcp_servers={}' -c 'web_search="disabled"' exec \
  --strict-config --ephemeral --skip-git-repo-check \
  --color never --sandbox read-only --model gpt-5.4 \
  'Correct the spelling error in this one word: teh. Reply with only the corrected word.'
```

Automatic Local is limited to at most 4,096 bytes of current-user text and no prior response or tool output. Codex input is either one string or first-turn developer/user message items ending in user; Claude has exactly one user message. LAO sends only the final user text to Local and disables tools. Every other non-canary request stays Cloud.

To repeat the local canary through both installed harnesses with sanitized pass/fail output:

```sh
lao smoke
```

When finished—or immediately if a later check fails—restore both clients and stop LAO from any directory:

```sh
lao off
```

Do not edit the managed Codex or Claude settings between `install` and `off`; the transaction refuses to overwrite unexpected user changes. A successful `off` reports exact restoration and leaves no daemon, worker, listener, plist, runtime key, or log.

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

## Contributor workflow

1. Read the product architecture and implementation plan below.
2. Run `cargo xtask check`, `cargo test --workspace`, and `cargo xtask extract` to verify the boundary baseline.
3. Keep cloud as the default and preserve the exact installed proof above.
4. Leave capture, eval, optimization, training, and extra backends disabled behind their APIs until the plan activates them.

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

The clean R2 install also routed the no-canary spelling request through the launchd-owned default path in both harnesses: Codex returned `the` in 3.79 seconds from the cold semantic/runtime state, and Claude returned `the` in 3.85 seconds with the worker warm. The forced-Cloud Codex proof completed in 2.67 seconds with llama.cpp absent. Measured daemon RSS was about 8 MiB before MiniLM loaded and 141 MiB afterward; the loaded llama.cpp worker was about 2.02 GiB. These are single proof measurements, not benchmarks.

`lao install` generates separate 256-bit caller keys, installs the daemon in owner-only product state, verifies launchd adoption and the exact inert hello before either client write, and applies the supported settings under one owner-only lock and crash record. The Qwen runtime starts only after an explicit canary or an automatic request passes the prototype threshold. Its measured restart-run peak was about 2.05 GiB RSS under the 6 GiB Light ceiling. `lao off` restored the original client bytes and permissions and left no daemon, worker, listener, plist, runtime key, or log. This remains a research proof rather than a supported release: signed packaging, interactive harness surfaces, API-key E2Es, and broader adapters remain future work.

The first post-Stage 1 resource slice is implemented: each local response holds a runtime lease until its stream completes or is dropped. After the final lease ends, a five-second watcher unloads the worker after roughly five observed idle minutes or as soon as it observes macOS memory pressure; a pressure-probe error also unloads safely. A later local request cold-starts the verified runtime again. Cloud traffic never acquires a lease, and background preload remains deferred.

The default `lao install` selection is `--router semantic --runtime llama-cpp`. `--router safe` keeps automatic work in Cloud, while `--router vllm-semantic` uses a user-managed vLLM Semantic Router decision endpoint. `--runtime external` only connects to a pre-existing protected IPv4-loopback endpoint. vLLM and SGLang are candidate implementations behind that API, not certified integrations: LAO does not install, probe, start, stop, or E2E-certify them yet.

To select a running vLLM or SGLang server without giving LAO ownership of it:

```sh
LAO_EXTERNAL_ADDR=127.0.0.1:8000 \
LAO_EXTERNAL_KEY_FILE=/absolute/path/to/owner-only/runtime.key \
lao install --runtime external
```
