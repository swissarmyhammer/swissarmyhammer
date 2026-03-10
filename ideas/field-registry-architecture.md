# Field Registry Architecture

## Crate: `swissarmyhammer-fields`

The field registry is a standalone crate — `swissarmyhammer-fields`. It owns field definitions and entity definitions (which fields belong to which entity types). It does **not** own field values — the (owner, field, value) triples remain in their current storage locations (e.g. `.kanban/tasks/`, `.kanban/tags/`). The crate is schema-only. It knows nothing about kanban, tasks, or tags.

Construction takes a `Path` and an optional set of default definitions:

```rust
let fields = FieldsContext::open(path.join("fields"))
    .with_defaults(kanban_defaults())
    .await?;
```

The crate creates and manages its own directory structure under that path. Field definitions are stored as individual YAML files — one per field. YAML is the on-disk format, not JSON. Consumers decide where that path lives.

### Default definitions

`swissarmyhammer-fields` knows nothing about kanban, tasks, or tags. Consumers provide built-in field definitions and entity templates via `with_defaults()`. On open, the crate writes any defaults that don't already exist on disk (matched by ULID). Definitions that are already present are left untouched — user customizations survive.

```rust
/// Kanban provides its built-in defaults
fn kanban_defaults() -> FieldDefaults {
    FieldDefaults::new()
        // Field definitions
        .field(FieldDef {
            id: ulid!("01JMTASK0000000000STATUS0"),
            name: "status",
            type_: FieldType::Select { options: vec![...] },
            default: Some("Backlog"),
            editor: Editor::Select,
            display: Display::Badge,
            ..Default::default()
        })
        .field(FieldDef {
            id: ulid!("01JMTASK0000000000TITLE00"),
            name: "title",
            ..Default::default()
        })
        // ... all built-in fields ...

        // Entity templates
        .entity(EntityDef {
            name: "task",
            body_field: Some("body"),
            fields: vec!["title", "status", "priority", "tags", "assignees", "due", "depends_on", "body"],
        })
        .entity(EntityDef {
            name: "tag",
            body_field: None,
            fields: vec!["tag_name", "color", "description", "usage", "last_used"],
        })
        .entity(EntityDef {
            name: "actor",
            body_field: None,
            fields: vec!["name", "actor_type"],
        })
        .entity(EntityDef {
            name: "column",
            body_field: None,
            fields: vec!["name", "order"],
        })
        .entity(EntityDef {
            name: "swimlane",
            body_field: None,
            fields: vec!["name", "order"],
        })
}
```

This means:
- **First open** seeds the full schema — definitions and entity templates written to `fields/`.
- **Subsequent opens** only add new defaults (e.g. a new version of kanban adds a built-in field). Existing definitions are not overwritten.
- **User customizations** are preserved — if the user renames `status` to `state`, the ULID still matches and the default is skipped.
- **Other consumers** provide their own defaults. A non-kanban app using `swissarmyhammer-fields` would pass different built-in fields entirely.

### First integration: KanbanContext

`KanbanContext` composes a `FieldsContext` stored at `.kanban/fields`:

```rust
pub struct KanbanContext {
    root: PathBuf,
    fields: FieldsContext,
}

impl KanbanContext {
    pub async fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let fields = FieldsContext::open(root.join("fields"))
            .with_defaults(kanban_defaults())
            .await?;
        Ok(Self { root, fields })
    }

    pub fn fields(&self) -> &FieldsContext {
        &self.fields
    }
}
```

On-disk layout under `.kanban/`:

```
.kanban/
  board.yaml
  tasks/
  columns/
  swimlanes/
  tags/
  actors/
  activity/
  fields/                        ← owned by FieldsContext
    definitions/                ← one .yaml file per field (e.g. status.yaml, priority.yaml)
    entities/                   ← one .yaml file per entity type (e.g. task.yaml, tag.yaml)
    lib/                        ← importable JS modules for validation functions
```

The `fields/` subtree is entirely managed by `swissarmyhammer-fields`. Kanban code reads field definitions and entity templates through the `FieldsContext` API, never by touching `fields/` directly. Field values (the actual data) remain in their own storage locations (`tasks/`, `tags/`, etc.).

## What a Field Is

