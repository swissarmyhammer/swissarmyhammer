---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffa80
project: expr-filter
title: Asymmetric --project-color CSS fallback — add --tag-color / --actor-color or drop --project-color
---
**File:** kanban-app/ui/src/index.css (line 77)

**Severity:** nit

**What:** This commit adds a single CSS custom property fallback at :root level:

```css
/* Mention decoration pill colors.
 * Per-element colors are set inline by cm-mention-decorations.ts based on
 * entity data; these variables provide neutral fallbacks for pills whose
 * entity data has not yet loaded (or is missing). Mirror this pattern if
 * you add a new mentionable entity type. */
--project-color: #888888;
```

But there is no corresponding `--tag-color` or `--actor-color` in `:root`. Only `--project-color` is declared. Grep confirms: the only such fallback in the entire src tree is the new `--project-color`.

The comment tells the next contributor "Mirror this pattern" — but the pattern was not mirrored for the already-existing `tag` and `actor` entity types, which also go through `cm-mention-decorations.ts`. So the comment is aspirational rather than descriptive.

**Why it does not break anything today:** `buildMentionTheme()` in `kanban-app/ui/src/lib/cm-mention-decorations.ts` line 138-140 already provides inline fallbacks:

```ts
backgroundColor: `color-mix(in srgb, var(${colorVar}, #888) 20%, transparent)`,
color: `var(${colorVar}, #888)`,
border: `1px solid color-mix(in srgb, var(${colorVar}, #888) 30%, transparent)`,
```

The `#888` inline fallback covers the "variable not set" case for every entity type. So `--project-color: #888888` in :root is functionally redundant — it produces the same pixel result as the inline fallback.

**Resolution options (pick one):**
1. **Remove `--project-color` from index.css** — the inline `#888` fallback in `buildMentionTheme` already provides the same visual result. Fewer cross-file couplings.
2. **Add `--tag-color` and `--actor-color` (and any future mentionable type) alongside `--project-color`** and make this the actual convention. If the goal is to centralize the neutral-gray fallback color so it can be tweaked in one place, all mentionable types should share the convention.

Option 1 is minimal and matches current behavior. Option 2 is a small refactor that makes the comment accurate.

**Subtasks:**
- [ ] Decide between removing `--project-color` or adding `--tag-color` / `--actor-color` siblings
- [ ] Apply the chosen option in kanban-app/ui/src/index.css
- [ ] Verification: `pnpm --filter swissarmyhammer-kanban-ui test` passes and manually eyeball a filter editor showing `#tag @user $project` — pills should render identically before and after
#review-finding #expr-filter