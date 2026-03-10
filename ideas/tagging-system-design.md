# Tagging System Design

## Core Philosophy

Tags are first-class objects with metadata, but creation is zero-friction. You never fill out a form to make a tag — you type `#sometag` in markdown and it exists. Enrichment (color, description) happens later, if and when you care.

Emoji replaces icons. `#🔥urgent` or `#🐛bug` carries its own visual weight without needing a separate icon field.

---

## Tag Object

```typescript
interface Tag {
  id: string          // the tag itself, normalized lowercase
  color: string       // auto-assigned on creation, user-overridable
  description: string // shown on hover tooltip, empty by default
}
```

Three fields. The kanban engine's change log handles provenance — when the tag was created, who changed the color, history of edits. The tag object itself stays minimal.

---

## Single Source of Truth: Markdown

There is no separate `tags[]` stored on a card. **The markdown body is the only place tags live.** The `tags` property on a card is purely computed — derived by parsing the body for `#tag` patterns every time it's read.

```typescript
function getEffectiveTags(card: Card): string[] {
  return parseTagsFromMarkdown(card.body)
}
```

Every consumer — table columns, filters, board swimlanes, grouping, search — calls `getEffectiveTags()`. Nothing ever writes to a `tags` array directly.

### Adding a tag via command

When someone uses the "add tag" command (keyboard shortcut, table cell editor, command palette) rather than typing inline, **the command appends `#tag` to the markdown body.** This is the only write path.

Before:
```markdown
Fix the login redirect loop that happens
when the session token expires mid-request
```

After adding `bug` via command:
```markdown
Fix the login redirect loop that happens
when the session token expires mid-request
#bug
```

After adding `auth` via command:
```markdown
Fix the login redirect loop that happens
when the session token expires mid-request
#bug #auth
```

Tags added by command accumulate at the bottom. Tags typed inline by the user stay wherever the user put them. The parser doesn't care about position — it scans the whole body.

### Removing a tag

Removing a tag is a text operation — find `#bug` in the body, delete it. One code path regardless of how the tag got there. No hidden state, no provenance ambiguity.

### Why not two sources?

An earlier design considered a stored `tags[]` array for "explicit" tags (added via UI) unioned with inline tags parsed from markdown. This was rejected because:

- Two sources means provenance ambiguity: which pile does a tag live in? Can you remove it?
- "Remove tag" has to know where the tag came from to decide what to do
- Hidden metadata tags that don't appear in the card body violate the markdown-native principle

One source, one parse path, no surprises. If a tag applies to the card, it's visible in the text.

---

## Tag Parsing

### Disambiguation from headings

Markdown `#` is already a heading. The rule every app uses (Obsidian, Bear, GitHub): `# ` with a trailing space is a heading, `#word` with no space and attached to a word character is a tag.

### Legal characters

A tag name can contain: alphanumeric characters, hyphens, underscores, forward slashes (for hierarchy), and emoji. It stops at spaces, punctuation like `.,:;!?()` and other special characters.

### Regex

```
(?<=^|[\s])#([\w\p{Emoji_Presentation}\p{Extended_Pictographic}][\w\p{Emoji_Presentation}\p{Extended_Pictographic}/\-]*)
```

Lookbehind for start-of-line or whitespace (prevents matching inside URLs or hex colors), then `#` followed by at least one word/emoji character, then any continuation of word chars, emoji, slashes, or hyphens.

### Hierarchy via naming convention

Following Bear and Obsidian's pattern: `#frontend/css` creates a `frontend` parent tag (if it doesn't exist) and a `css` child tag nested under it. Hierarchy is typed into existence, not configured in a settings panel. But because tags are first-class objects, the parent/child relationship can be stored structurally — renaming "frontend" doesn't break nesting.

---

## Tag Object Lifecycle

When the markdown parser encounters `#sometag`:

1. **Normalize:** lowercase, trim
2. **Lookup:** check the tag store (`Map<string, Tag>` or state management equivalent)
3. **If exists:** use it for color/description in rendering
4. **If new:** create a `Tag` object with auto-assigned color, empty description, and add to the store

