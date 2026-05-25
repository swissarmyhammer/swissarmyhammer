---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffaa80
title: Add tags to models; filter kanban AI panel selector by `kanban` tag
---
## What

Give model configs an optional `tags` list and use a `kanban` tag to control which models appear in the kanban app's AI panel model selector, so the built-in model set can grow without cluttering the panel.

## Final design (after in-review revisions)

The requirements were refined during review. Final behavior:

1. **`tags` on model configs** — `crates/swissarmyhammer-config/src/model.rs`:
   - `#[serde(default)] pub tags: Vec<String>` on `ModelInfo`, populated from YAML frontmatter at both construction sites (`load_builtin_models`, `validate_and_create_model_info`).
   - `pub fn parse_model_tags(content: &str) -> Vec<String>` + a list-aware `extract_yaml_frontmatter_list` helper (handles `tags: [a, b]` and block-sequence form; empty when no frontmatter/no key).

2. **AI panel selector** — `apps/kanban-app/src/ai/models.rs::ai_list_models`:
   - The `kanban` tag gates which models appear. Built-in `claude-code` is the only model tagged `kanban` for now, so the panel shows **only Claude Code**.
   - Local llama models are opt-in via the `kanban` tag (none tagged → none shown). Embedding executors stay excluded regardless of tag.
   - **Claude Code is always `available`** — the agent spawns the `claude` CLI at use time, so the entry is no longer gated on a `which(\"claude\")` probe (that probe fails in a GUI app with a stripped PATH even when `claude` works). CLI detection now only enriches the `hint` with the resolved path when found.

3. **Built-in YAML** — only `builtin/models/claude-code.yaml` carries `tags: [kanban]`; llama chat models, embedding models, and the test model are untagged.

4. **CLI `model list`** — `tags` added to `AgentRow`/`VerboseAgentRow` so json/yaml structured output includes them; table view unchanged.

5. **Frontend auto-select** — `apps/kanban-app/ui/src/components/ai-panel-container.tsx`: auto-select now re-selects a default when the persisted model id is no longer offered (a previously-selected llama model dropped out of the list), and falls back to the first model so the panel never strands in `NoModelState`. This fixes \"Claude Code doesn't auto-select even though it's the only option.\"

## Acceptance Criteria

- [x] `ModelInfo` has a `tags: Vec<String>` field populated from YAML frontmatter for all sources.
- [x] `parse_model_tags` returns `[\"kanban\", \"cli\"]` for list/block forms; `[]` for no-frontmatter / no-`tags` content.
- [x] Only `claude-code` carries `kanban` among built-ins; llama/embedding/test models do not.
- [x] `ai_list_models()` returns only `kanban`-tagged models → currently just Claude Code; untagged llamas and embedding models excluded.
- [x] Claude Code's entry is always `available`; CLI detection only sets the `hint`.
- [x] `model list` CLI output (json/yaml/table) still works; `tags` included in structured output.
- [x] AI panel auto-selects Claude Code (incl. when a stale persisted llama id exists).

## Tests

- [x] `crates/swissarmyhammer-config/src/model.rs`: `parse_model_tags` (list / block / missing-frontmatter / no-key) and `test_load_builtin_models_only_claude_code_carries_kanban_tag`.
- [x] `apps/kanban-app/src/ai/models.rs`: `list_models_returns_exactly_kanban_tagged_models`, `list_models_excludes_untagged_llama_model`, `list_models_excludes_local_llama_models_for_now`, `list_models_claude_code_available_even_when_cli_not_detected`, `list_models_includes_claude_code_entry_reflecting_detection` (15 tests total).
- [x] `apps/swissarmyhammer-cli/.../model/display.rs`: `agent_row_includes_tags_in_structured_output` / verbose variant.
- [x] `apps/kanban-app/ui/.../ai-panel-container.test.tsx`: `re-selects a default when the persisted model is no longer offered` + lone-model auto-select.
- [x] `cargo test -p swissarmyhammer-config` (full crate green), `cargo test -p kanban-app ai::models` (15), `cargo test -p swissarmyhammer-cli model` (87, single-threaded — pre-existing global-`HOME` env race in `test_model_list_with_invalid_model_files`).
- [x] `cd apps/kanban-app/ui && npm test` → 2444 UI tests pass (tsc clean).
- [x] `cargo clippy` clean on `swissarmyhammer-config`, `swissarmyhammer-cli`, `kanban-app`.

## Workflow

- Used `/tdd` throughout — failing tests first, then implement to green.

## Review Findings (2026-05-25 12:42)

### Warnings
- [x] `apps/kanban-app/src/ai/models.rs:97-99` — The `Model::available` field doc is now stale and contradicts the code. It states \"A Claude Code entry is `false` when the `claude` CLI is not detected\", but `claude_code_model()` (line 120) now unconditionally sets `available: true`, and the `claude_code_model` doc (lines 106-113) explains this deliberate change. A field-level comment that asserts the opposite of the actual behavior will mislead the next reader. Update it to: Claude Code is always `true` (the CLI is spawned lazily at use time); local llama models are always `true` (weights fetched lazily).
- [x] `apps/kanban-app/src/ai/models.rs:101-102` — The `Model::hint` field doc is stale for the Claude Code case. It says the hint \"Carries the 'install Claude Code' hint when the entry is present-but-disabled\", but Claude Code is never present-but-disabled anymore; its hint now carries the resolved CLI path when detected and is `None` otherwise (see `claude_code_model`, line 115, and the `list_models_claude_code_available_even_when_cli_not_detected` test asserting `hint.is_none()`). Reword to describe the current contract: the resolved CLI path for Claude Code (when detected), or a model description for local llama entries.

### Nits
- [x] `crates/swissarmyhammer-config/src/model.rs:787-800` — `extract_yaml_frontmatter_list` silently drops a scalar `tags:` value. A config that writes `tags: kanban` (a bare string instead of a sequence) parses to a YAML scalar, `as_sequence()` returns `None`, and the whole key is discarded → empty `Vec`, so the model silently never surfaces in the panel. This is consistent with the lenient parsing style and the `_list` naming, so it is acceptable as-is; consider either accepting a lone scalar as a single-element list or documenting on `parse_model_tags` that `tags` must be a YAML sequence.