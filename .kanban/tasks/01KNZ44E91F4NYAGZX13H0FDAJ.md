---
assignees:
- wballard
depends_on:
- 01KNZ432XEGXX0VWPFSDXXXG32
- 01KNZ42426TGY4AVSDMQMRFQGS
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff9a80
project: pill-via-cm6
title: 'MentionView component: ID → CM6 pill renderer'
---
## What

Create the React component that replaces `MentionPill`. Takes one or more `{entityType, id}` references, looks up the entities, synthesizes a CM6 doc string like `$slug` or `$s1 $s2 $s3`, and mounts a `TextViewer` with mention extensions so CM6 widget-renders the pills with display names.

This is what `BadgeDisplay` (reference branch), `BadgeListDisplay` (reference branch), and `MarkdownDisplay` (in a future card) all delegate to.

**Files to create:**
- `kanban-app/ui/src/components/mention-view.tsx`

**Two input shapes (single component supports both):**
```ts
interface MentionViewProps {
  /** Single mention (scalar reference field) */
  entityType?: string;
  id?: string;
  /** Multiple mentions (list reference / computed tag list / multiple inline) */
  items?: Array<{ entityType: string; id?: string; slug?: string }>;
  /** Optional className passed through to TextViewer */
  className?: string;
  /**
   * Predicates the containing FocusScope uses to claim focus on nav commands.
   * Passed through for keyboard navigation integration.
   */
  claimWhen?: ClaimPredicate[];
  focusMoniker?: string;
  showFocusBar?: boolean;
  /** Extra commands (e.g. task.untag) added to the context menu */
  extraCommands?: CommandDef[];
}
```

**Rendering pipeline:**
1. Resolve each `{entityType, id | slug}` to its mention-text form:
   - Look up entity in entity store by id (or find by slugified displayField if slug is provided).
   - Read `mention_display_field` from schema for the entity type.
   - Get the entity's prefix from `mentionableTypes`.
   - Slugify the display-field value and build `${prefix}${slug}`.
   - For missing entities (stale id / unknown slug), emit `${prefix}${rawValue}` — the CM6 widget pipeline already handles unknown slugs with muted mark styling (from the widget card).
2. Join the mention strings with a single space: `"$proj-a ^task-b @alice"`.
3. Build a scoped mention extension array that covers only the entity types present in `items` — saves bundling unrelated decoration infra for a single-pill render.
4. Wrap the `TextViewer` in the same `FocusScope` + `EntityCommands` machinery `MentionPill` uses today. The FocusScope provides click-to-focus, context menu, and keyboard nav; CM6 just renders the visible pill.

**Integration with existing React wiring (carry over from `MentionPill`):**
- `useEntityCommands` for the per-entity context menu (entity.inspect, etc.).
- Per-pill focus moniker (use the entity moniker, or `focusMoniker` override for list contexts).
- `taskId` + `extraCommands` path for task-scoped extras (e.g. `task.untag`). When there are multiple items, the untag command applies per-item — pass it through correctly, or punt on that for list mode and only support extras for single-mention mode.
  - For list mode, the old `BadgeListDisplay` supported per-pill focus monikers and per-pill claim predicates. We need to keep that working. This may mean rendering one `TextViewer` per item for the list case (so each gets its own FocusScope). That's fine — each is a one-line one-mention viewer.

**Two internal modes:**
- **Single mode** (`entityType` + `id`): one FocusScope wrapping one `TextViewer` with a one-mention doc.
- **List mode** (`items`): a flex container with one FocusScope per item, each containing a `TextViewer` with its single mention. This preserves per-pill keyboard nav (nav.left/nav.right between items).

**Stale entities (answer 3 from planning):** If lookup fails, the fallback is the raw slug (or raw id if even the slug isn't known) with muted mark styling. The widget-card's fallback path already handles this inside CM6; MentionView just has to pass the raw string through.

## Acceptance Criteria
- [ ] New component `MentionView` at `components/mention-view.tsx`
- [ ] Single-mode renders one `TextViewer` with one mention string, wrapped in a FocusScope
- [ ] List-mode renders multiple `TextViewer`s in a `flex flex-wrap gap-1.5` container, one FocusScope per item
- [ ] Visible pill text matches what CM6's mention widget produces (clipped display name, not slug)
- [ ] Click on a pill focuses its FocusScope; right-click opens the correct entity context menu
- [ ] Unknown id → raw slug muted pill (visual regression must show muted styling)
- [ ] Keyboard nav.left/nav.right moves focus between list items when rendered in full mode

## Tests
- [ ] `kanban-app/ui/src/components/mention-view.test.tsx` (new) — render single mode with a known project id, assert the widget's DOM text equals the clipped display name
- [ ] Render single mode with an unknown id, assert the muted raw-slug styling is present
- [ ] Render list mode with 3 items across 2 different entity types, assert each pill appears with correct display name
- [ ] Render list mode, simulate nav.right from the first pill, assert focus moves to the second pill
- [ ] Render with a `taskId` + extraCommands; right-click simulation → context menu shows "Remove Tag" (or whatever the caller passed)
- [ ] Run: `bun test mention-view` — all pass

## Workflow
- Use `/tdd` — start with the simplest test (single mode with known id), then unknown id, then list mode, then keyboard nav. Each gets its own red → green cycle.
