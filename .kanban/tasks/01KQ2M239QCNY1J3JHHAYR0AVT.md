---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffa280
project: spatial-nav
title: 'ARCHITECTURE.md: document swissarmyhammer-focus crate'
---
## What

Add a section to `ARCHITECTURE.md` describing the `swissarmyhammer-focus` crate. Currently `ARCHITECTURE.md` does not mention it at all; once the spatial-nav stack lands more fully it should be reflected in the architecture doc.

## Acceptance Criteria
- [x] `ARCHITECTURE.md` has a section describing `swissarmyhammer-focus`
- [x] Section covers the registry / state / strategy split
- [x] Section covers the layer-as-hard-boundary contract (nav never crosses `LayerKey`)
- [x] Section covers the three-rule beam-search cascade (within-zone, cross-zone leaf fallback, no-op) plus zone-level sibling-only nav
- [x] If a Mermaid diagram is used elsewhere in `ARCHITECTURE.md`, the new section follows the same style

## Tests
- No code tests — this is a documentation task. Reviewer reads the section against the actual crate surface and confirms the description matches.

## Notes

Flagged as a follow-up nit on review of the spatial-nav algorithm card (`01KNQXXF5W7G4JP73C6ZCMKYKX`). Best to tackle this once more of the spatial-nav epic has landed so the docs stay accurate.

## Implementation Notes

Added a new `### Spatial Focus and Keyboard Navigation` subsection to section 5 (UI Programs) of `ARCHITECTURE.md`, placed between the kanban-app material and `### Patterns`. The section opens with crate purpose (Tier 2, domain-free, headless) and covers:

- **Registry / State / Strategy split** — three independent types with different mutation rates, plus the rationale for the split (mutation rates, single-source-of-truth, pluggable algorithm). Includes ASCII tree diagram of the data layout.
- **Layer as a hard modal boundary** — explicit "nav never crosses a `LayerKey`" contract, layer forest example (window / inspector / dialog / palette), and the implementation point that `BeamNavStrategy` filters by layer at the candidate-iteration step.
- **Three-rule beam-search cascade and zone-level sibling-only nav** — enumerates the three rules (within-zone, cross-zone leaf fallback, no-op) for leaves, the sibling-zone-only rule for zones, and edge commands. Includes ASCII branch diagram of the navigation decision tree.

`ARCHITECTURE.md` does not use Mermaid diagrams elsewhere — it uses fenced-code ASCII tree/box diagrams — so the new section matches that existing style.

## Review Findings (2026-04-25 16:08)

### Nits
- [x] `ARCHITECTURE.md` (intro paragraph of the new "Spatial Focus and Keyboard Navigation" section) — The crate is described as "a Tier-1 crate", but by the strict tier rules in section 1 of this same document, `swissarmyhammer-focus` is Tier 2: Tier 0 = "zero workspace dependencies", `swissarmyhammer-common` has a workspace dependency on `swissarmyhammer-directory` so `common` is Tier 1, and `swissarmyhammer-focus` depending on `common` puts it at Tier 2 ("Depends on Tier 0-1"). The "domain-free, headless infrastructure" framing is morally Tier-1-flavored, but the literal tier number is off by one against the doc's own definitions. Either drop the explicit tier label and keep the descriptive "domain-free, headless" framing, or correct the number to Tier 2. **Resolved:** Corrected the number to "Tier 2" (this also resolves the hyphenation nit below in the same edit).
- [x] `ARCHITECTURE.md` (intro paragraph: "drive the kernel through three peer types described below") — The phrase "three peer types" is already used inside the crate (`swissarmyhammer-focus/src/scope.rs` module docs) to mean `Focusable` / `FocusZone` / `FocusScope`. Reusing the same phrase here for `SpatialRegistry` / `SpatialState` / `NavStrategy` will collide for any reader who follows the link to the source. Suggest "three top-level types" or "three independent stores" to disambiguate. **Resolved:** Changed to "three top-level types".
- [x] `ARCHITECTURE.md` (intro paragraph: "It is a Tier-1 crate") — Hyphenated "Tier-1" is inconsistent with the rest of `ARCHITECTURE.md`, which uses "Tier 0", "Tier 1", "Tier 2" with a space (see lines 15-23 of the same file). Match the prevailing style. **Resolved:** Now reads "Tier 2" with a space, matching the prevailing style.