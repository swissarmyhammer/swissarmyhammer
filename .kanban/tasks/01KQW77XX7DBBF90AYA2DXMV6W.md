---
assignees:
- wballard
depends_on:
- 01KQW6M2P2MF7KDGZ8SQT481T5
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff9b80
project: spatial-nav
title: 'spatial-nav redesign step 14: docs purge — README + module comments + rustdoc, kill historical narration, slash wordcount'
---
## Parent

Final step in the architectural redesign tracked by **01KQTC1VNQM9KC90S65P7QX9N1**.

## The problem

Documentation across the crate has grown bloated and nearly unreadable, with three cross-cutting failures:

1. **Excess length.** README is 347 lines. `lib.rs` `//!` is 91 lines. `navigate.rs` `//!` is 112 lines. `registry.rs` has 58 lines of crate-comment plus paragraph-long rustdoc on individual items. Reading any of it is a chore.

2. **Historical narration everywhere.** Doc comments describe how the code *got here*, not what it *does*. Examples to grep for and delete:
   - `Step N of the spatial-nav redesign...`
   - `After step 12 the field is removed`
   - `Once the snapshot-driven IPC lands...`
   - `Today this type is unused; step 6...`
   - `transitional dual-write phase of the spatial-nav redesign`
   - `the redesign's removal target`
   - References to `01KQTC1VNQM9KC90S65P7QX9N1`, `01KQQSXM2PEYR1WAQ7QXW3B8ME`, `01KQ9XBAG5P9W3JREQYNGAYM8Y`, or any other prior-design ULID
   - `pre-redesign call` / `post-redesign` / `formerly` / `previously` / `legacy`
   
   **Referencing prior designs is worthless.** The reader is trying to understand the code as it exists. They don't need a story about what it used to be.

3. **Way too wordy.** Where one sentence answers "what is this and what does it do," the existing prose runs three paragraphs. Strip every adjective, every qualifier, every "this is the X that does Y because Z used to be W."

This step is **not an audit and tweak**. It's a **purge and rewrite** across:
- `swissarmyhammer-focus/README.md`
- All `//!` module-level docs in `swissarmyhammer-focus/src/*.rs`
- All `///` item-level rustdoc in `swissarmyhammer-focus/src/*.rs`

## Hard targets

| File | Current `//!` lines | Target |
|---|---|---|
| `README.md` | 347 | ≤ 150 |
| `lib.rs` | 91 | ≤ 25 |
| `navigate.rs` | 112 | ≤ 30 |
| `registry.rs` | 58 | ≤ 20 |
| `state.rs` | 32 | ≤ 20 |
| `snapshot.rs` | 36 | ≤ 20 |
| `scope.rs` | 36 | ≤ 15 |
| `types.rs` | 40 | ≤ 15 |
| `layer.rs` | 18 | leave / trim if obvious |
| `observer.rs` | 19 | leave / trim if obvious |

Item-level rustdoc on `pub` types and methods: keep one paragraph max, ideally 2-3 sentences. Multi-paragraph rustdoc is allowed only when the item has a genuinely complex contract (pathfinding, fallback resolution rule cascade) — and even then, three paragraphs maximum.

## What "describe the code as it exists" means

Doc comments answer two questions:

1. **What is this?** (one sentence)
2. **What does it guarantee / what's the contract?** (one paragraph max)

That's it. Not:
- How it relates to a previous design
- Which step of which redesign added it
- What it used to be
- What it will become
- Why we chose this over an alternative we considered

If the API has a non-obvious invariant (e.g., "must be called before X for reason Y"), state the invariant. Don't narrate the journey to it.

## Process

For each file:

1. **Read the existing doc comment cold.** Highlight every sentence that mentions a step number, a ULID, a tense-shift verb (`used to`, `will be`, `formerly`, `now that`), or describes an alternative design.
2. **Delete those sentences.** No editing — delete.
3. **Re-read what's left.** If it still describes "what is this and what's the contract" clearly, you're done with that file. If gaps remain, write one sentence to fill the gap.
4. **Word-count check.** If the doc is still over the target, you're being too verbose. Cut adjectives. Cut "this is the X that does Y" phrasing — just say what Y is.
5. **Verify code references.** Every type/method name mentioned in the doc must still exist in the current code (post-step-12).

