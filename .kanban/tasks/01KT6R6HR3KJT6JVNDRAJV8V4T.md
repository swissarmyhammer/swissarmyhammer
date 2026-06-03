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
- `short_id(ulid) -> String` = last 7 Crockford-base32 chars of the ULID, lowercased: `ulid[19..26].to_lowercase()`. Never stored — always derivable.
  - Why the tail: a ULID's first 10 chars are a ms timestamp (low entropy, time-clustered); the last 16 are uniform random. Last 7 ≈ 35 bits, matches git's default abbreviation, effectively collision-free to ~100k tasks. Tail is already uniform → no hashing.
- Board-level resolver: `resolve_short_id(&board, &str) -> Option<TaskId>`. Case-insensitive. Accepts the bare 7-char short id, a leading `^`, OR a full 26-char ULID (full → last-7 → lookup). Returns exactly one task or none. Fixed length only — no adaptive/variable-length suffix matching.
- Live in `crates/swissarmyhammer-kanban` (has board/task context). Add `short_id` as a derived accessor on the task type.

## Storage policy (canonical)
- The full 26-char ULID is the ONE stored identity. Every structured reference field that holds a task id — `depends_on`, attachment refs, etc. — STORES the full ULID, never the short id. (Confirmed: depends_on stores full ULIDs.)
- The short id is display + input only. When a structured ref field is edited via `^` autocomplete (short id entered), normalize the short id back to the full ULID on commit before storing.
- Exception — free-text bodies (task description markdown): the literal token the user inserted is what's stored, which from autocomplete is `^<short>`. This is safe because short ids are unique-at-create and stable for the task's life; the pill layer re-resolves them on render. No normalization of body text.
- JSONL on disk and filenames are unchanged.

## Out of scope (separate cards)
- Create-time uniqueness / regenerate-on-collision → short-ids-create-uniqueness
- Tool/CLI input + output (incl. ref-field input normalization) → short-ids-tool-api
- Filter `^ref` eval → short-ids-filter-eval
- CM6 mention identity / pills / autocomplete → short-ids-mention-identity + the two CM6 cards

## Acceptance
- `short_id` derivation + round-trip unit tests.
- Resolver tests: exact short id, `^`-prefixed, full ULID, case-insensitive, not-found, and a real lookup against a constructed task.

Reference: https://github.com/kenn-io/kata — "short IDs derived from each issue's ULID".