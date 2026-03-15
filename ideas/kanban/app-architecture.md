# App Architecture: Views, Grid, and Commands

## The Core Idea

Every action is a command. Commands live in composable scopes that nest with the component tree. The command palette — opened by `:` or `Mod+Shift+P` — resolves against the scope chain. The deepest scope wins.

The build order follows the dependency chain:

1. **Command system** — scope chain, palette UI, keybindings
2. **Views** — registry, left nav, view switching
3. **Grid** — shared table infrastructure, cell navigation, cell editing
4. **Inspector** — generic property sheet, scope shadowing
5. **Perspectives** — named field lists with overrides, filter/group fns, sort stack

Each phase adds a scope layer. Each layer doesn't know about the others.

---

## Phase 1: Command System

### Scope chain

Commands live in scopes. Scopes nest with the component tree. When a command is invoked, resolution walks up from the focused element's nearest scope:

```
1. Find the scope closest to the focused DOM element
2. Check that scope for a matching command id
3. If found and available → execute, stop
4. If not found → move to parent scope
5. Repeat until match or root
6. No match anywhere → unhandled
```

For nested scopes: deepest wins. For sibling scopes (like multiple inspectors): only the sibling whose subtree contains the focused element participates. The others are on different branches, invisible to resolution.

### Shadowing and blocking

**Shadowing:** A deeper scope registers the same command id as a shallower one. The deeper one wins. When the deeper scope unmounts, the shallower one takes over. No teardown logic.

**Blocking:** A scope registers a command with `available: false`. Resolution stops — the command exists at this level but is unavailable. It doesn't continue upward. The palette won't show it. The key binding does nothing.

**Pass-through:** A scope simply doesn't register a command. Resolution walks past it to the parent. The scope has no opinion on that command.

### Command definition

```yaml
id: app.save
name: Save
description: "Save all changes"
keys:
  vim: ":w"
  cua: Mod+S
  emacs: C-x C-s
execute: save all
```

Commands with arguments:

```yaml
id: nav.view
name: Switch view
pattern: ":view <name>"
args:
  - name: name
    autocomplete: view names from registry
execute: set active view
```

### The command palette

`:` and `Mod+Shift+P` open the **same UI** — a popover that shows all available commands from the current scope chain. The only difference is how it opens.

The text input inside the palette is a CM6 single-line instance. Not a plain `<input>`, not cmdk's built-in input. CM6. This means the user's keymap works inside the palette — vim motions for editing the filter text, emacs bindings, all of it. Same editor contract as every other text input in the app.

```
┌─────────────────────────────────────────────┐
│  > CM6 single-line input here...            │
├─────────────────────────────────────────────┤
│  Save                              Mod+S   │
│  Switch view                      :view     │
│  Search                              /      │
│  Help                               F1      │
│  ─── Tasks ───                              │
│  Create task                      :new      │
│  Archive selected                 :archive  │
│  ─── Grid ───                               │
│  Sort by column                   :sort     │
└─────────────────────────────────────────────┘
```

Behavior:

```
- Opens as a centered popover
- Text input is CM6 single-line with user's keymap
- Shows all commands from useAvailableCommands() — scope-aware
- Grouped by scope depth (global, view, grid)
- Fuzzy search filters the list as you type
- Shows keybinding hints for current keymap mode
- For commands with arguments: selecting advances to argument input
  (e.g. select ":sort" → CM6 input for field name, with autocomplete)
- Escape dismisses
- Enter on a command executes it
```

When launched with `:`, the CM6 input is pre-focused. When launched with `Mod+Shift+P`, same UI, same list. One UI, two entry points.

### Keybindings

Keybindings are a separate layer. They map key sequences to command ids. The binding table is global per keymap mode — resolution of the command id goes through the scope chain.

```yaml
keymap: vim
bindings:
  ":":          app.command        # opens palette
  Mod+Shift+P:  app.palette        # also opens palette (same UI)
  "/":          app.search
  j:            grid.down
  k:            grid.up
  h:            grid.left
  l:            grid.right
  i:            grid.edit
  Enter:        grid.edit
  dd:           grid.delete
  o:            grid.newBelow
  O:            grid.newAbove
  gg:           grid.top
  G:            grid.bottom
  V:            grid.selectToggle
  zo:           grid.expandGroup
  zc:           grid.collapseGroup
  zR:           grid.expandAll
  zM:           grid.collapseAll
  yy:           grid.copy
  p:            grid.paste
  u:            app.undo
  Mod+R:        app.redo
  Escape:       app.dismiss
```

