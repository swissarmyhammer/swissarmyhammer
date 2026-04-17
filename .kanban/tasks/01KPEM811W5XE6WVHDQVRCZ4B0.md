---
assignees:
- claude-code
depends_on:
- 01KPENGNDX2526DZCRYT9N1E9P
position_column: todo
position_ordinal: d080
title: 'Commands: document cross-cutting rule + YAML hygiene test (no cross-cutting in entity schemas)'
---
## What

Establish the rule the rest of this plan enforces. Today there is no documented convention for what belongs in `swissarmyhammer-commands/builtin/commands/entity.yaml` vs. a type-specific command YAML vs. a per-entity schema (`swissarmyhammer-kanban/builtin/entities/*.yaml`). Duplicate `ui.inspect` blocks copy-pasted across six entity YAMLs, redeclared `entity.*` commands inside `task.yaml`/`tag.yaml`/`project.yaml`, and a broken `project.delete` all trace back to this ambiguity.

### Rule to encode

- **Command declarations** (full contract: `id`, `name`, `params`, `undoable`, `keys`, `context_menu`) live in `swissarmyhammer-commands/builtin/commands/*.yaml`, split by noun:
  - `entity.yaml` — cross-cutting commands that apply to any entity by target moniker: `entity.add`, `entity.delete`, `entity.archive`, `entity.unarchive`, `entity.cut`, `entity.copy`, `entity.paste`, `entity.update_field`. No `scope:` pinning.
  - `ui.yaml` — UI commands, including the cross-cutting `ui.inspect`.
  - `task.yaml`, `tag.yaml`, `project.yaml`, `column.yaml`, `attachment.yaml`, `perspective.yaml` — type-specific commands only.
- **The cross-cutting signal is the params declaration**, not a new flag. A command whose primary param declares `from: target` operates on whatever the context menu fired over; the scope_commands emitter auto-emits it once per entity moniker in the scope chain. Rust `available()` provides the per-type opt-out (e.g. attachments reject `entity.archive`).
- **Per-entity schemas** (`swissarmyhammer-kanban/builtin/entities/*.yaml`) list ONLY type-specific command references with overlay metadata (`context_menu`, `keys`). Cross-cutting commands are NOT listed — they auto-emit from the registry.

### Hygiene test

The test scans every `swissarmyhammer-kanban/builtin/entities/*.yaml` and fails if any entry's `id` matches a known cross-cutting command (`ui.inspect`, `entity.delete`, `entity.archive`, `entity.unarchive`, `entity.copy`, `entity.cut`, `entity.paste`, `entity.add`, `entity.update_field`). Those commands must not appear in entity schemas at all — they emit via the cross-cutting mechanism.

### Files to touch

- `swissarmyhammer-commands/builtin/commands/entity.yaml` — header comment block explaining the rule and listing cross-cutting IDs.
- `/Users/wballard/.claude/projects/-Users-wballard-github-swissarmyhammer-swissarmyhammer/memory/` — add `feedback_command_organization.md` memory and index from `MEMORY.md`.
- `swissarmyhammer-kanban/src/scope_commands.rs` (tests module) — add `yaml_hygiene_no_cross_cutting_in_entity_schemas`.

### Subtasks

- [ ] Write rule as a header comment in `entity.yaml` with the cross-cutting ID list.
- [ ] Write the feedback memory file and link from `MEMORY.md`.
- [ ] Add the hygiene test. It MUST fail on the current branch (six entity YAMLs list `ui.inspect`; task/tag/project YAMLs list more cross-cutting commands).

## Acceptance Criteria

- [ ] `entity.yaml` opens with the rule-comment header naming the cross-cutting IDs.
- [ ] `MEMORY.md` indexes a new `feedback_command_organization.md` memory.
- [ ] Hygiene test exists and **fails on this branch** — the failure drives the rest of the plan.

## Tests

- [ ] Add `yaml_hygiene_no_cross_cutting_in_entity_schemas` in `swissarmyhammer-kanban/src/scope_commands.rs` tests module. Load every `builtin/entities/*.yaml`, iterate `commands:` entries, fail listing every entity/command pair that violates the rule.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban yaml_hygiene_no_cross_cutting_in_entity_schemas` — expect FAIL on this branch, expect PASS after the plan lands.

## Workflow

- Use `/tdd` — the hygiene test is the RED step for the whole plan. Commit it failing; later cards turn it green.

#commands