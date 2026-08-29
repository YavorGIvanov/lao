# Agent guide

This file applies to the whole repository. Read it before acting and reread it before finishing. When editing this file, follow its own rules: keep only instructions that change behavior.

## Read first

1. [README.md](README.md) — product summary and manifesto. Always read the manifesto.
2. [architecture.html](architecture.html) — human map from the system to folders and packages. Keep it current.
3. [PRODUCT_VISION_AND_ARCHITECTURE.md](PRODUCT_VISION_AND_ARCHITECTURE.md) — product, trust, privacy, and architecture decisions.
4. [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) — task order, contracts, acceptance criteria, and current evidence.

Read only the relevant parts after the manifesto. Do not duplicate these documents here.

## Work small

- Understand the intent and root cause before editing.
- State the goal, non-goals, acceptance criteria, and untouched scope.
- Choose the minimal sufficient solution.
- Prefer deleting or reusing code to adding abstractions.
- Add no speculative framework, compatibility track, config layer, or future feature.
- Keep names, code, comments, dependencies, and diffs small.
- Work in one task first. Delegate only a bounded independent problem when it clearly helps.
- Ask before destructive or irreversible actions.

## Preserve the product

- Keep Codex and Claude Code as the user's harnesses.
- Keep cloud as the safe default until local quality and fit are proven.
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

## Finish

- Review the actual diff for scope, secrets, debug residue, and needless code.
- For security or architecture changes, perform a focused adversarial review.
- Always perform a final simplification pass.
- Keep README status, the architecture page, product vision, and implementation plan consistent when their claims change.
- Run proportionate formatting, tests, lint, architecture checks, and extraction checks.
- Stop when acceptance is met.

Git author and committer must always be `YavorGIvanov <yavorgenadiev@gmail.com>`. Never add Codex as author or coauthor.
