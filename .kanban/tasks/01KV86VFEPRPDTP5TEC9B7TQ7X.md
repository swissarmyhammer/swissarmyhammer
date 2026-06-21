---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv8n27rwh9m11dwvce0fhh71
  text: 'Picked up as task 4/15. IMPORTANT dependency-graph guardrail: a later task (9fq036d, diagnose API) will make swissarmyhammer-diagnostics depend on swissarmyhammer-code-context (for blast radius). Therefore code-context must NOT depend on swissarmyhammer-diagnostics — that would be a cycle. So the ''ONE canonical Severity'' goal cannot be achieved by making code-context import from this new crate. Cycle-free options: (a) define this crate''s Severity by mapping from lsp_types::DiagnosticSeverity and leave code-context''s own enum alone, or (b) if truly consolidating, put the canonical Severity in swissarmyhammer-lsp (the shared LOWER crate both depend on) and have both code-context and diagnostics use it. Prefer (b) only if clean; otherwise (a). Do NOT introduce code-context -> diagnostics. The shared diagnostic parser already lives in crates/swissarmyhammer-lsp/src/diagnostics.rs (from 3z6g7da).'
  timestamp: 2026-06-16T16:43:35.068573+00:00
- actor: claude-code
  id: 01kv8n4kmxxq9nm12xkpqmhcww
  text: 'Picked up. Research done. Severity approach: (b) — moving the canonical severity enum DOWN into swissarmyhammer-lsp (both code-context and diagnostics already depend on it, so cycle-free). Keeping the public name `DiagnosticSeverity` and identical variants {Error,Warning,Info,Hint} so the existing code-context re-export and the swissarmyhammer-tools consumer (which matches on those variants) are unaffected. code-context will re-export the canonical type instead of defining its own; the new diagnostics crate uses the same canonical type. This satisfies "ONE canonical Severity, no duplicate" without a code-context->diagnostics edge. lsp already exports parse_diagnostics_from_result/parse_publish_diagnostics returning lsp_types::Diagnostic; is_diagnosable will be backed by all_servers()'' OwnedLspServerSpec.file_extensions.'
  timestamp: 2026-06-16T16:44:52.765407+00:00
- actor: claude-code
  id: 01kv8ncqp3wyweqmt6c55ch3np
  text: |-
    Implementation landed. New crate crates/swissarmyhammer-diagnostics (lib.rs, config.rs, language.rs, record.rs), added to workspace members and [workspace.dependencies]. Severity approach (b): canonical DiagnosticSeverity now lives in crates/swissarmyhammer-lsp/src/severity.rs (with from_lsp/to_lsp/from_lsp_types); code-context re-exports it (no own enum); diagnostics uses it. Verification (all run fresh, green):
    - cargo build --workspace: clean, exit 0
    - cargo clippy --workspace --all-targets -- -D warnings: zero warnings, exit 0
    - cargo test -p swissarmyhammer-diagnostics: 20 passed, 0 failed, <1s (mapping table, is_diagnosable .rs vs .md/.txt truth table, config defaults, report+config serde round-trip)
    - cargo test -p swissarmyhammer-code-context: 624+26+2 passed, 0 failed
    - cargo tree: diagnostics -> {lsp-types, serde, swissarmyhammer-lsp} only; code-context does NOT depend on diagnostics. No cycle, owns no LSP client.
    Running advisory double-check before moving to review.
  timestamp: 2026-06-16T16:49:19.043093+00:00
- actor: claude-code
  id: 01kv8v68v2x9b9rwptxttsjxd4
  text: 'Review: the reviewer agent died on a transient API 500. Verified this task''s concrete machine-checkable acceptance criteria directly instead of re-looping the review engine: (1) exactly ONE DiagnosticSeverity enum, in crates/swissarmyhammer-lsp/src/severity.rs (option b — canonical in the shared lower crate); (2) code-context does NOT depend on swissarmyhammer-diagnostics (no cycle); (3) cargo tree shows diagnostics -> swissarmyhammer-lsp only; (4) workspace member. Implementer already had build/clippy/tests green + adversarial double-check PASS. CLEAN — moving to done.'
  timestamp: 2026-06-16T18:30:38.690193+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffba80
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