A field definition describes a named, typed attribute. `status` stores a string from a fixed set, renders as a colored badge, edits as a select dropdown, sorts by a custom ordering. Every surface in the app that touches `status` derives its behavior from this one definition.

A field instance is an (owner, field, value) triple — logically. The definition lives in the registry. The instances live in entity files (YAML keys or frontmatter keys, depending on the entity template). An entity is just an owner ID — it doesn't have a shape, it just has whatever fields appear in its file.

Fields are not columns. A column is a view-level concern — a field placed in a table at a width. One field can appear as a table column, a board lane grouping, an inspector property row, a `:filter` option, and a command palette grouping choice — all from the same definition.

## Data Model

The **logical** data model is EAV — (owner, field name, value) triples:

```
(task_01HQ3..., title,       "Fix login bug")
(task_01HQ3..., status,      "In Progress")
(task_01HQ3..., priority,    "P1")
(task_01HQ3..., due,         "2025-03-15")
(task_01HQ3..., body,        "Users report intermittent 401s...")
(task_01HQ3..., sprint,      "Sprint 23")
(tag_01JM4...,  tag_name,    "backend")
(tag_01JM4...,  color,       "#3b82f6")
(tag_01JM4...,  description, "Backend infrastructure work")
```

The triples are logical — the **physical** storage depends on the entity template. The `body_field` property determines the format:

- **With `body_field`:** markdown with YAML frontmatter (`.md`). The body after `---` is the designated field.
- **Without `body_field`:** plain YAML (`.yaml`). All fields are YAML keys.

Entity with `body_field` (task — has a markdown body):

```yaml
# .kanban/tasks/01HQ3....md
title: "Fix login bug"
status: "In Progress"
priority: "P1"
sprint: "Sprint 23"
---
Users report intermittent 401s after session timeout.

- [ ] Reproduce locally
- [x] Check token refresh logic
```

Entity without `body_field` (tag — plain YAML, all fields are keys):

```yaml
# .kanban/tags/01JM4....yaml
tag_name: "backend"
color: "#3b82f6"
description: "Backend infrastructure work"
```

Entity without `body_field` (actor — plain YAML):

```yaml
# .kanban/actors/01HQ4....yaml
actor_name: "alice"
email: "alice@example.com"
avatar: "https://..."
```

Any field can be the body field — different entity types can use different fields. This keeps entities human-readable and editable — you can open them in any text editor.

No structural difference between built-in and user-created fields. Both are just keys in the file (YAML or frontmatter). ULIDs exist as stable internal identity for references that must survive renames (perspectives, sort state, column visibility). The data layer uses names.

## Field Definition

```yaml
id: 01JMTASK0000000000STATUS0       # ULID, stable forever
name: status                         # human-facing, renameable
description: "Current workflow state" # optional, for :help and field management UI

type:
  kind: select
  options:
    - value: Backlog
      order: 0
      color: gray
    - value: Todo
      order: 1
      color: blue
    - value: In Progress
      order: 2
      color: yellow
    - value: In Review
      order: 3
      color: purple
    - value: Done
      order: 4
      color: green

default: Backlog

editor: select
display: badge

sort: option-order                   # sort by option.order, not alphabetical
filter: exact                        # filter by exact match against option values
group: value                         # group label = the value itself
```

That's the whole thing. ID, name, what it stores, how it edits, how it displays, how it queries. Plus an optional `validate` — a JavaScript function that transforms or rejects values on write.

## Field Types

A type says what shape the value is. It constrains which editors and displays make sense, but doesn't dictate them.

```yaml
# Simple types
kind: text

kind: markdown
single_line: true           # or false for multi-line

kind: date

kind: number
min: 0
max: 100

kind: color

# Choice types
kind: select
options:
  - value: P0
    label: Critical         # optional display label
    color: red
    icon: fire              # optional
    order: 0                # explicit sort position

kind: multi-select
options: [...]              # same shape as select

# Reference types — pointers to other entities
kind: reference
entity: task                # what entity type this references
multiple: true              # list of IDs (depends_on, assignees) vs single ID

# Derived type — read-only, no stored triple
kind: computed
derive: tag-usage-count     # named derivation, resolved at runtime
```

**Reference vs computed-from-entities — two different patterns:**