`Mod` = `Cmd` on Mac, `Ctrl` on Windows/Linux. The binding table uses `Mod` as a platform-neutral modifier. The key handler resolves `Mod` to the correct physical key at runtime. One binding table per keymap mode, works on all platforms.

When a key is pressed:

```
1. Look up key sequence in binding table for current keymap
2. Get command id
3. Resolve command id through scope chain
4. If resolved and available → execute
5. If not resolved → unhandled
```

Multi-key sequences (`gg`, `dd`, `zo`): pending-key buffer with ~500ms timeout. Lookup table of 1-2 key sequences, not a full vim parser.

### Modes

Three modes, same structure regardless of keymap:

```
Normal     keystrokes dispatch commands via binding table → scope chain
Command    palette is open (launched by : or Mod+Shift+P)
Search     / (vim) or Mod+F (cua) — filters/highlights visible content
```

### Primitive commands

Every entity mutation in the app decomposes into three primitives:

```yaml
set:    { owner, field, value }     # write one EAV triple
delete: { owner }                   # remove all triples for an owner
create: { owner, field_set }        # new owner with defaults from field definitions
```

Every high-level entity command is a composition of these:

```
grid.edit commits     → set(task_id, Status, "Done")
task.archive          → set(task_id, Status, "Archived")
board drag to lane    → set(task_id, Status, "In Review")
tags.recolor          → set(tag_id, Color, "#ff0000")
grid.delete           → delete(task_id)
grid.newBelow         → create(new_id, task_fields)
tags.merge            → N × set(...) + delete(old_tag)
```

There is no entity mutation that isn't one of these three. If a new command is added, it composes the same primitives. The entity undo stack, persistence layer, and event system all operate on this one shape.

Perspective mutations have their own primitives and their own changelog (see phase 5).

### Undo / redo

Three undo systems, different granularity:

**CM6 undo** — character-level, within a single editing session. Editing a title, undoing keystrokes. CM6 owns this entirely. When the edit is committed (Escape out, Enter, click away), that session is over and the result becomes a single entity changelog entry.

**Entity undo** — command-level. Global stack, not per-entity. Not persisted across sessions. Every entity mutation logs its inverse so it can be reversed.

**Perspective undo** — config-level. Separate stack from entity undo. Perspective creates, updates, and deletes log their own inverses.

`u` undoes the most recent mutation from whichever changelog applies. If you just changed a task's status → entity undo. If you just changed a perspective's filters → perspective undo. The two never interleave.

The entity undo log entry:

```yaml
# Single field edit
- op: set
  owner: task_01HQ3
  field: Status
  value: "Done"
  previous: "Todo"
```

Undo replays the inverse: `set(task_01HQ3, Status, "Todo")`. Redo replays the forward: `set(task_01HQ3, Status, "Done")`.

For `delete`, the previous is the entire entity — all its triples:

```yaml
- op: delete
  owner: task_01HQ3
  previous:
    Title: "Fix login bug"
    Status: "In Progress"
    Priority: "P1"
    Sprint: "Sprint 23"
```

Undo of a delete = create with those exact values. Undo of a create = delete.

### Transactions

Compound commands group primitives into a single undo entry:

```yaml
command: "Merge #fe → #frontend"
ops:
  - { op: set, owner: task_01, field: Tags, value: "frontend", previous: "fe" }
  - { op: set, owner: task_02, field: Tags, value: "frontend", previous: "fe" }
  - { op: set, owner: task_03, field: Tags, value: "frontend", previous: "fe" }
  - op: delete
    owner: tag_fe
    previous: { Tag: "fe", Color: "#abc", Description: "frontend" }
```

One `Mod+Z` undoes the whole merge — replays all inverses in reverse order. The user doesn't see four separate undos.

Any command that touches more than one primitive wraps them in a transaction:

```
tags.merge       → transaction of N × set + delete
bulk status change (visual mode + :set Status Done) → transaction of N × set
task.new with template → transaction of create + N × set
```

Single-primitive commands (most edits) don't need an explicit transaction — they're already atomic.

### Undo commands

```yaml
- id: app.undo
  name: Undo
  keys: { vim: u, cua: Mod+Z, emacs: C-/ }
  execute: pop undo stack, replay inverses, push to redo stack

- id: app.redo
  name: Redo
  keys: { vim: Mod+R, cua: Mod+Shift+Z, emacs: C-? }
  execute: pop redo stack, replay forward, push to undo stack
```

These live on the global scope. Available everywhere. The redo stack clears when a new mutation happens (standard undo/redo behavior).

Stack is bounded — keep last ~100 entries. Old entries fall off the bottom. No persistence across sessions.

