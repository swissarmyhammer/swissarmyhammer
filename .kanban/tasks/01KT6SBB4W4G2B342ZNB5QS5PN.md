---
assignees:
- claude-code
depends_on:
- 01KT6SAXCBZFE6S0DEPZDJSQAA
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffee80
project: short-ids
title: 'Short IDs: CM6 — render ^<short> in bodies as task pills'
---
Parse `^`-references inside CM6 editor/display bodies and decorate them as task pills. The pill LABEL is the short form `^<short>`; the task TITLE shows in the pill's hover tooltip. Resolves BOTH short (`^8rfp1r`) and full-ULID (`^01KT4CNAYW7JG0X8F8W28RFP1R`) forms; both display as the same `^<short>` pill.

## Intentional asymmetry
Unlike `#tag`/`@actor`/`$project` pills (which show the entity NAME as the label), a `^task` pill shows the short ID as the label and puts the title in the tooltip — task titles are long sentence-like strings, not short handles. Do not "fix" this to show the title inline.

## Background (from scoping)
- Generic pill stack exists and is prefix-parameterized: `cm-mention-decorations.ts` (ViewPlugin + Decoration.replace/mark + atomicRanges), `cm-mention-widget.ts` (`MentionWidget`, renders `${prefix}${clipDisplayName(displayName)}`), `mention-finder.ts` (`findMentionsInText`), `mention-meta.ts` (`MentionMeta { color, displayName, description }`), plus an existing tooltip path.
- Editable task description mounts at `components/fields/registrations/markdown.tsx`; read-only display at `fields/displays/markdown-display.tsx`.
- `depends_on` (full ULIDs) already renders as pills via `MentionView` — keep consistent.

## Scope
- Match `^` references by SHAPE, not by enumerating a slug list: `^` + exactly 26 OR exactly 7 Crockford-base32 chars, longest-first (26 before 7), with existing boundary guards (`[\w-]` neighbors) and fenced-code/inline-code/heading skips.
- Normalize a matched full ULID to its last-7, resolve against the single short-id-keyed task metaMap (from short-ids-mention-identity).
- Pill rendering for tasks: LABEL = `^<short>`, TOOLTIP = title. Likely mapping: for the task metaMap set displayName = short id (so the generic widget labels the pill `^<short>`) and description = title (so the existing tooltip path shows the title). Confirm/route the tooltip to the title.
- Unknown id (shape matches, resolution misses) → muted raw text (existing behavior), no crash.
- Feed the short-id-keyed task metaMap into the description editor + read-only display.
- Confirm `depends_on` field pills still render and now display `^<short>` with title tooltip.

## Acceptance
- Typing/pasting `^8rfp1r` OR `^01KT4CNAYW7JG0X8F8W28RFP1R` in a description renders a `^<short>` pill; hovering shows the task title.
- Read-only markdown display renders the same pill + tooltip for both forms.
- Caret-adjacent reverts to the raw editable token; unknown id shows muted raw text, no crash.

Depends on short-ids-mention-identity.