**Reference fields** (`kind: reference`) store entity IDs directly. `depends_on` stores `["01HQ3...", "01HQ4..."]` in frontmatter — a literal list of task ULIDs. The field owns its value. Reference fields get a **default validation** that prunes dangling IDs (see Validation below). `multiple: true` means array of IDs; `multiple: false` means a single ID or null.

**Computed fields with entity derivation** (`kind: computed`) don't store anything. `tags` is computed — its derive function (`parse-body-tags`) loads all tag entities and checks which `#<tag_name>` patterns appear in the body text (instr match). The body IS the source of truth:
- Adding a tag = typing `#tag_name` in the body
- Removing a tag = deleting `#tag_name` from the body
- The field value is always derived, never stale

Both patterns need access to entity stores, but for different reasons: references need `exists()` for validation, computed fields need the full entity set for derivation. Both get their entity access through `ctx.lookup`.

The `order` field on options is what makes `:sort status` produce Backlog → Todo → In Progress → In Review → Done, not alphabetical. Same options array drives board lane ordering.

## Validation

A field definition can include a `validate` property — a JavaScript function body. The function runs on both read and write — clean in, clean out. On write, it can transform or reject values before storage. On read, it ensures stale data is cleaned up (e.g. dangling references pruned).

The function receives a single context object:

```javascript
// The validation function body receives one argument: ctx
// Destructure what you need:
//
//   ctx.value   — the incoming field value (string, number, etc.)
//   ctx.fields  — object with all other field values on the entity
//   ctx.name    — the field name being validated
//   ctx.lookup  — async entity lookup function
//                 await ctx.lookup(type, id)  → entity or null  (get one)
//                 await ctx.lookup(type)      → [...]           (get all of that type)
//                 e.g. await ctx.lookup("task", "01HQ3...")  → {id, title, status, ...}
//                 e.g. await ctx.lookup("tag")               → [{id, tag_name, ...}, ...]
//
// Return the (possibly transformed) value, or throw to reject.
// Can be sync or async (return a value or a Promise).
```

`ctx.lookup` is a single async function provided by the consumer. Two calling conventions:
- `await ctx.lookup(type, id)` → entity object or `null` (get one — used by reference validation)
- `await ctx.lookup(type)` → array of entity objects (get all — used by computed derivations like `parse-body-tags`)

The consumer (e.g. kanban) implements this to query its own entity stores. `swissarmyhammer-fields` doesn't know how to look up entities — it just passes through whatever lookup function the consumer provides.

Example — `tag_name` validation:

```yaml
validate: |
  const { value } = ctx;
  let v = value.trim().replace(/ +/g, "_").replace(/\0/g, "");
  if (v.length === 0) throw new Error("tag_name cannot be empty");
  return v;
```

Example — async validation with an imported helper:

```yaml
validate: |
  import { normalize } from "helpers/text.js";
  const { value, fields } = ctx;
  const v = await normalize(value);
  if (fields.status === "Done") throw new Error("cannot change on completed tasks");
  return v;
```

### Validation imports

Validation functions can import JS modules from `fields/lib/`:

```
fields/
  definitions/
  entities/
  lib/                          ← importable JS modules
    helpers/
      text.js                   ← import { normalize } from "helpers/text.js"
```

Imports resolve relative to `fields/lib/`. No network, no absolute paths, no `node_modules` — just local files within the fields directory. This keeps validation sandboxed while allowing shared logic across field validators.

The validation engine uses `swissarmyhammer-js` (QuickJS). QuickJS supports ES module imports and async/await natively. **Note:** `swissarmyhammer-js` currently does not drain the pending job queue after evaluation — it needs to call `rt.execute_pending_job()` in a loop after each eval for Promises to resolve. It also needs a module loader configured to resolve imports from the `fields/lib/` directory. Both are small additions to the worker loop.

Validation functions are sandboxed — no network, no filesystem access outside `fields/lib/`. They see the value, sibling fields, and can import local helpers.

If no `validate` is specified, the value passes through unchanged (type-level constraints like select options are enforced separately).

### Default validation for reference fields

Reference fields (`kind: reference`) get an automatic default validation even without an explicit `validate` property. The default prunes dangling references — IDs that don't resolve to an existing entity are silently removed:

