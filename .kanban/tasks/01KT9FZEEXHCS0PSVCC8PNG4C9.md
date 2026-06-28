---
assignees:
- claude-code
depends_on:
- 01KT9FYTVE2CMAGZQW29G1M6Q6
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff280
project: plugin-arch
title: 'Docs: document the SDK event-subscription API (.on); remove stale "inert/RESERVED" language'
---
Document the event-subscription API now that it is real. Depends on the SDK surface card.

WRITING CONSTRAINT (user feedback): do NOT use internal architecture jargon ("plane", "bridge", "fan-in") in plugin-author-facing docs — nobody outside the codebase knows those words. Describe events by their concrete string names and which service emits them.

## Scope
- `ideas/plugins/plugin-architecture.md`: add an "Events" section — `this.<server>.on(event, cb)` returns an off handle; events are the notifications each service DECLARES (shown by string name in the generated types); the cached-flag pattern for reactive `available()` (the canonical consumer). Explain in concrete terms (e.g. "the command service emits an `executed` event after each command runs"), not abstractions.
- `ideas/plugins/command-service.md`: cross-link the cached-flag/`available` pattern to the now-real `.on()` API.
- Code comments: remove "intentionally inert" / "event API not implemented in this SDK task" / "wired by a later task" language in sdk/plugin.ts and anywhere else calling the event API reserved/unimplemented.

## Acceptance
Docs describe the real API in concrete terms (event string names + emitting service), no stale "RESERVED/inert" claims remain, and no internal jargon leaks into author-facing prose.