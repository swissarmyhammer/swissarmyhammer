---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
project: local-review
title: 'Design: review fleet architecture, validator catalog & finding model'
---
## What
Keystone design task. Produce a written design (`ideas/local-review.md`) that pins the architecture every downstream task builds against. No production code; the artifact is the design doc + the locked decisions below.

Locked decisions (from planning + follow-up directives):
- **Engine crate**: rename `avp-common` → **`swissarmyhammer-validators`** and make it the SHARED ENGINE for the pluggable review system. It owns: the rules-as-data loader (hook-free), the finding data model, the probe registry, and the fleet/verify/synthesis orchestration, plus the reusable ACP agent-execution primitive (the bare parallel/AIMD executor extracted from today's `runner.rs`, minus hook coupling).
- **The validator IS the shard; the file is the grain.** No "dimension" concept. The fleet runs ONE agent per `(validator, file)` pair. Each validator is authored as a single focused concern (duplication, dead-code, data-driven, security, …). Three nouns only: **validator** (pluggable on-disk unit = one agent's job per file), **rule** (one check inside a validator), **finding** (one confirmed issue, tagged with its source validator).
- **Validators are data-driven, declaring their own agent behavior** in frontmatter: `name`, `description` (the agent's mandate), `match.files` (globs), `severity`, and **`probes`** — names from the engine probe catalog the engine runs and injects as evidence. No `trigger` field (nothing hooks).
- **Probes** are engine-run structural facts about a changed entity, NOT agent tool-call instructions. Catalog: `callers` (inbound callgraph of an added symbol — fact), `duplicates` (near-identical blocks for a changed block — fact), `similar` (semantic `search code` on an added function body → reuse candidates — candidate evidence). Fact-probes power a deterministic auto-refute guard in verify; `search_symbol`-by-name and `blastradius`-as-a-check are NOT probes (incoherent / context-only).
- **Tool** (operation-based, verb-noun, thin wrapper over the engine): the verb `review` dispatches on the scope NOUN — `review file` (path or glob), `review working` (uncommitted vs HEAD), `review sha` (commit/range); shared modifiers `validators?[]`, `backend?`. Plus `list validators`, `get validator`, `check validators`. No `install` op (installation is initialization). No `list dimensions`.
- **Backend**: reuse `ConnectionTo<Agent>`. Default the session agent; allow a flag to force local Llama for fully-offline review.
- **Retire AVP entirely**: delete `apps/avp-cli`; remove the hook-execution machinery (chain links, hook context, turn-diff sidecars) KEEPING the loader + the bare ACP executor; remove the AVP validator doctor rule (`check_avp_directory` in `crates/mirdan/src/doctor.rs`). All rules — including safety — run only via on-demand review; the real-time block on dangerous commands/secret commits is intentionally dropped.
- **Directories**: validators load from builtin (embedded) → `~/.validators` (user) → `./.validators` (project), precedence in that order. Builtin set is materialized to disk on init.

Decide and document in the doc:
1. **Validator catalog** — the focused validators that replace the current `review` SKILL.md 7 layers + the existing `builtin/validators/*/rules/*.md`. A table mapping every current review layer and every existing rule to exactly one validator, each with its `match.files`, `severity`, and `probes`.
2. **Finding data model** — `{ file, line, validator, severity (blocker|warning|nit), claim, evidence, suggestion }`.
3. **Probe catalog** — the fixed set (`callers`/`duplicates`/`similar`), each as subject (entity from the semantic diff) → code_context op → fact/candidate → which validator consumes it, and which are deterministic (guard-able).
4. **Tool op surface** — `review file`/`review working`/`review sha` + `list/get/check validators`, with exact arg/return shapes; mirror the git tool's op-dispatch shape (`crates/swissarmyhammer-tools/src/mcp/tools/git/mod.rs`).
5. **Agent context payload** — the exact per-agent prompt: [change purpose] + [validator mandate + rules + output contract] + [one file: path, structured diff, bounded `source_slice` (header + changed entities + window, NOT whole file), probe evidence]. Plus the file-grain + batching policy.
6. **Fleet + verify flow** — stages: scope (noun → per-validator per-file work-list with probe results) → fan out (one agent per validator×file, batched) → verify (deterministic probe guard, then adversarial refute) → synthesize → kanban.
7. **Migration & teardown map** — which avp-common modules survive into `swissarmyhammer-validators`, which die with the hooks, where the safety rule markdown lands as validators, and the full `avp-common` dependent list to update on rename (`agent-client-protocol-extras`, `apps/swissarmyhammer-cli`, root `Cargo.toml`; `apps/avp-cli` is deleted).

## Acceptance Criteria
- [ ] `ideas/local-review.md` exists with all seven sections filled in concretely; the validator catalog table maps every current layer + existing rule to one focused validator with its `probes`.
- [ ] No "dimension" concept anywhere; the validator is the shard and the file is the grain.
- [ ] The probe catalog lists only coherent probes (subject + question + answer derivable from the diff); the agent context payload and the verb-noun op surface are pinned.
- [ ] The doc names the exact existing files/types each stage reuses and each teardown item deletes (paths resolve).
- [ ] Downstream board tasks reference the doc's decisions where previously TBD.

## Tests
- [ ] No automated tests — design-only task. Verification is review of the doc against the locked decisions and the existing code it references (paths must resolve).

## Workflow
- Research-and-write task. Read `builtin/skills/review/SKILL.md`, `crates/avp-common/src/validator/{loader,runner,types}.rs`, the chain/hook links, `apps/avp-cli/`, `crates/mirdan/src/doctor.rs`, and the git/code_context tool mods before writing. Do NOT use `/tdd` (no code).