### CM6 is the text editor. Everywhere.

Every text input in the app is a CodeMirror 6 instance. No plain `<input>`, no `<textarea>`, no cmdk input. CM6.

```
Command palette filter  → CM6 single-line
Command argument input  → CM6 single-line with autocomplete
Search input            → CM6 single-line
Cell editors (text)     → CM6 single-line or multi-line
Inspector text fields   → CM6 single-line or multi-line
```

This means one keymap contract everywhere. The user picks vim, emacs, or CUA once and it works in every text context. Tag autocomplete (typing `#` triggers completion) works in cell editors and in the body editor — same CM6 extension, same behavior.

Non-text editors (select dropdowns, date pickers, color palettes) are not CM6 — they're structured widgets. But any time a cursor blinks in a text field, it's CM6.

### What phase 1 delivers

```
- CommandScope provider (React context)
- Scope chain resolution (walk up from focused element)
- Command palette UI (CM6 single-line input, single popover for : and Mod+Shift+P)
- Keybinding layer (key → command id → scope resolution)
- useAvailableCommands() hook (for palette and :help)
- Primitive entity mutations: set, delete, create
- Entity changelog (global undo/redo, ~100 entries, transactions for compound commands)
- Bottom bar showing mode indicator
- Global commands: app.save, app.undo, app.redo, app.command,
  app.palette, app.search, app.keymap, app.theme, app.help
```

### Phase 1 scope chain

```
CommandScope (global)
  └─ <AppShell />     ← focused

:help works. Mod+Shift+P shows global commands. : opens same palette.
```

---

## Phase 2: Views

### View definition

```yaml
id: tasks
name: Tasks
icon: check-square
kind: grid
field_set: task_fields
commands: [task.new, task.archive]
```

```yaml
id: tags
name: Tags
icon: tag
kind: grid
field_set: tag_fields
commands: [tags.merge, tags.orphans, tags.recolor, tags.showCards]
```

```yaml
id: board
name: Board
icon: kanban
kind: board
field_set: task_fields
group_by: Status
commands: [board.lane, board.newCard, board.collapseAll, board.expandAll]
```

A view is metadata — not a component. `kind` is a renderer hint. `field_set` names the entity field list. `commands` lists what command ids to add to the scope on mount.

### View registry

Flat list. The app reads it on startup to build the nav and generate `:view` autocomplete.

Adding a view = adding an entry. No routing changes, no layout changes, no new nav code.

### Left nav

Generated from the view registry. Each registered view becomes an icon.

```
┌──────┬──────────────────────────────────────────────────┐
│ NAV  │  VIEW AREA                                       │
│      │                                                  │
│  ☐   │  (active view renders here)                      │
│  #   │                                                  │
│  ▥   │                                                  │
│      │                                                  │
│      │  ┌────────────────────────────────────────────┐  │
│      │  │  -- NORMAL --          Tasks  ▪ sorted: P  │  │
│      │  └────────────────────────────────────────────┘  │
└──────┴──────────────────────────────────────────────────┘
```

Nav behavior:

```
- Click icon → dispatch nav.view command
- Active view highlighted
- Always visible, collapsed to icons
- Does NOT scroll with content
```

### View switching

Switching is a command, not a route change:

```yaml
- id: nav.view
  pattern: ":view <name>"
  autocomplete: view names from registry
  execute: set active view

# Generated aliases
- id: nav.tasks
  alias: ":view tasks"
- id: nav.tags
  alias: ":view tags"
- id: nav.board
  alias: ":view board"
```

What happens on switch:

```
1. Save current view state (scroll, focus, selection) to cache
2. Current view unmounts — its command scope exits the chain
3. Set active view ID
4. New view mounts — its command scope enters the chain
5. Restore cached view state if returning to a previously-visited view
6. Nav highlights new icon
7. Bottom bar updates view name
```

View state is ephemeral, per-session. Lost on reload. (Perspectives handle persistence — phase 5.)

### View-specific commands

Each view declares commands that appear in the palette only when that view is active:

```yaml
# Tasks view
- id: task.new
  name: Create task
  keys: { vim: ":new" }
- id: task.archive
  name: Archive selected
  keys: { vim: ":archive" }

# Tags view
- id: tags.merge
  name: Merge selected
  keys: { vim: "m (visual) / :merge" }
- id: tags.orphans
  name: Show orphans
  keys: { vim: ":orphans" }
- id: tags.recolor
  name: Recolor selected
  keys: { vim: ":color" }
- id: tags.showCards
  name: Cards using tag
  keys: { vim: ":cards" }

# Board view
- id: board.lane
  name: Jump to lane
  pattern: ":lane <status>"
  autocomplete: options from group field
- id: board.newCard
  name: New card
  keys: { vim: ":card new" }
- id: board.collapseAll
  name: Collapse lanes
- id: board.expandAll
  name: Expand lanes
```