```javascript
// Automatic default for reference fields (applied by the engine, not written in YAML):
const ids = Array.isArray(ctx.value) ? ctx.value : [ctx.value];
const valid = [];
for (const id of ids) {
  if (await ctx.lookup(entityType, id) !== null) {
    valid.push(id);
  }
}
return multiple ? valid : (valid[0] ?? null);
```

This means:
- A `depends_on` list with a deleted task ID → that ID is silently dropped from the list.
- An `assignees` list with a deleted actor ID → that ID is silently dropped.
- No error thrown — broken references are cleaned up, not rejected.

If an explicit `validate` is provided on a reference field, it runs **instead of** the default (not in addition to). The explicit validator can call `ctx.lookup` itself if it wants the same pruning behavior plus additional logic.

### Lookup provider registration

The consumer provides a single lookup function when creating the `ValidationEngine`:

```rust
#[async_trait]
pub trait EntityLookup: Send + Sync {
    async fn get(&self, entity_type: &str, id: &str) -> Option<serde_json::Value>;
    async fn list(&self, entity_type: &str) -> Vec<serde_json::Value>;
}

let engine = ValidationEngine::new()
    .with_lookup(kanban_lookup);  // implements EntityLookup
```

The consumer's lookup implementation dispatches on `entity_type` to check the right store. `swissarmyhammer-fields` owns the validation engine and the `ctx.lookup` plumbing. The consumer provides the single lookup. This keeps the crate decoupled from any specific storage backend.

## Editors and Displays

Separate from type because the same type can present differently.

**Editors** — how you change a value:

```
markdown          # CodeMirror, single-line or multi-line
select            # dropdown from options
multi-select      # multi-pick from options
date              # date picker
color-palette     # color swatches
none              # computed fields — no editor
```

**Displays** — how you show a value:

```
markdown          # rendered markdown
badge             # colored pill (for select values)
badge-list        # multiple colored pills (for multi-select)
avatar            # user avatar
date              # formatted date
color-swatch      # color circle
number            # plain number
text              # plain text
```

A `select` type field could display as `badge` or `text`. The definition chooses.

## Entity Definitions

An entity definition is a **template** — it declares the default set of fields stamped onto new instances of that type. Stored as YAML in `fields/entities/`:

```yaml
# fields/entities/task.yaml
name: task
body_field: body                     # this field is stored as the markdown body after ---
fields:
  - title
  - status
  - priority
  - tags
  - assignees
  - due
  - depends_on
  - body

# fields/entities/tag.yaml
name: tag
fields:
  - tag_name
  - color
  - description
  - usage
  - last_used

# fields/entities/actor.yaml
name: actor
fields:
  - name
  - actor_type

# fields/entities/column.yaml
name: column
fields:
  - name
  - order

# fields/entities/swimlane.yaml
name: swimlane
fields:
  - name
  - order
```

Entity definitions are templates, not constraints:

- **Sparse.** Not every instance will have a value for every field in the definition. A task with no `due` date simply has no `due` triple.
- **Extensible.** Individual instances can have fields not listed in their entity definition. A particular task might have a `sprint` field even if `sprint` isn't in the task entity definition.
- **One-directional.** Entity definitions point at fields. Fields never point back at entities — no `entity` property, no backlinks. If a field is renamed, the system scans entity definitions to update references. This is fine because renames are rare.

When creating a new entity instance, the definition provides the starting field set. After that, the instance's actual fields are just whatever (owner, field, value) triples exist for it. The definition is consulted for UI defaults (which fields to show in the inspector, which columns to display in a table), but never enforced as a constraint.

Entity definitions are how `swissarmyhammer-fields` knows what templates exist — the crate doesn't hardcode "task" or "tag".

## Built-In Fields

### Task Fields

