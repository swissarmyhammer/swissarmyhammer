---
assignees:
- claude-code
position_column: todo
position_ordinal: a480
project: diagnostics
title: 'Create swissarmyhammer-diagnostics crate: report types, config, lsp_types mapping'
---
## What
New crate `swissarmyhammer-diagnostics` holding the model-free core: report types, config, the `lsp_types::Diagnostic → report record` mapping, and the shared diagnosable-language predicate. It owns NO client — it sits on the shared session in `swissarmyhammer-lsp`. It is a crate (not a module) because it has two consumers: the `diagnostics` MCP tool and the inline-on-edit fold-in, and belongs to neither.

- Create `crates/swissarmyhammer-diagnostics` and add to the workspace `Cargo.toml` members.
- Types: `DiagnosticsReport { diagnostics: Vec<DiagnosticRecord>, counts: Counts }`, `DiagnosticRecord { path, range, severity, message, code, source, containing_symbol }`, `Counts { errors, warnings }`.
- **Severity:** code-context already defines `pub enum DiagnosticSeverity { Error, Warning, Info, Hint }` with `from_lsp`/`to_lsp` at `crates/swissarmyhammer-code-context/src/ops/get_diagnostics.rs:23`. Make ONE canonical `Severity` in this crate (relocate/rename that existing enum, or re-export it) and have code-context use the canonical one — do NOT leave two competing severity enums. Map from `lsp_types::DiagnosticSeverity` via the existing `from_lsp` logic.
- `map(lsp_types::Diagnostic, path) -> DiagnosticRecord` — pure mapping (port the table from code-context `ops/get_diagnostics.rs` parsing + its tests).
- **`is_diagnosable(path) -> bool`** — the single shared language gate, backed by the LSP supervisor's server-spec `file_extensions` (the supervisor in `swissarmyhammer-lsp` knows which languages have a server). Both consumers (the `diagnostics` tool and the inline-on-edit fold-in s15vmdw) call this ONE helper so `.md`/`.txt` exclusion is defined and tested in exactly one place, not reimplemented twice.
- `DiagnosticsConfig { severities, settle_window, per_report_cap, per_language_enabled }` with defaults: errors+warnings, short settle, capped, all detected languages. **No persistence** (config + any cache are derived state, never written to disk).
- Depends on `swissarmyhammer-lsp` (session/supervisor/spec types) and `lsp-types`.

## Depends on
- "Invert lsp↔code-context dependency..." (b3ahkva) — the shared crate home/types must exist. (Type/config work here can otherwise proceed in parallel with the session tasks.)

## Acceptance Criteria
- [ ] `swissarmyhammer-diagnostics` exists in the workspace, depends on `swissarmyhammer-lsp`, owns no LSP client.
- [ ] ONE canonical `Severity` enum (the existing `DiagnosticSeverity` relocated/renamed); code-context references it — no duplicate severity type remains.
- [ ] Pure `map()` from `lsp_types::Diagnostic` to `DiagnosticRecord`.
- [ ] `is_diagnosable(path)` helper backed by the supervisor's spec `file_extensions`; `.rs` true, `.md`/`.txt` false.
- [ ] Report/record/config types serializable; nothing persisted.

## Tests
- [ ] `cargo test -p swissarmyhammer-diagnostics`: mapping tests (port the `DiagnosticSeverity`/code/source/severity table from code-context `ops/get_diagnostics.rs` tests); `is_diagnosable` truth table (`.rs` vs `.md`/`.txt`); config-default tests; report serde round-trip. All model-free, <1s.

## Workflow
- Use `/tdd` — write the mapping + `is_diagnosable` + config tests first. #diagnostics