### Board navigation

The board has its own spatial model — not a grid:

```
h/l     → move between lanes
j/k     → move between cards within a lane
Enter   → open inspector for focused card
:lane X → jump to lane by name
```

These commands live on the board view's scope. No grid scope in the chain.

The board reads from the field registry:

```
group field    → registry.byName(view.group_by) → options → lanes
card fields    → subset of field set → what to show on each card
lane ordering  → option.order on the group field
card ordering  → current sort state
drag between lanes → updates the group field value on the entity
```

Any select field is a valid board grouping key.

### What phase 2 delivers

```
- View registry (flat YAML list of view definitions)
- Left nav (generated from registry, icon per view)
- View switching (command-based, state cached per view)
- View command scope layer (between global and grid)
- nav.view command + per-view aliases
- Board view renderer with lane/card navigation
- Bottom bar shows active view name
```

### Phase 2 scope chain

```
CommandScope (global)
  ├─ nav.view, nav.tasks, nav.tags, nav.board
  └─ CommandScope (tasks view)          ← active
       ├─ task.new, task.archive
       └─ (view content)     ← focused
```

`:view tags` → resolved at global → switch. `:archive` → resolved at tasks view. Palette shows global + tasks commands.

---

## Phase 3: Grid

### What the grid provides

Both tasks and tags are `kind: grid`. They share everything:

```
- Cell navigation (h/j/k/l, gg, G, 0, $)
- Cell editing (i/Enter → editor, Escape → normal)
- Visual selection (V, shift+j/k)
- Sort and column visibility commands
- DataTable renderer (shared component)
- Cell editors (markdown, select, multi-select, date, color-palette)
- Cell displays (markdown, badge, badge-list, avatar, date, color-swatch, number)
```

What differs per grid view:

```
- field_set → different fields → different columns
- commands → task.new/archive vs tags.merge/orphans/recolor
- data source → task store vs tag store
```

A grid view is fully described by: its field set, its data source, and its view-specific commands.

### Grid commands

```yaml
# Navigation
- { id: grid.up,     keys: { vim: k, cua: "↑", emacs: C-p } }
- { id: grid.down,   keys: { vim: j, cua: "↓", emacs: C-n } }
- { id: grid.left,   keys: { vim: h, cua: "←", emacs: C-b } }
- { id: grid.right,  keys: { vim: l, cua: "→", emacs: C-f } }
- { id: grid.top,    keys: { vim: gg, cua: Mod+Home, emacs: "M-<" } }
- { id: grid.bottom, keys: { vim: G,  cua: Mod+End,  emacs: "M->" } }
- { id: grid.firstCol, keys: { vim: "0", cua: Home, emacs: C-a } }
- { id: grid.lastCol,  keys: { vim: "$", cua: End,  emacs: C-e } }

# Editing
- { id: grid.edit,     keys: { vim: "i / Enter", cua: Enter, emacs: C-o } }
- { id: grid.delete,   keys: { vim: dd, cua: Delete, emacs: C-d } }
- { id: grid.newBelow, keys: { vim: o } }
- { id: grid.newAbove, keys: { vim: O } }
- { id: grid.copy,     keys: { vim: yy, cua: Mod+C, emacs: M-w } }
- { id: grid.paste,    keys: { vim: p,  cua: Mod+V, emacs: C-y } }

# Selection
- { id: grid.selectToggle, keys: { vim: V, cua: Shift+Click, emacs: C-SPC } }
- { id: grid.selectDown,   keys: { vim: "j (visual)", cua: "Shift+↓" } }
- { id: grid.selectUp,     keys: { vim: "k (visual)", cua: "Shift+↑" } }

# Grouping
- { id: grid.expandGroup,   keys: { vim: zo } }
- { id: grid.collapseGroup, keys: { vim: zc } }
- { id: grid.expandAll,     keys: { vim: zR } }
- { id: grid.collapseAll,   keys: { vim: zM } }

# Commands (via palette)
- { id: grid.sort,        pattern: ":sort <field> [asc|desc] [, <field> [asc|desc] ...]" }
- { id: grid.columns,     pattern: ":columns" }
```

`:sort` autocompletes from the active view's field set. The grid scope gets the field set from the view above it.

`:filter` and `:group` are perspective commands, not grid commands — they set JS functions on the active perspective (see phase 5).

