---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
project: short-ids
title: 'Short IDs: core derivation + board resolver'
---
Foundation card for the Short Task IDs epic. Pure identity layer that everything else depends on. No UI, no create-time changes here.

## Scope
- `short_id(ulid) -> String` = last 7 Crockford-base32 chars of the ULID, lowercased: `ulid[19..26].to_lowercase()`. Never stored — always derivable. This is the CANONICAL short handle.
  - Why the SUFFIX (not a prefix): a ULID's first 10 chars are a ms timestamp that advances 32x slower per position — tasks minted in the same ~12-day window share `01KT…`, same ~17-min burst share `01KT6S…`. So a fixed-length PREFIX collides for time-clustered tasks (a real 7-char prefix `01KT6SA` matched 4 sibling cards on this very board). The last 16 chars are uniform random; the last 7 ≈ 35 bits, git-abbreviation length, collision-free to ~100k tasks even within a same-second burst.

## Resolver (forgiving input, one canonical output)
`resolve_short_ref(&board, &str) -> ResolveResult` where ResolveResult distinguishes Found(TaskId) / NotFound / Ambiguous(Vec<TaskId>) (NOT a bare Option — prefix matching needs to report ambiguity loudly). Steps, case-insensitive, after stripping an optional leading `^`:
1. Full 26-char ULID → that task.
2. Exactly a task's 7-char short id (suffix) → that task. Canonical form wins over any prefix interpretation.
3. Otherwise treat input as a ULID PREFIX (git-style): if exactly one task's ULID starts with it → that task; if more than one → Ambiguous (caller surfaces "ambiguous, N matches", lists short ids); if none → NotFound.
- Rationale for accepting prefixes: agents instinctively abbreviate ULIDs by prefix; accepting a unique prefix means that habit still resolves. But a unique prefix is often long (8–10+ chars) in same-session bursts, so it is forgiving input only — the displayed/canonical handle is always the 7-char suffix.

## Storage policy (canonical)
- The full 26-char ULID is the ONE stored identity. Every structured reference field that holds a task id — `depends_on`, attachment refs, etc. — STORES the full ULID, never the short id. (Confirmed: depends_on stores full ULIDs.)
- The short id is display + input only. When a structured ref field is edited via `^` autocomplete (short id entered), normalize back to the full ULID on commit before storing.
- Exception — free-text bodies (task description markdown): the literal token the user inserted is stored, which from autocomplete is `^<short>`. Safe because short ids are unique-at-create and stable; the pill layer re-resolves on render. No normalization of body text.
- JSONL on disk and filenames are unchanged.

## Out of scope (separate cards)
- Create-time uniqueness / regenerate-on-collision → short-ids-create-uniqueness
- Tool/CLI input + output (incl. ref-field input normalization) → short-ids-tool-api
- Filter `^ref` eval → short-ids-filter-eval
- CM6 mention identity / pills / autocomplete → short-ids-mention-identity + the two CM6 cards

## Acceptance
- `short_id` derivation + round-trip unit tests.
- Resolver tests: full ULID; exact 7-char short id; `^`-prefixed; case-insensitive; unique ULID prefix resolves; ambiguous prefix returns Ambiguous with >1 match (use the real same-prefix sibling case, e.g. `01KT6SA` matching 4 tasks); short-id match beats a colliding prefix interpretation; not-found.

Reference: https://github.com/kenn-io/kata — "short IDs derived from each issue's ULID".