---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9f80
title: 'validator findings: crates/claude-agent/src/lib.rs (pre-existing)'
---
Pre-existing validator findings surfaced by `review file crates/claude-agent/src/lib.rs` on 2026-08-08. None of these lines belong to card ^811xj0q (warm prefix reuse telemetry); that card only changed line 238, swapping the literal `"cache_usage"` for `CacheUsage::META_KEY`. Filed separately so ^811xj0q is not blocked by code it never touched.

## Review Findings (2026-08-08 05:20)

- [ ] `crates/claude-agent/src/lib.rs:1` — Crate-level documentation must include a code example showing common use cases; the module-level doc comment (lines 1–5) has none. Add a # Example section to the crate-level documentation showing the typical usage pattern: creating an agent with `create_agent()` and executing a prompt with `execute_prompt()`.
- [ ] `crates/claude-agent/src/lib.rs:11` — missing documentation for a module.
- [ ] `crates/claude-agent/src/lib.rs:25` — missing documentation for a module.
- [ ] `crates/claude-agent/src/lib.rs:33` — missing documentation for a module.
- [ ] `crates/claude-agent/src/lib.rs:35` — missing documentation for a module.
- [ ] `crates/claude-agent/src/lib.rs:40` — missing documentation for a module.
- [ ] `crates/claude-agent/src/lib.rs:47` — missing documentation for a module.
- [ ] `crates/claude-agent/src/lib.rs:53` — Test module is commented out rather than deleted. When tests are no longer valid or relevant (because the code being tested was deleted), they should be removed entirely per rule guidance, not left as commented-out code. Delete lines 52–53 entirely. If there is doubt about permanent removal, move the tests to a dedicated pending or WIP file and mark them clearly, rather than leaving them commented out in production code.
- [ ] `crates/claude-agent/src/lib.rs:60` — missing documentation for a module.
- [ ] `crates/claude-agent/src/lib.rs:90` — Function name `todowrite_to_acp_plan` uses an unusual prefix `todowrite_` that deviates from Rust naming conventions and the established patterns in this file. Standard Rust conversion functions use names like `to_acp_plan`, `as_acp_plan`, or `convert_to_acp_plan`. Rename to `to_acp_plan` or `convert_to_acp_plan` to match standard Rust naming conventions for conversion functions.
- [ ] `crates/claude-agent/src/lib.rs:90` — Function name `todowrite_to_agent_plan` uses an unusual prefix `todowrite_` that deviates from Rust naming conventions and the established patterns in this file. Standard Rust conversion functions use names like `to_agent_plan`, `as_agent_plan`, or `convert_to_agent_plan`. Rename to `to_agent_plan` or `convert_to_agent_plan` to match standard Rust naming conventions for conversion functions.
- [ ] `crates/claude-agent/src/lib.rs:346` — Three getter methods (notification_count, matched_count, skipped) with nearly identical implementations — each loads an atomic value with Relaxed ordering and returns it. They differ only in field name and return type, making them one function with parameterized arguments. Duplication of this pattern inflates the surface that maintenance must touch; if the ordering or load pattern ever needs to change, all three must be updated. Extract to a macro — e.g., `macro_rules! atomic_getter { ($name:ident, $field:ident, $type:ty) => { #[must_use] pub fn $name(&self) -> $type { self.$field.load(std::sync::atomic::Ordering::Relaxed) } } }` — then invoke it once per field. This eliminates duplication and makes the pattern explicit.