`:sort Priority asc` sets a single-field sort stack. `:sort Priority asc, Due asc` sets a multi-field sort stack. Each field uses its comparator from the field definition (or perspective override). Direction is asc or desc. The sort stack replaces any existing sort — it doesn't append.

### Column generation

For each field name in the view's field set:

```
definition   = registry.byName(name)
column id    = definition.id              # ULID — stable for visibility maps
column header= definition.name
column value = row[definition.name]       # name is the data key
column cell  = definition.display
column editor= definition.editor
column sort  = definition.sort
```

### Edit mode

```
Normal → Edit:  grid.edit fires → editor mounts for focused cell
Edit → Normal:  editor signals completion with new value:
  - CM6 with vim: Esc once = vim-insert→vim-normal, Esc again = exit
  - Select/Calendar: value selection or Escape
Commit:         set(owner, field, new_value) logged to undo stack with previous value
```

During editing, `u` is CM6 undo (character-level). After commit, `u` is entity undo (reverts the whole field change). Clean handoff — CM6 session collapses to one entity changelog entry.

The editor type comes from the field definition's `editor` property:

```
markdown      → CM6 with keymap, markdown lang, tag autocomplete
                (single-line or multi-line from field definition)
select        → dropdown from options
multi-select  → multi-pick from options
date          → date picker
color-palette → color swatches
none          → computed field, no editor
```

The markdown editor is always CM6 — same as the palette input, same as the search input. One editor, one keymap, everywhere text goes in.

### Display types

```
markdown      → rendered markdown, #tags as colored pills
badge         → colored pill (status, priority)
badge-list    → multiple colored pills (tags)
avatar        → user avatar + name
date          → formatted ("Today", "Tomorrow", overdue in red)
color-swatch  → colored circle
number        → right-aligned text
text          → plain text
```

### What phase 3 delivers

```
- useGrid hook (navigation, selection, visual mode, edit mode)
- Grid command scope (registered on mount, unregistered on unmount)
- Shared DataTable renderer
- Cell editors (CM6 markdown, select, multi-select, date, color-palette)
- Cell displays (markdown, badge, badge-list, avatar, date, color-swatch, number)
- Column generation from field registry
- :sort command with field autocomplete
```

### Phase 3 scope chain

```
CommandScope (global)
  ├─ nav.view, app.save, app.help, ...
  └─ CommandScope (tasks view)
       ├─ task.new, task.archive
       └─ CommandScope (grid)
            ├─ grid.up, grid.down, grid.edit, grid.sort, ...
            └─ <DataTable />     ← focused
```

`k` → grid.up → move row focus up.
`:sort Priority asc` → grid.sort → sort by priority.
`:archive` → not in grid → resolved at tasks view → archive.
`:view tags` → not in grid or tasks → resolved at global → switch.
Palette shows all three layers of commands.

---

## Phase 4: Inspector

### What the inspector is

A generic property sheet. Takes a field set and an entity, renders property rows. Doesn't know about tasks or tags.

```
for each name in field_set:
  definition = registry.byName(name)
  value      = entity[name]                 # or derive for computed
  render:
    label    = definition.name
    display  = component from definition.display
    editor   = component from definition.editor (if editor != none)
```

Header and footer slots for entity-specific chrome:

```
Task inspector:  footer = subtask checklist
Tag inspector:   header = tag preview pill, footer = usage list
```

The core property sheet is entity-agnostic.

### Inspector scope

The inspector adds a command scope that **shadows** grid navigation and **blocks** destructive grid commands:

```yaml
# Shadows — same command id, different behavior
- id: grid.up
  execute: move to previous property row
- id: grid.down
  execute: move to next property row
- id: grid.edit
  execute: edit focused property

# Blocks — command exists but unavailable
- id: grid.delete
  available: false    # prevents accidental row deletion while inspecting
```

Commands NOT registered by the inspector pass through:

```
grid.left, grid.right     → pass to grid (not meaningful in inspector)
grid.sort    → pass to grid
tags.merge, task.archive  → pass to view scope
nav.view, app.save        → pass to global
```

### Sibling inspectors

Multiple inspectors are siblings, not nested:

```
CommandScope (grid)
  ├─ <DataTable />
  ├─ CommandScope (inspector A)
  │    └─ <Inspector entity="frontend" />
  └─ CommandScope (inspector B)     ← focused
       └─ <Inspector entity="backend" />
```

Resolution walks up from whichever branch has DOM focus. Click inspector A → A's scope resolves. Click the table → both inspector scopes are on sibling branches, irrelevant.

### What phase 4 delivers