```yaml
- id: 01JMTASK0000000000TITLE00
  name: title
  type: { kind: markdown, single_line: true }
  editor: markdown
  display: markdown
  sort: alphanumeric

- id: 01JMTASK0000000000STATUS0
  name: status
  type:
    kind: select
    options:
      - { value: Backlog,     order: 0, color: gray }
      - { value: Todo,        order: 1, color: blue }
      - { value: In Progress, order: 2, color: yellow }
      - { value: In Review,   order: 3, color: purple }
      - { value: Done,        order: 4, color: green }
  default: Backlog
  editor: select
  display: badge
  sort: option-order
  group: value

- id: 01JMTASK0000000000PRIORTY
  name: priority
  type:
    kind: select
    options:
      - { value: P0, order: 0, color: red,    label: Critical }
      - { value: P1, order: 1, color: orange, label: High }
      - { value: P2, order: 2, color: yellow, label: Medium }
      - { value: P3, order: 3, color: blue,   label: Low }
  editor: select
  display: badge
  sort: option-order

- id: 01JMTASK00000000000TAGS00
  name: tags
  type: { kind: computed, derive: parse-body-tags }
  editor: none                       # tags are edited by typing #tag in the body
  display: badge-list
  filter: substring

- id: 01JMTASK0000000000ASSIGN0
  name: assignees
  type: { kind: reference, entity: actor, multiple: true }
  editor: multi-select
  display: avatar

- id: 01JMTASK0000000000DEPS000
  name: depends_on
  type: { kind: reference, entity: task, multiple: true }
  editor: multi-select               # pick from task list
  display: badge-list
  filter: substring

- id: 01JMTASK0000000000DUEDAT0
  name: due
  type: { kind: date }
  editor: date
  display: date
  sort: datetime

- id: 01JMTASK00000000000BODY00
  name: body
  type: { kind: markdown, single_line: false }
  editor: markdown
  display: markdown
```

### Tag Fields

```yaml
- id: 01JMTAG000000000000ID000
  name: tag_name
  type: { kind: text, single_line: true }
  editor: markdown
  display: text
  sort: alphanumeric
  validate: |
    const { value } = ctx;
    let v = value.trim().replace(/ +/g, "_").replace(/\0/g, "");
    if (v.length === 0) throw new Error("tag_name cannot be empty");
    return v;

- id: 01JMTAG00000000000COLOR00
  name: color
  type: { kind: color }
  editor: color-palette
  display: color-swatch

- id: 01JMTAG00000000000DESC000
  name: description
  type: { kind: markdown, single_line: true }
  editor: markdown
  display: markdown

- id: 01JMTAG00000000000USAGE00
  name: usage
  type: { kind: computed, derive: tag-usage-count }
  editor: none
  display: number
  sort: numeric

- id: 01JMTAG00000000000LAST000
  name: last_used
  type: { kind: computed, derive: tag-last-used }
  editor: none
  display: date
  sort: datetime
```

### Shared Fields

```yaml
- id: 01JMSHRD0000000000NAME000
  name: name
  type: { kind: text, single_line: true }
  editor: markdown
  display: text
  sort: alphanumeric

- id: 01JMSHRD0000000000ORDER00
  name: order
  type: { kind: number, min: 0 }
  editor: number
  display: number
  sort: numeric

- id: 01JMACTR0000000000TYPE000
  name: actor_type
  type:
    kind: select
    options:
      - { value: human, order: 0 }
      - { value: agent, order: 1 }
  editor: select
  display: badge
```

`name` and `order` are shared field definitions used by multiple entity templates (actor, column, swimlane). One definition, referenced by name in each template's field list. `tag_name` is deliberately separate from `name` because it has special validation (space→underscore, trim, no nulls).

Task uses `body` for its main markdown content; tags use `description` for a short text annotation. No name conflict.

## Computed and Reference Fields

Some fields don't store static values — they're computed from other data or reference other entities.

**`tags` — purely computed, not stored.** The `tags` field has no frontmatter key. Its value is derived at read time by the `parse-body-tags` derivation: load all tag entities, check which `#<tag_name>` patterns appear in the entity's body text (instr match against each known tag). The body IS the source of truth. Editing tags = editing body text. `#bug` in the description = this entity has the `bug` tag. Auto-creation of new tag entities from bare `#newtag` patterns is a consumer-level concern.

**`usage`, `last_used` — purely computed from query.** These tag fields derive their values from runtime queries (`tag-usage-count`, `tag-last-used`). No stored triple, no frontmatter key.

**`assignees`, `depends_on` — reference fields, stored.** These store entity IDs directly in frontmatter. `depends_on: ["01HQ3...", "01HQ4..."]` is a literal list of task ULIDs. The `reference` type declares the target entity type. Reference fields get a default validation that prunes dangling IDs (see Validation above). Their editor presents a pick list populated from the target entity store at read time.

