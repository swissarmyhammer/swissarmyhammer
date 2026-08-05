---
assignees:
- claude-code
position_column: todo
position_ordinal: fc80
title: crates/mirdan/src/install.rs is too large for the review engine — duplication can never read it
---
# Problem

`crates/mirdan/src/install.rs` is never reviewed by the `duplication` validator. The review engine skips it every time, in its own words:

> ⚠️ 1 file(s) not reviewed — the rendered prompt would exceed the agent's prompt cap:
> - `crates/mirdan/src/install.rs` — 567352 rendered bytes, over the 476042-byte batch budget; not reviewed by: duplication (narrow the scope)

Observed on ^mawfv02: the implementer hit it on four self-review passes, and the formal `/review` of `0e63e1031~1..0e63e1031` reproduced it. `install.rs` was the largest file in that change (187 lines changed) and held the code the card was about, so the card's own subject went unreviewed for duplication.

# Why the engine's own remedy does not work

The skip message says "narrow the scope". That does not help here. The budget is per **(validator, file) pair** — one file's rendered block must fit on its own. A `review file` run limited to `crates/mirdan/src/install.rs` still renders 567352 bytes and still exceeds the 476042-byte cap. No scoping, filtering, or `batch_size` value makes a single oversized file fit.

Raising `batch_size` cannot fix it either: the batch budget is clamped down to the agent's prompt cap, so the cap is the real ceiling.

# Why this matters

The skip is reported, but it is easy to miss in a long review, and nothing fails. A file can sit permanently outside one validator's coverage while every review of it returns "clean" for that dimension. `install.rs` is the install/uninstall path for every CLI in the workspace — duplication there is exactly the defect class that rots.

This is also a silent-coverage problem in general: any file that grows past the cap drops out of a validator's reach with no gate failing.

# Fix

Split `crates/mirdan/src/install.rs` so no single file's rendered block exceeds the prompt cap. It is one file carrying several concerns — component installation, profile application, MCP config writing, skills/agents deployment, and their test modules — which is why it reached this size.

Related: ^927239f (`mirdan install.rs cleanup: constants, dispatch dedup, nesting`) already proposes cleanup in this file. Splitting it may subsume or reshape that card — read it before starting.

Consider also whether the engine should treat "a file no validator could read" as a harder signal than a warning line in the report, since today a permanently-unreviewable file is indistinguishable from a clean one at a glance.

# Acceptance

- `crates/mirdan/src/install.rs` no longer appears in any review's "not reviewed — would exceed the agent's prompt cap" list.
- A `review file` run against every resulting file completes with none skipped.
- The split preserves behaviour: `cargo nextest run --workspace` green, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo fmt --all -- --check` clean.
- `sah init` / `sah deinit` and the `kanban` / `code-context` / `shelltool` equivalents still round-trip against a real isolated `$HOME`. #bug #review