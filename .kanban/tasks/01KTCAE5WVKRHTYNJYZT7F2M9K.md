---
assignees:
- claude-code
depends_on:
- 01KTCADKCJT9WEW123VYTFZZZH
position_column: todo
position_ordinal: '8280'
project: card-comments
title: Define comments field YAML and add it to the task entity
---
## What
Declare the `comments` field as builtin field-definition data and attach it to the task entity, following the data-driven YAML pattern used by every existing field (see `crates/swissarmyhammer-kanban/builtin/definitions/attachments.yaml` and `scheduled.yaml`, and how `attachments` is listed in `crates/swissarmyhammer-kanban/builtin/entities/task.yaml`).

Builtin definitions are loaded dynamically via `include_dir!` (`crates/swissarmyhammer-kanban/src/defaults.rs:28` — `BUILTIN_DEFINITIONS`), merged with local `.kanban/definitions/` overrides. CONFIRMED: there is no manual filename registry, so a new YAML file is picked up automatically.

Files:
1. Create `crates/swissarmyhammer-kanban/builtin/definitions/comments.yaml`:
   - `id: "00000000000000000000000018"` — the next free sentinel id. CONFIRMED the current highest in `builtin/definitions/` is `...17` (status_date); `...16` is completed. Use `...18`. (Sentinel ids are zero-padded and sort before real ULIDs; the last two chars are the builtin field code.)
   - `name: comments`
   - `description: Conversation log`
   - `type: { kind: comment-log }` (the variant added in the dependency task)
   - `icon: message-square` (Lucide)
   - `editor: comment-log`, `display: comment-log` (explicit, matching the inferred values)
   - `section: log` (a NEW dedicated, labeled section — see file 2; NOT the footer)
   - no `sort`/`groupable` (a log is not a grid column)
2. Edit `crates/swissarmyhammer-kanban/builtin/entities/task.yaml`:
   - Add a new section to the `sections:` list (currently header, body, dates, system, footer at lines ~276-284): append `- id: log` with `label: Log`. Place it LAST so the conversation log renders at the bottom of the inspector, below footer/attachments.
   - Add `comments` to the `fields:` list (order in this list does not affect section grouping, but keep it near `attachments` for readability).

## Acceptance Criteria
- [ ] `comments.yaml` exists with id `...18`, `kind: comment-log`, and `section: log`.
- [ ] `task.yaml` declares a `log` section with `label: Log` (placed last) and lists `comments` in its fields.
- [ ] A board initialized from builtins exposes a `comments` field on the `task` entity schema with editor/display `comment-log` and section `log`, and the task entity has a `log` section labeled "Log".
- [ ] No duplicate sentinel id with any other file in `builtin/definitions/`.

## Tests
- [ ] Add a test in the kanban crate (alongside existing schema/defaults tests — see `crates/swissarmyhammer-kanban/src/defaults.rs` and `schema.rs` test modules) that loads the builtin task entity definition and asserts the `comments` field is present with `effective_editor()==\"comment-log\"`, `effective_display()==\"comment-log\"`, and `section==\"log\"`, and that the entity declares a `log` section with label `Log`.
- [ ] Add a test asserting all builtin definition sentinel ids are unique (guards against id collision).
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — green.

## Workflow
- Use `/tdd` — write the schema-load assertion test first (it fails until the YAML exists), then add the YAML.