The key distinction: **computed fields derive values, reference fields store IDs.** Both need entity store access — computed for derivation, references for validation.

## User-Defined Fields

A user creates a custom field through the field management UI. The persisted definition:

```yaml
id: 01HQ3USERFIELD00000SPRINT
name: sprint
type:
  kind: select
  options:
    - { value: Sprint 22, order: 0 }
    - { value: Sprint 23, order: 1 }
    - { value: Sprint 24, order: 2 }
```

The field definition has no `entity` property — fields don't know who uses them. Adding the field to an entity template is a separate step: push the name onto the entity definition's field list. Or skip the template entirely — any instance can have any field regardless of its entity definition.

Users can create: text, markdown, select, multi-select, date, number. Not computed or color (system-only).

Editor and display are inferred from type if not specified:

```
text         → editor: markdown (single-line),  display: text
markdown     → editor: markdown,                display: markdown
select       → editor: select,                  display: badge
multi-select → editor: multi-select,            display: badge-list
date         → editor: date,                    display: date
number       → editor: number,                  display: number
```

## Operations

### Create field

```
1. [fields]   Generate ULID
2. [fields]   Register definition in registry (indexed by both ULID and name)
3. [fields]   Push name onto entity template's field list
4. [consumer] Emit field-added
5. [consumer] Views re-derive (table gets column, board gets grouping option, inspector gets row)
```

### Edit field options

```
1. [fields]   Update definition in registry (e.g. add "Sprint 25" to sprint's options)
2. [fields]   Persist updated definition YAML
3. [consumer] Emit field-updated
4. [consumer] All surfaces using this field's options update:
              - filter dropdowns
              - select editors
              - board lanes (if grouped by this field)
              - command autocomplete
```

### Rename field

Rename touches three things: registry, stored data, entity templates.

```
1. [fields]   Update registry: field.name = "iteration"     (ULID unchanged)
2. [consumer] Rewrite store: rename frontmatter key "sprint" → "iteration" in all entity files
3. [fields]   Update entity templates: "sprint" → "iteration" in field lists
4. [consumer] Emit field-updated
5. [consumer] Views re-derive: column headers, autocomplete, inspector labels update
```

ULID-based references (perspectives, sort state, column visibility) are untouched.

### Delete field

```
1. [consumer] Remove frontmatter key "sprint" from all entity files that have it
2. [fields]   Remove "sprint" from entity template field lists
3. [fields]   Remove persisted definition YAML
4. [fields]   Unregister from registry
5. [consumer] Emit field-removed
6. [consumer] Views re-derive: column disappears, grouping option disappears
```

### Read field value

For a stored field: look up `entity[field.name]` — the name is the key.

For a computed field: call the named derive function. No stored triple exists.

### Resolve name → ULID

Commands and autocomplete work with names. Persistent state stores ULIDs.

```
User types:    :sort status desc
Autocomplete:  field names from current field set
Resolve:       "status" → registry lookup → 01JMTASK0000000000STATUS0
Store:         sort state records the ULID
Display:       column header shows current name (from registry)
```

If `status` is later renamed to `state`, the stored ULID still resolves. The header shows `state`.

## How the Registry Connects to Everything

### Table view

Generates columns from the entity field list:

```
for each name in task_fields:
  definition = registry.byName(name)
  column id        = definition.id          # ULID
  column header    = definition.name
  column value     = row[definition.name]   # name is the data key
  column cell      = definition.display
  column editor    = definition.editor
  column sort      = definition.sort
  column filter    = definition.filter
```

Column visibility is a ULID → boolean map (survives renames).

### Board view

Groups by a select field. Lanes come from options.

```
group_field = registry.byName("status")
lanes       = group_field.type.options      # ordered by option.order
card_fields = registry.resolve(["priority", "tags", "assignees", "due"])
```

`:board group priority` → lanes are P0, P1, P2, P3 in order.
`:board group sprint` → lanes are user-defined sprint options in order.

Any select field is a valid board grouping key.

### Inspector

Generic property sheet. Takes a field set and an entity, renders rows:

```
for each name in field_set:
  definition = registry.byName(name)
  value      = entity[name]                 # or derive for computed
  render:
    label    = definition.name
    display  = component from definition.display
    editor   = component from definition.editor (if editor != none)
```

