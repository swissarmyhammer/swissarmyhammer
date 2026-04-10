---
assignees:
- claude-code
depends_on:
- 01KNVSGJ7X7A95T8XV80E9M7Y3
- 01KNVSK9V84X79TKGPP9XCYKFH
position_column: done
position_ordinal: ffffffffffffffffffffffaf80
project: expr-filter
title: 'kanban skill: document $project filter syntax'
---
## What

Teach agents to filter by project using the new `$project-slug` DSL atom by updating the kanban skill documentation. Keep the edits surgical — the skill is already well-structured and the change is additive.

**File to modify:** `builtin/skills/kanban/SKILL.md`

**⚠ Source-of-truth reminder:** `.skills/kanban/SKILL.md` is GENERATED from `builtin/skills/kanban/SKILL.md`. Do NOT edit the `.skills/` copy — edit only `builtin/skills/kanban/SKILL.md`. A memory entry in `MEMORY.md` documents this convention.

**Changes to make:**

1. **Filter DSL table (in the `## Filtering Work` → `### Filter DSL` section)** — add a row for `$project-slug`, maintaining alphabetical-by-syntax ordering or insert position-wise between `#tag` and `@user`:
   ```markdown
   | `$project-slug` | Match tasks assigned to this project (by project slug/id) |
   ```

2. **Picking Up Work examples (`### Picking Up Work` section)** — add one example that combines a project filter with other atoms, e.g.:
   ```json
   {"op": "next task", "filter": "$auth-migration"}
   {"op": "next task", "filter": "$auth-migration && @alice"}
   ```

3. **Listing Tasks examples (`### Listing Tasks` section)** — add example(s) mirroring the style of existing examples:
   ```json
   {"op": "list tasks", "filter": "$auth-migration"}
   {"op": "list tasks", "filter": "$auth-migration && #bug"}
   {"op": "list tasks", "filter": "$auth-migration || $frontend"}
   ```

4. **`## Using Projects to Group Tasks` section** — this section already exists and explains how to create/assign projects. Add a new subsection `### Filtering by Project` (before `### Workflow`) that tells agents to use `$project-slug` in any filter expression:
   ```markdown
   ### Filtering by Project

   Once tasks are assigned to projects, use the `$project-slug` atom in any filter to scope work to a specific project. It composes with other atoms the same way `#tag` and `@user` do:

   ```json
   {"op": "next task", "filter": "$auth-migration"}
   {"op": "list tasks", "filter": "$auth-migration &&
   {"op": "list tasks", "filter": "!$auth-migration"}   // exclude project
   ```
   ```

5. **Workflow bullets** — the existing `### Workflow` bullet "3. Use projects to filter and focus work in the UI" currently lacks specificity. Update it to reference the `$project-slug` DSL:
   > 3. Use the `$project-slug` filter atom in `list tasks` and `next task` to focus work on a project

**Context:**
- The skill lives in `builtin/skills/kanban/SKILL.md` and is synced to `.skills/kanban/SKILL.md` by the build (per `MEMORY.md`).
- The changes must stay consistent with the Filter DSL section's existing table formatting, code-fence language (`json`), and tone.
- Do not touch the pipe-escaping in the existing `||` rows — keep the `\|\|` style intact.

## Acceptance Criteria

- [ ] `builtin/skills/kanban/SKILL.md` Filter DSL table includes a `$project-slug` row, placed consistently with the other atom rows
- [ ] `### Picking Up Work` has at least one `$project-slug` example
- [ ] `### Listing Tasks` has at least one `$project-slug` example (preferably showing composition with `#tag` or another atom)
- [ ] `## Using Projects to Group Tasks` has a new `### Filtering by Project` subsection positioned before `### Workflow`
- [ ] The existing `### Workflow` bullet about filtering is updated to reference `$project-slug`
- [ ] Markdown rendering is valid (no broken tables, mis-escaped pipes, mis-closed code fences)
- [ ] `.skills/kanban/SKILL.md` is NOT edited by hand — only `builtin/skills/kanban/SKILL.md` is touched directly; if the generator needs to re-sync, let it
- [ ] The skill text does not claim `$project` does anything the DSL cannot yet do

## Tests

- [ ] Open `builtin/skills/kanban/SKILL.md` in a markdown previewer (or manually eyeball) and confirm the table renders correctly
- [ ] Grep `builtin/skills/kanban/SKILL.md` for `$project-slug` and confirm it appears in the right sections
- [ ] If the repo has a skill validation / lint step (e.g. `cargo run --bin validate-skills` or similar), run it and confirm it passes
- [ ] Run whatever command rebuilds `.skills/` from `builtin/` and confirm the generated file updates (don't commit by hand — let the generator do it, or follow the project's usual regeneration workflow)

## Workflow
- This is a docs-only card. Read the current `SKILL.md`, make the additions described, verify rendering. No TDD needed — follow the skill's existing style exactly. #expr-filter