## Required README structure (≤ 150 lines)

A reader who knows nothing opens the README and sees:

1. **Top paragraph** (3-5 lines): headless spatial-nav kernel; layers + focus state + pathfinding + fallback; consumers ship per-decision `NavSnapshot`s, kernel reads/decides/discards.
2. **Primitives** (≤ 15 lines): `Layer`, `NavSnapshot`, `SnapshotScope` — one sentence each, link to rustdoc.
3. **Operations** (≤ 40 lines): nav (up/down/left/right), drill in/out, first/last sibling, focus, focus-lost, clear, push-layer, pop-layer. IPC signature + 1-2 sentences each. Beam-pick math gets a paragraph. Fallback rule cascade gets a paragraph.
4. **Consumer contract** (≤ 15 lines): consumer owns scope structure + geometry; kernel owns focus + pathfinding + fallback; per-decision snapshots are the only data exchange.
5. **Coordinate system / scrolling** (≤ 20 lines): keep the existing reference content, trim adjectives.

Anything else is rustdoc material.

## What to delete from `//!` and `///` docs

Run these greps as a first pass, delete all matches:

```bash
cd swissarmyhammer-focus
grep -rn "step [0-9]" src/
grep -rn "redesign\|the redesign" src/
grep -rn "transitional\|in transit" src/
grep -rn "pre-redesign\|post-redesign\|formerly\|previously\|used to\|will be" src/
grep -rn "01KQTC1VNQM9KC90S65P7QX9N1\|01KQQSXM2PEYR1WAQ7QXW3B8ME\|01KQ9XBAG5P9W3JREQYNGAYM8Y" src/
grep -rn "future state\|after step\|once step" src/
```

Each match is a sentence (or paragraph) that needs to be deleted, not "softened." If deleting it leaves a sentence stranded, delete the surrounding sentence too.

## What to add (sparingly)

When you delete a wordy paragraph, check whether anything important gets lost. If the original prose contained a real invariant or contract, restate it in one sentence. Most of the time deleted prose was scaffolding around a single sentence; once you delete the scaffolding the kept sentence stands fine.

If a `pub` type has zero rustdoc after the purge, add one sentence describing what it is. Don't restore prose; one sentence.

## Acceptance criteria

- `wc -l swissarmyhammer-focus/README.md` returns ≤ 150
- Every per-file `//!` line count is at or below the target above
- All grep patterns above return **zero hits** in `swissarmyhammer-focus/src/` and `swissarmyhammer-focus/README.md`
- Zero references to deleted APIs (`register_scope`, `unregister_scope`, `update_rect`, `check_overlap_warning`, `state.handle_unregister`, etc.)
- Every type / method / IPC name in any doc comment exists in the current code
- `cargo doc -p swissarmyhammer-focus --no-deps` builds clean; `cargo test -p swissarmyhammer-focus` passes (catches dead doc-link errors)
- A reader who has never seen this codebase can answer "what does this crate do" in 30 seconds from the top of the README

## Files

- `swissarmyhammer-focus/README.md` — major trim
- `swissarmyhammer-focus/src/lib.rs` — crate `//!`
- `swissarmyhammer-focus/src/navigate.rs` — module `//!` + every `///` rustdoc
- `swissarmyhammer-focus/src/registry.rs` — module `//!` + every `///` rustdoc
- `swissarmyhammer-focus/src/state.rs` — module `//!` + every `///` rustdoc
- `swissarmyhammer-focus/src/snapshot.rs` — module `//!` + every `///` rustdoc
- `swissarmyhammer-focus/src/scope.rs` — module `//!` + every `///` rustdoc
- `swissarmyhammer-focus/src/types.rs` — module `//!` + every `///` rustdoc
- `swissarmyhammer-focus/src/layer.rs` — review, trim if needed
- `swissarmyhammer-focus/src/observer.rs` — review, trim if needed

## Out of scope

- Behavioral changes
- Test docs / test comments (those can stay verbose if they aid future test maintenance)
- Polishing prose style — this is brutalist editing, not stylistic refinement
- Memory-file cleanup (the user maintains those) #stateless-nav