The inspector doesn't know about tasks or tags. Header and footer slots are where entity-specific chrome goes (tag preview pill, subtask checklist). The core is just "for each field, render."

### Command system

`:sort`, `:filter`, `:group` autocomplete from the current field set:

```
:sort autocomplete
  offers: sortable field names from current field set
  resolves input name → ULID for storage

:filter autocomplete (two-stage)
  stage 1: field names from current field set
  stage 2: if field is select, offer option values; else free text

:group autocomplete
  offers: select/multi-select field names from current field set
```

### Perspectives

A saved perspective references field ULIDs (survive renames):

```yaml
id: perspective_01
name: "Active Sprint"
view: table
filters:
  - field: 01JMTASK0000000000STATUS0
    operator: not-eq
    value: Done
  - field: 01HQ3USERFIELD00000SPRINT
    operator: eq
    value: Sprint 23
group_by: 01JMTASK0000000000STATUS0
sort:
  - field: 01JMTASK0000000000PRIORTY
    direction: asc
visible_fields:
  - 01JMTASK0000000000TITLE00
  - 01JMTASK0000000000STATUS0
  - 01JMTASK0000000000PRIORTY
  - 01HQ3USERFIELD00000SPRINT
```

If a field is deleted, perspective clauses referencing its ULID gracefully degrade (ignored on resolve).

### :help fields

Generated from registry. Lists all fields with name, type, editor, display. Always current, never stale.

## What the Registry Does NOT Own

- **Field values.** The registry knows the schema. The (owner, name, value) triples live in the store.
- **View layout.** Column widths, column order, visibility — view config. Visibility uses ULIDs.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Field identity | ULID, stable, generated once | Renames don't break internal references. |
| Field name | Renameable label, unique within a field set | Human-facing everywhere: data keys, commands, column headers. |
| Data storage key | Field name | YAML reads like a person wrote it. Worth the rename migration. |
| Internal reference key | ULID | Perspectives, sort state, column visibility survive renames. |
| Entity storage | YAML frontmatter + markdown body (logically EAV triples keyed by field name) | Human-readable files. No typed entity shapes. Built-in and user fields are the same frontmatter keys. |
| Entity definitions | YAML templates in `fields/entities/`, listing default field names | Templates, not constraints. Instances can diverge (sparse or extended). |
| Entity-field association | One-directional: entity definitions list fields, fields never reference entities | No backlinks. Renames scan entity definitions. Rare operation, simple implementation. |
| Field definition | Pure schema: name, type, editor, display, sort/filter/group, validate | No entity ownership, no built-in flag, no required flag, no accessor. |
| Validation | Pluggable JavaScript function body per field, run via swissarmyhammer-js | Sandboxed transforms — see value + sibling fields, import from `fields/lib/`, return new value or throw. |
| Read-only | Inferred: type computed + editor none | Duck type it. |
| Computed fields | Named derive function, no stored triple | Same as stored fields from the display side. |
| Reference fields | `kind: reference` with `entity` and `multiple` | Explicit entity pointers. Default validation prunes dangling IDs silently. |
| Reference validation | Consumer-provided `ctx.lookup(type, id)` returning entity or null | Keeps fields crate decoupled from storage. Single lookup function registered at engine creation. |
| Tags field | Computed via `parse-body-tags`, not stored | Tags live in the body as `#tag` patterns. No frontmatter key. Derive loads all tags, does instr match against body. |
| Computed entity access | Derive functions get `ctx.lookup` same as validation | Computed fields like `tags` need the full entity set. Same lookup provider mechanism. |
| Editor/display inference | Derived from type for user fields, explicit for built-in | User fields get sensible defaults. Built-in get precise control. |
| User creatable types | text, markdown, select, multi-select, date, number | No computed, no color — simple for users. |
| Field rename | Migrates stored data + field list entry, ULID references untouched | Renames are rare. Readable data is worth it. |
| Field deletion | Removes definition + triples + field list entry | Clean, no orphaned data. |
| Field naming | All snake_case | Consistent, programmer-friendly. No mixed casing. |
| Command resolution | User types names, system stores ULIDs | `:sort status desc` → resolve → store ULID. Rename-proof. |