### Auto-color assignment

Hash the tag name to an index into a curated palette. Deterministic — same tag always gets the same color until the user overrides it.

```typescript
const TAG_PALETTE = [
  '#ef4444', '#f97316', '#eab308', '#22c55e',
  '#14b8a6', '#3b82f6', '#8b5cf6', '#ec4899',
  '#6366f1', '#06b6d4', '#84cc16', '#f43f5e',
]

function autoColor(tagName: string): string {
  let hash = 0
  for (const char of tagName) {
    hash = ((hash << 5) - hash + char.charCodeAt(0)) | 0
  }
  return TAG_PALETTE[Math.abs(hash) % TAG_PALETTE.length]
}
```

---

## CodeMirror 6 — Inline Tag Rendering

Tags in the editor render as colored pills inline. This is a CM6 `ViewPlugin` with `DecorationSet`.

### Decoration approach

The plugin scans the document (or just visible ranges for performance) for the tag regex, then applies `Decoration.mark()` with a CSS class. The tag's color from the store is injected as a CSS custom property on the mark element:

```css
.cm-tag-mark {
  background: color-mix(in srgb, var(--tag-color) 15%, transparent);
  border-radius: 3px;
  padding: 1px 4px;
}

.cm-tag-mark .cm-tag-hash {
  opacity: 0.5;
}
```

The `style` attribute on the decoration sets `--tag-color` per-tag:

```
style="--tag-color: #4a9eff"
```

### Hover tooltip

CM6's `hoverTooltip` extension. Register a function that fires when the cursor hovers over a range:

1. Check if the range has the tag decoration class
2. Look up the tag in the store
3. If description is non-empty, return a tooltip DOM element
4. If description is empty, return `null` — no blank bubble

CM6's tooltip system handles positioning, viewport collision, and dismissal automatically.

### Double-click → Tag Inspector

A CM6 `EditorView.domEventHandlers({ dblclick })` handler:

