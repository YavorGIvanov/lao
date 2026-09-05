# Agent guide

This file applies to the whole repository. Read it before acting and reread it before finishing. When editing this file, follow its own rules: keep only instructions that change behavior.

## Read first

1. [README.md](README.md) — product summary and manifesto. Always read the manifesto.
2. [architecture.html](architecture.html) — human map from the system to folders and packages. Keep it current.
3. [PRODUCT_VISION_AND_ARCHITECTURE.md](PRODUCT_VISION_AND_ARCHITECTURE.md) — product, trust, privacy, and architecture decisions.
4. [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) — task order, contracts, acceptance criteria, and current evidence.

Read only the relevant parts after the manifesto. The implementation plan separates current evidence from future intent; do not implement the backlog unless the task calls for it. Do not duplicate these documents here.

## Follow through

- Treat a request for action as authorization to carry it through implementation, verification, and handoff. Use context to resolve routine choices; ask only when a missing answer materially changes the result.
- Preserve the original task when the user adds requirements or asks for status. Answer briefly and continue the authorized work.
- User instructions take precedence over repository and skill guidelines. If a skill blocks work, link the exact file, quote the instruction, and explain why it applies; do not invent an approval requirement.
- Ask before destructive or irreversible actions unless already authorized. Finish the safe preparation first so the user can review a concrete result; never ask again for existing authorization.

## Work small

- Understand the intent and root cause before editing.
- Define the intended outcome, boundaries, and acceptance criteria before editing. Keep progress updates brief and focused on findings.
- Delete unnecessary behavior before simplifying it; optimize or automate only a measured need. A review may end with no changes.
- Prefer deleting or reusing code to adding abstractions.
- Add no speculative framework, compatibility track, config layer, or future feature.
- Keep names, code, comments, dependencies, and diffs small.
- Keep broad planning and security review in the parent harness. Delegate only a bounded independent task with exact writable paths when it saves time or improves evidence; review its diff and verify even when the worker reports failure.

## Preserve the product

- Keep Codex and Claude Code as the user's harnesses.
- Keep cloud as the safe default until local quality and fit are proven. Each local packet gets fresh disposable state and exact file permissions; never resume an earlier packet or treat permission patterns as filenames.
- Never consume the whole machine.
- Do not read or copy harness-owned provider credentials.
- Require explicit consent for capture, spend, cloud evaluation, or training.
- Keep components independent: APIs between them, private state within them, and no sibling implementation imports.
- Reuse proven components for the PoC behind seams we can own later.

## Test only what changed

- Run the closest existing tests first.
- Add a test only when changed behavior lacks evidence or the user requires it.
- Prefer a cheap real end-to-end path over a mock when it is safe and trivial.
- Otherwise add at most one main case and one essential failure case.
- Add no test framework, broad matrix, snapshot system, or unrelated coverage.
- A test must name the accepted requirement it protects.
- If the test is more complex than the behavior, simplify both.
- Never expose credentials, private data, or raw client output.
- After required checks pass, repeat or broaden them only for new changes, failures, or unresolved concerns. Leave installed-client and cloud-spending tests opt-in.

## Finish

- Review the actual diff for scope, secrets, debug residue, and needless code.
- For security or architecture changes, perform a focused adversarial review.
- Always perform a final simplification pass.
- Keep README status, the architecture page, product vision, and implementation plan consistent when their claims change.
- Run `cargo fmt --all -- --check` and `git diff --check`. For Rust changes run focused tests and strict Clippy; for API or dependency changes also run workspace tests, `cargo xtask check`, and `cargo xtask extract`. Limit builds to two jobs on the test Mac.
- Report the result, verification, and material limitations in plain language. Distinguish current evidence from historical measurements. Stop when acceptance is met.

Git author and committer must always be `YavorGIvanov <yavorgenadiev@gmail.com>`. Never add Codex as author or coauthor.
