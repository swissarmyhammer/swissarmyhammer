---
assignees:
- claude-code
depends_on:
- 01KTBN925WPAWDYXS12W5HETEH
position_column: todo
position_ordinal: '9280'
project: local-review
title: 'Engine: probe registry — engine-run code_context probes bound from the diff'
---
## What
The mechanism that makes a validator's `probes` real. A **probe** is a code_context operation the ENGINE runs on the agent's behalf and injects as ground-truth evidence — never a "please call this tool" instruction the agent can skip (the exact failure mode of today's review). Build the catalog + runner in `swissarmyhammer-validators::review`.

**Catalog (exactly these three — coherent: subject + question both derivable from the diff):**
| Probe name | code_context op | Subject (from semantic diff) | Returns | Kind |
|------------|-----------------|------------------------------|---------|------|
| `callers` | `get callgraph` (inbound) | each **added** symbol | inbound call sites (or none) | **fact** (guard-able) |
| `duplicates` | `find duplicates` | each changed file, filtered to the changed blocks | near-identical blocks elsewhere + similarity | **fact** (guard-able) |
| `similar` | `search code` (semantic) | each **added function body**, self excluded | semantically-similar existing code (reuse candidates) | **candidate** (agent-interpreted) |

Dropped as incoherent: `search_symbol` (searching by an added symbol's own name returns the new symbol, not a reuse target) and `get_blastradius` (returns context, not a checkable fact — it stays a tool the correctness agent MAY call, not a probe).

- Validators declare probes by these **semantic names** (`probes: [callers]`, etc.), not op names. Each catalog entry records its **kind** (`fact` vs `candidate`) so the verify guard knows which probes can deterministically refute (only `fact` probes) and which only inform (`candidate`).
- Derive probe arguments from the git semantic diff's changed entities (`ChangeEntry { entity_type, entity_name, file_path }` from the git `get diff` tool): added symbols for `callers`, changed files/blocks for `duplicates`, added function bodies for `similar`.
- **`duplicates` must cover the changed set, not only the index.** A HEAD-based code_context index won't contain another just-changed file, so the same block pasted into two new files would be missed. The `duplicates` probe is responsible for comparing changed blocks against each other (the working/changed set) in addition to the index.
- `run_probes(probe_names, file_change) -> ProbeResults` executes the named probes via the existing code_context tool as a library (do NOT reimplement duplicate/callgraph/search logic). Results are structured (per probe: name, kind, bound target, rows) so they render as evidence AND can be machine-checked by the verify guard.
- `similar` excludes the changed entity itself from results; semantic search returns top-k so cap and de-self.
- Probes are read-only, indexed, bounded (must not hang). Resolve code_context/CWD from the session/work-dir, never `current_dir()`.
- `probe_exists(name) -> bool` so `check validators` can validate a validator's declared probes against the catalog.

## Acceptance Criteria
- [ ] The catalog has exactly `callers`/`duplicates`/`similar`, each with op binding, arg-derivation, and a `fact`/`candidate` kind.
- [ ] `run_probes` derives args from the semantic diff's entities/files and returns structured results; `similar` excludes self.
- [ ] `duplicates` detects a block duplicated between two changed-but-unindexed files (changed-set comparison), not just index hits.
- [ ] Unknown probe name → clear error; `probe_exists` callable from the validator linter.
- [ ] Reuses the code_context tool as a library; no reimplemented duplicate/callgraph/search logic.

## Tests
- [ ] Integration test (real code_context index over a temp repo): a file adding a function duplicating an existing one → `duplicates` returns the hit; `callers` on the new uncalled symbol returns empty inbound; `similar` on a body that reimplements an existing util returns the util (and not itself).
- [ ] Changed-set dup test: same block added to two new files → `duplicates` flags it despite neither being indexed.
- [ ] `run_probes` with an unknown name errors; `probe_exists` returns false for it.
- [ ] `cargo test -p swissarmyhammer-validators review::probes` green.

## Workflow
- Use `/tdd` — assert real probe results over a temp-repo code_context index first, then implement the catalog + runner. The catalog is DATA (a table of entries), not a `match` arm per probe with copy-pasted call code — one code path parameterized by the entry. Depends on the rename (engine crate).