```
- Inspector component (generic property sheet from field set)
- Inspector command scope (shadows grid.up/down/edit, blocks grid.delete)
- Header/footer slots for entity-specific chrome
- Sibling inspector support (focus-based resolution)
- PropertyRow renderer (label + display/editor from field definition)
```

### Phase 4 scope chain

```
CommandScope (global)
  └─ CommandScope (tags view)
       ├─ tags.merge, tags.orphans, tags.recolor
       └─ CommandScope (grid)
            ├─ grid.up, grid.down, grid.edit, grid.sort, ...
            ├─ <DataTable />
            └─ CommandScope (inspector)     ← focused
                 ├─ grid.up (shadows), grid.down (shadows)
                 ├─ grid.edit (shadows), grid.delete (blocked)
                 └─ <Inspector />
```

`k` → grid.up → resolved at inspector → move between properties.
`dd` → grid.delete → resolved at inspector → blocked.
`:merge` → not in inspector → not in grid → resolved at tags view.
`:view board` → walks to global → switch.
Palette shows: inspector-available commands + grid (minus blocked) + view + global.

---

## Phase 5: Perspectives

### What a perspective is

A perspective is a named, ordered list of fields with per-field overrides, plus a filter function and a group function. Not an entity. Own storage, own changelog.

```yaml
name: "Active Sprint"
view: board

fields:
  - field: 01JMTASK0000000000TITLE00      # ULID — survives renames
  - field: 01JMTASK0000000000STATUS0
    width: 150                              # override field definition's 120
  - field: 01JMTASK0000000000PRIORTY
    caption: "P"                            # override column header
    width: 60
  - field: 01HQ3USERFIELD00000SPRINT
    display: text                           # override badge with plain text

filter: (entity) => entity.Status !== "Done" && entity.Sprint === "Sprint 23"
group: (entity) => entity.Status
sort:
  - field: 01JMTASK0000000000PRIORTY
    direction: asc
  - field: 01JMTASK0000000000DUEDAT0
    direction: asc
```

The `fields` list is the column order. Each entry references a field by ULID and can override any display property from the base definition:

```
caption     override column header (default: field name)
width       override column width (default: field definition width)
editor      override editor type
display     override display type
sort        override sort comparator (default: field definition's comparator)
```

Properties not overridden fall through to the field definition. The resolution chain:

```
perspective field override → field definition default
```

### Sort: two levels

**Field-level sort** — a comparator function. How do you compare two values of this field? Defaults to lexical. Select fields use option-order. Dates use datetime. Numbers use numeric. The field definition declares this. A perspective field override can change it (e.g. sort a select field alphabetically instead of by option.order).

**Perspective-level sort** — an ordered list of `(field, direction)` pairs. The sort stack. "Sort by Priority ascending, then by Due ascending." Each entry uses its field's comparator (or the perspective override of that comparator), applied in the specified direction.

```yaml
sort:
  - field: 01JMTASK0000000000PRIORTY       # uses Priority's comparator (option-order)
    direction: asc                           # P0 first
  - field: 01JMTASK0000000000DUEDAT0       # uses Due's comparator (datetime)
    direction: asc                           # earliest first
```

`:sort Priority asc` sets a single-field sort. `:sort Priority asc, Due asc` sets a multi-field sort. Each field name resolves to a ULID for storage.

### Filter and group are functions

No predicate DSL. No JSON operators. Filter and group are JS functions over the entity list.

```typescript
// Filter: receives entity, returns boolean
filter: (entity) => entity.Status !== "Done"

// More complex filter
filter: (entity) => {
  const due = new Date(entity.Due)
  return due < new Date() && entity.Status !== "Done"
}

// Group: receives entity, returns group key
group: (entity) => entity.Priority

// Custom grouping
group: (entity) => {
  const due = new Date(entity.Due)
  if (due < new Date()) return "Overdue"
  if (due < tomorrow()) return "Today"
  return "Upcoming"
}
```

Filter returns a boolean — entity is included or not. Group returns a string — the lane label on a board, the group header in a grid. Both have full access to the entity. Arbitrary logic — date ranges, computed conditions, string matching, whatever JS can do.

No filter or group specified → show all entities, no grouping.

### Why functions, not predicates

A predicate DSL (`{ field: Status, operator: not-eq, value: Done }`) looks clean in YAML but:

```
- Always ends up reimplementing a subset of JS, badly
- Can't express date math, regex, computed conditions
- Needs an interpreter that maps operator strings to functions
- Autocomplete and validation become DSL problems instead of JS problems
- Combining conditions requires AND/OR/NOT tree structures
```