1. Check if the click target has the tag class
2. Extract the tag name from the text content
3. Open the inspector popover (React, portaled to body, anchored to the tag's DOM rect)

The hover tooltip dismisses naturally when the mouse stops hovering. The inspector is a separate popover managed by React. No stacking.

---

## React-Markdown — Display Mode Tag Rendering

For display contexts (table cells, card previews, board cards), react-markdown needs a custom remark plugin.

### Remark plugin approach

A remark plugin that walks text nodes in the mdast (markdown abstract syntax tree), finds `#tag` patterns, and replaces them with a custom node type. The react-markdown component map renders that node type as a styled `<span>`:

```tsx
function TagPill({ tagId }: { tagId: string }) {
  const tag = useTagStore(tagId)
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          className="inline-flex items-center rounded px-1.5 py-0.5 text-xs font-medium"
          style={{
            backgroundColor: `color-mix(in srgb, ${tag.color} 15%, transparent)`,
            color: tag.color,
          }}
          onDoubleClick={() => openTagInspector(tagId)}
        >
          #{tag.id}
        </span>
      </TooltipTrigger>
      {tag.description && (
        <TooltipContent>{tag.description}</TooltipContent>
      )}
    </Tooltip>
  )
}
```

Uses shadcn's `<Tooltip>` for styled hover descriptions that match the rest of the UI. Double-click opens the same inspector as the CM6 version.

---

## Tag Inspector

A small popover anchored to the tag pill. Not a modal, not a sidebar. Three fields:

### Tag name (id)

Editable text input showing the tag's `id`. If renamed, every occurrence of `#oldname` in every card body gets find-and-replaced with `#newname`. The tag object's `id` updates in the store. This is a refactor operation — like "rename symbol" in an IDE. The kanban engine's change log records the rename.

### Description

Small textarea. Empty by default. Shows on hover as a tooltip anywhere the tag appears (both in CM6 editor and react-markdown display mode).

### Color

A row of swatches from the 12-16 color palette, plus a custom color picker. The current color is highlighted. Changing it updates the tag in the store; every rendered instance of that tag updates reactively.

That's it. No icons (emoji in the tag name handles that), no group assignment (can be added later), no usage stats in the inspector (the engine can surface those elsewhere).

---

## Tag Autocomplete

CodeMirror 6's `@codemirror/autocomplete` provides the completion system. A custom completion source handles tag suggestions.

### Trigger

The completion source fires when the cursor is preceded by `#` plus one or more word characters. Use `context.matchBefore()` with the tag regex to grab the partial tag. If no match, return `null` and nothing happens.

```typescript
function tagCompletionSource(context: CompletionContext): CompletionResult | null {
  const match = context.matchBefore(/#[\w\p{Emoji_Presentation}\p{Extended_Pictographic}][\w\p{Emoji_Presentation}\p{Extended_Pictographic}/\-]*/u)
  if (!match) return null

  const fragment = match.text.slice(1).toLowerCase() // strip #
  const allTags = getTagStore().getAll()

  const matches = allTags
    .filter(tag => tag.id.includes(fragment))
    .sort((a, b) => {
      // prefix matches first, then substring matches
      const aPrefix = a.id.startsWith(fragment) ? 0 : 1
      const bPrefix = b.id.startsWith(fragment) ? 0 : 1
      return aPrefix - bPrefix || a.id.localeCompare(b.id)
    })
    .map(tag => ({
      label: `#${tag.id}`,
      apply: `#${tag.id}`,
      detail: tag.description || undefined,
      // custom rendering to show color dot
    }))

  // "Create new" option if no exact match
  const exactMatch = allTags.some(t => t.id === fragment)
  if (!exactMatch && fragment.length > 0) {
    matches.push({
      label: `Create #${fragment}`,
      apply: `#${fragment}`,
      // side effect: create tag in store on apply
    })
  }

  return {
    from: match.from,
    options: matches,
    filter: false, // we handle filtering ourselves
  }
}
```

### Custom rendering in the dropdown

Each completion option shows a colored dot (or swatch) next to the tag name. CM6 autocompletion supports this via the `addToOptions` configuration or the `render` property on individual completions. The dot's color comes from the tag's `color` field in the store.

### Create-on-select

If the user selects the "Create #whatever" option:

1. A new `Tag` object is created in the store (auto-color, empty description)
2. `#whatever` is inserted into the document (standard CM6 completion behavior)
3. The decoration plugin picks it up on the next scan and renders the pill

No extra step. The tag exists and is rendered in one action.

---

## Renaming a Tag

Renaming is the most complex operation because tags live in markdown text, not a structured field. When a tag is renamed in the inspector:

1. **Update the tag store:** change the `id` on the `Tag` object (or delete old, create new with same color/description)
2. **Find-and-replace across all card bodies:** every occurrence of `#oldname` in every card's markdown body becomes `#newname`
3. **The kanban engine's change log** records the rename with before/after

This is a batch text mutation. The engine should expose this as a single atomic operation so it's one undo step.

---

## Design Decisions Summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Tag data model | `{ id, color, description }` | Minimal. Engine change log handles timestamps/history |
| Icons | No — use emoji in tag name | `#🐛bug` is more expressive than a separate icon field |
| Source of truth | Markdown body only | One parse path, no provenance ambiguity |
| Stored `tags[]` array | Computed, never written | Derived from parsing body on every read |
| Adding tag via command | Appends `#tag` to body | Every write path goes through markdown |
| Removing a tag | Delete `#tag` from body text | One code path for all removals |
| Tag hierarchy | `/` separator in name | `#frontend/css` creates parent/child automatically |
| Auto-color | Hash name → palette index | Deterministic, no user action needed at creation |
| Tag inspector | Popover on double-click | Rename, description, color — nothing else |
| Hover tooltip | Description only | Shows when description is non-empty |
| Autocomplete | CM6 completion source on `#` + char | Fuzzy match existing tags, "Create new" at bottom |
| Inline rendering | CM6 `Decoration.mark()` with CSS custom prop for color | Colored pills in editor |
| Display rendering | Remark plugin → custom React component | Colored pills in react-markdown output |
