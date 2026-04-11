---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffb180
project: expr-filter
title: Update stale highlight.ts docstring — Tag and Mention are no longer styled by styleTags
---
**File:** kanban-app/ui/src/lang-filter/highlight.ts (lines 1-6)

**Severity:** nit

**What:** The file-level docstring says:

> "Maps grammar node types to CodeMirror highlight tags so that tags, mentions, refs, and operators each render in distinct colors within the editor theme."

This is stale. The new inline comment inside `styleTags({...})` correctly explains that Tag, Mention, and Project are intentionally omitted from `styleTags` because they get their colors from the pill decoration system (via `createMentionDecorations`). But the file-level docstring still asserts the old behavior — that tags and mentions are highlighted here.

**Suggestion:** Rewrite the docstring to match the current behavior. Example:

```ts
/**
 * Syntax highlighting mapping for the filter DSL grammar.
 *
 * Maps Ref and operator nodes to CodeMirror highlight tags. Tag, Mention,
 * and Project nodes are intentionally NOT mapped here — they get their
 * colors from the mention decoration system (colored pills) in
 * `cm-mention-decorations.ts`. Adding them to `styleTags` would cause
 * `defaultHighlightStyle` to override the entity pill colors.
 */
```

**Subtasks:**
- [ ] Rewrite the file-level JSDoc in kanban-app/ui/ui/src/lang-filter/highlight.ts to list only the node types actually styled (Ref + operators) and explicitly note Tag/Mention/Project are owned by the decoration system
- [ ] Verification: `pnpm --filter swissarmyhammer-kanban-ui test` — existing highlight tests still pass
#review-finding #expr-filter