A JS function is the predicate. The runtime is the interpreter. No translation layer.

For the command palette, `:filter` launches a CM6 input where the user writes (or edits) the filter expression. Autocomplete offers field names. The function is stored as a string and evaluated.

### Perspective vs. view state

```
View state:    ephemeral, per-session. Scroll position, focused cell, selection.
               Lost on reload. Cached when switching views.

Perspective:   named config. Ordered field list with overrides, filter fn, group fn, sort.
               Own storage, own changelog. Survives reload.
               Field ULIDs survive renames.
```

### Perspective changelog

Separate undo/redo stack from the entity changelog:

```yaml
create:  { id, perspective }                    # whole object
update:  { id, property, value, previous }      # one property changed
delete:  { id, previous }                       # whole object for undo
```

`:perspective save` logs a `create`. Editing a perspective's filter logs an `update`. Deleting logs a `delete` with the full object. Independent from entity undo — the two never interleave.

### Perspective commands

```yaml
- id: perspective.load
  pattern: ":perspective <n>"
  autocomplete: perspective names from perspective store
  execute: switch to perspective's view + apply its config

- id: perspective.save
  pattern: ":perspective save <n>"
  execute: snapshot current state, create perspective

- id: perspective.delete
  pattern: ":perspective delete <n>"
  execute: delete perspective, log to perspective changelog

- id: perspective.filter
  pattern: ":filter"
  execute: open CM6 input to write/edit JS filter function

- id: perspective.clearFilter
  pattern: ":nofilter"
  execute: clear filter function

- id: perspective.group
  pattern: ":group"
  execute: open CM6 input to write/edit JS group function

- id: perspective.clearGroup
  pattern: ":nogroup"
  execute: clear group function
```

These live on the global scope. `:filter` and `:group` open a CM6 input where the user writes a JS function. Autocomplete offers field names from the active view's field set.

### Load perspective flow

```
1. Read perspective from perspective store
2. Switch to perspective's view (triggers view switch flow)
3. Apply fields list: set column order, apply per-field overrides
4. Apply filter function: filter entity list
5. Apply group function: group entities (lanes on board, group rows in grid)
6. Apply sort: resolve ULIDs → field definitions → sort state
```

### Save perspective flow

```
1. Snapshot current state:
   - view: current active view ID
   - fields: current visible columns in order, with any width/display/sort overrides
   - filter: current filter function (if any)
   - group: current group function (if any)
   - sort: current sort stack (field ULIDs + directions)
2. Create perspective object with user-provided name
3. Write to perspective store
4. Log create to perspective changelog
```

### Column rendering with perspectives

When a grid renders columns, it merges the perspective's field list with the field registry:

```
for each entry in perspective.fields:
  definition = registry.byId(entry.field)         # base definition
  
  column.id      = definition.id                   # ULID
  column.header  = entry.caption  ?? definition.name
  column.width   = entry.width    ?? definition.width
  column.editor  = entry.editor   ?? definition.editor
  column.display = entry.display  ?? definition.display
  column.sort    = entry.sort     ?? definition.sort ?? lexical
  column.value   = row[definition.name]
```

Then the perspective's sort stack is applied:

```
for each { field, direction } in perspective.sort:
  comparator = resolved column.sort for that field    # from merge above
  apply comparator with direction (asc = natural, desc = inverted)
```

No perspective active → columns come from the view's field_set with definition defaults, no sort applied.

### What phase 5 delivers

```
- Perspective as named config (ordered field list + overrides + filter fn + group fn + sort stack)
- Perspective store (separate from entity store)
- Perspective changelog (separate undo/redo from entity mutations)
- Per-field overrides: caption, width, editor, display, sort comparator
- Sort stack: ordered list of (field, direction) using field-level comparators
- Filter and group as JS functions (no predicate DSL)
- perspective.load, perspective.save, perspective.delete commands
- perspective.filter, perspective.group commands (set JS functions via CM6 input)
- perspective.clearFilter, perspective.clearGroup commands
- Column rendering merge (perspective override → field definition default → lexical)
```

---

## Full Architecture

### Scope chain (all phases)

```
CommandScope (global)                              ← phase 1
  ├─ app.save, app.undo, app.redo, app.command, app.palette, app.search, app.help
  ├─ nav.view, nav.tasks, nav.tags, nav.board      ← phase 2
  ├─ perspective.load, perspective.save, perspective.filter, perspective.group  ← phase 5
  │
  └─ CommandScope (active view)                     ← phase 2
       ├─ task.new, task.archive  (or tags.merge, board.lane, ...)
       │
       └─ CommandScope (grid)                       ← phase 3
            ├─ grid.up, grid.down, grid.edit, grid.sort, ...
            ├─ <DataTable />
            │
            └─ CommandScope (inspector)             ← phase 4
                 ├─ grid.up (shadows), grid.delete (blocked)
                 └─ <Inspector />
```

