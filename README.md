# Local Agent Optimizer

Local Agent Optimizer is a working title for an open-source, nearly invisible layer between coding-agent clients and local or cloud models.

The product is intended to let people continue using Codex and Claude Code normally while it:

- discovers what local model can run comfortably on their hardware;
- routes a conservative subset of bounded tasks to that model;
- preserves the native cloud path for difficult or risky work;
- captures valuable, reproducible task evidence locally with consent;
- evaluates new models against the user's own work; and
- eventually supports explicitly authorized local-model personalization.

This repository contains the research-backed product specification, implementation plan, architecture skeleton, and complete Stage 1 installed proof. It does not yet contain production automatic routing or release packaging.

## Test Stage 1 on the supported Mac

This is a research proof for the 24 GiB Apple M4 Mac used by the project. It transactionally changes the user settings for both Codex and Claude Code. It does not read or copy either harness's credential store, and `lao off` restores the original settings exactly.

Use two separate directories: a fresh LAO clone that owns the build and installer, and an existing project where you normally work with Codex. In one terminal, set their absolute paths without exporting them:

```sh
LAO_CHECKOUT=/absolute/path/to/your/lao-clone
WORK_REPO=/absolute/path/to/your/existing-project
```

Before starting, confirm the exact tested tools and existing saved logins:

```sh
cargo --version
codex --version
codex login status
claude --version
command -v llama-server
llama-server --version
```

The tested versions are Rust/Cargo 1.98.0, Codex 0.146.0, Claude Code 2.1.251, and llama.cpp 10280 (`61881b1f7`). Codex must report its existing ChatGPT login, Claude Code must already be logged in, and `llama-server` must resolve to `/opt/homebrew/bin/llama-server`. Install verifies or downloads the immutable 1,117,320,768-byte Qwen model, so allow about 1.1 GB of network traffic and cache space on the first run.

Build both required binaries, review the proposed model and resource ceiling, then install:

```sh
cd "$LAO_CHECKOUT"
cargo build -p lao-cli -p lao-daemon
./target/debug/lao preview
./target/debug/lao install
```

Installation must end with `installed: Codex and Claude now use the launchd-owned LAO gate`. An already-running Codex process does not reload the new settings. Move to the existing work repository and launch a new process:

```sh
cd "$WORK_REPO"
env -u LAO_LOCAL_SELECTOR codex
```

This is the intended cloud-safe experience: continue using Codex normally in the work repository while model requests traverse LAO and stay on the saved-login cloud path. The interactive TUI is useful for evaluating setup feel, but it is not yet part of the accepted Stage 1 compatibility evidence.

First prove that an ordinary Codex request still uses the saved-login cloud path. This consumes one normal Codex turn:

```sh
env -u LAO_LOCAL_SELECTOR codex -c 'model_reasoning_effort="low"' \
  -c 'mcp_servers={}' -c 'web_search="disabled"' exec \
  --strict-config --ephemeral --skip-git-repo-check \
  --color never --sandbox read-only --model gpt-5.4 \
  'Reply exactly CODEX_CLOUD_E2E_OK. Do not use tools.'
```

The final line must be `CODEX_CLOUD_E2E_OK`. The local worker remains unloaded for this command.

Then run the explicit, bounded local canary through Codex:

```sh
LAO_LOCAL_SELECTOR=canary codex -c 'model_reasoning_effort="low"' \
  -c 'mcp_servers={}' -c 'web_search="disabled"' exec \
  --strict-config --ephemeral --skip-git-repo-check \
  --color never --sandbox read-only --model lao-local \
  'Reply exactly 42. Do not use tools.'
```

The final line must be `42`. Keep `LAO_LOCAL_SELECTOR=canary` inline exactly as shown; never `export` it. The first local turn takes roughly 24 seconds on the tested Mac and starts an approximately 2.05 GiB worker. The accepted Stage 1 check uses Codex's non-interactive `exec` mode; the interactive TUI is not yet claimed as tested.

To repeat the local canary through both installed harnesses with sanitized pass/fail output:

```sh
"$LAO_CHECKOUT/target/debug/lao" smoke
```

An install failure rolls back automatically. After a successful install, when finished—or immediately if a later check fails—restore both clients and stop LAO:

```sh
"$LAO_CHECKOUT/target/debug/lao" off
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

The cloud-safe baseline is complete: the private gate retains request bodies, headers, credentials, and targets, and gives the router only `Context(client, operation, canary)`. Normal contexts route to native cloud. Only an exact, non-secret canary selector may produce Local; the gate consumes it, removes native secrets, and binds the request to the runtime's loopback endpoint and private bearer.

Stage 1 is complete on the supported 24 GiB M4 Mac. Pinned llama.cpp 10280 serves native Responses and Messages HTTP/SSE, so this slice passes request bodies and response streams without a translation layer and exposes the model only as `lao-local`. Installed Codex 0.146.0 and Claude Code 2.1.251 each completed saved-login cloud requests and the same real local canary through one gate and router, including after a daemon restart.

`lao install` generates separate 256-bit caller keys, installs the daemon in owner-only product state, verifies launchd adoption and the exact inert hello before either client write, and applies the supported settings under one owner-only lock and crash record. The local runtime starts only for an explicit canary. Its measured restart-run peak was about 2.05 GiB RSS under the 6 GiB Light ceiling. `lao off` restored the original client bytes and permissions and left no daemon, worker, listener, plist, runtime key, or log. This remains a research proof rather than a supported release: signed packaging, interactive harness surfaces, API-key E2Es, and broader adapters remain future work.
