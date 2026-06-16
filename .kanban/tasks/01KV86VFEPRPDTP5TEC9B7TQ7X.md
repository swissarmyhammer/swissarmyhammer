---
assignees:
- claude-code
position_column: todo
position_ordinal: a480
project: diagnostics
title: 'Create swissarmyhammer-diagnostics crate: report types, config, lsp_types mapping'
---
## What
New crate `swissarmyhammer-diagnostics` holding the model-free core: report types, config, and the `lsp_types::Diagnostic → report record` mapping. It owns NO client — it sits on the shared session in `swissarmyhammer-lsp`. It is a crate (not a module) because it has two consumers: the `diagnostics` MCP tool and the `files edit` op, and belongs to neither.

- Create `crates/swissarmyhammer-diagnostics` and add to the workspace `Cargo.toml` members.
- Types: `DiagnosticsReport { diagnostics: Vec<DiagnosticRecord>, counts: Counts }`, `DiagnosticRecord { path, range, severity, message, code, source, containing_symbol }`, `Severity { Error|Warning|Info|Hint }` (reuse/relocate the enum from code-context `ops/get_diagnostics.rs` so there is one severity type). `Counts { errors, warnings }`.
- `map(lsp_types::Diagnostic, path) -> DiagnosticRecord` — pure mapping.
- `DiagnosticsConfig { severities, settle_window, per_report_cap, per_language_enabled }` with defaults: errors+warnings, short settle, capped, all detected languages. **No persistence.**
- Depends on `swissarmyhammer-lsp` (for session/diagnostic types) and `lsp-types`.

## Depends on
- "Invert lsp↔code-context dependency..." (b3ahkva) — the shared crate home/types must exist. (Type/config work here can otherwise proceed in parallel with the session tasks.)

## Acceptance Criteria
- [ ] `swissarmyhammer-diagnostics` exists in the workspace, depends on `swissarmyhammer-lsp`, owns no LSP client.
- [ ] Report/record/severity/config types defined and serializable; one canonical `Severity` enum (not duplicated in code-context).
- [ ] Pure `map()` from `lsp_types::Diagnostic` to `DiagnosticRecord`.

## Tests
- [ ] `cargo test -p swissarmyhammer-diagnostics`: mapping tests (lsp_types::Diagnostic → DiagnosticRecord incl. code/source/severity edge cases — port the table from code-context `ops/get_diagnostics.rs` tests), config-default tests, report serde round-trip. All model-free, <1s.

## Workflow
- Use `/tdd` — write the mapping/config tests first. #diagnostics