### Layout

```
┌──────┬──────────────────────────────────────────────────┐
│ NAV  │                                                  │
│      │  ┌────────────────────────────────────────────┐  │
│  ☐   │  │                                            │  │
│  #   │  │  Active view (grid or board)               │  │
│  ▥   │  │                                            │  │
│      │  └────────────────────────────────────────────┘  │
│      │                                                  │
│      │  ┌────────────────────────────────────────────┐  │
│      │  │  Inspector (optional)                      │  │
│      │  └────────────────────────────────────────────┘  │
│      │                                                  │
│      │  ┌────────────────────────────────────────────┐  │
│      │  │  -- NORMAL --          Tasks  ▪ sorted: P  │  │
│      │  └────────────────────────────────────────────┘  │
└──────┴──────────────────────────────────────────────────┘

Left nav:    from view registry (phase 2)
View area:   grid (phase 3) or board (phase 2)
Inspector:   generic property sheet (phase 4)
Bottom bar:  mode + view name + sort/filter indicator (phase 1+)
Palette:     popover, same UI for : and Mod+Shift+P (phase 1)
```

### Resolution examples

```
User is in tasks grid, inspector open, inspector focused:

  k         → grid.up      → inspector (shadows) → move between properties
  dd        → grid.delete  → inspector (blocked)  → nothing
  :archive  → task.archive → tasks view scope     → archive selected
  :sort P   → grid.sort    → grid scope           → sort by priority
  :view tags→ nav.view     → global scope         → switch to tags
  u         → app.undo     → global scope         → revert last changelog entry
  Mod+Shift+P             → palette shows all available commands

User closes inspector, table focused:

  k         → grid.up      → grid scope           → move row up
  dd        → grid.delete  → grid scope           → delete row
  :archive  → task.archive → tasks view scope     → archive

User switches to board:

  l         → board.right  → board view scope     → next lane
  :lane Done→ board.lane   → board view scope     → jump to Done lane
  :view tasks→ nav.view    → global scope         → switch back
```

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Command system | Composable scopes nesting with component tree | Mount = register. Unmount = gone. |
| Resolution | Focused branch, nearest ancestor wins | Deepest for nested; focus-based for siblings. |
| Shadowing | Same command id at deeper scope | Inspector's grid.up moves between fields, not rows. |
| Blocking | available: false stops upward walk | Inspector blocks grid.delete. |
| Pass-through | Don't register = passes to parent | Inspector doesn't capture :merge. |
| Mutations | Three entity primitives: set, delete, create | Every entity command decomposes. Entity changelog operates on one shape. |
| Undo | Two changelogs: entity (global, ~100 entries) and perspective (separate) | Entity and perspective mutations never interleave. `u` applies to most recent. |
| CM6 undo vs app undo | CM6 owns character-level within edit session; entity changelog owns command-level | Committing an edit produces one entity changelog entry from the CM6 session. |
| Command UI | Single palette popover for both `:` and `Mod+Shift+P` | One UI, two entry points. Fuzzy search, scope-aware, keybinding hints. |
| Text input | CM6 single-line everywhere: palette, search, cell editors, inspector | One keymap contract. Vim/emacs/CUA works in every text context. |
| Keybindings | Global table: key → command id, per keymap mode | Static config. Dynamic resolution via scope chain. |
| View registration | Flat YAML list of definitions | Nav and aliases generated. New view = new entry. |
| View switching | Command, not routing | State cached per view. Scope chain handles command availability. |
| Nav | Generated from view registry | Never hard-codes views. |
| Grid sharing | useGrid hook, shared DataTable, shared editors/displays | Tasks and Tags differ only in field set and view commands. |
| Board | Own renderer, own navigation, same field registry | Lanes from select options. h/l j/k. Inspector shared. |
| Inspector | Generic property sheet from field set | Entity-agnostic. Header/footer slots for entity chrome. |
| Scope layering | global → view → grid → inspector | Each layer adds commands. Each layer is independent. |
| Perspectives | Named ordered field list + per-field overrides + filter/group fns | Own storage, own changelog. Override any display prop. JS functions, not predicate DSL. |
| View state | Ephemeral, per-session | Not worth persisting. Perspectives handle the persistent case. |

See also: [Field Registry Architecture](field-registry-architecture.md) for field definitions, EAV data model, and field operations.
