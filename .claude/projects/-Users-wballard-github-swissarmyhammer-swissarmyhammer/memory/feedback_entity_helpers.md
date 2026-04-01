---
name: entity-helpers
description: Don't proliferate small helper functions in -entity or -fields crates — keep logic inline in the context methods
type: feedback
---

Don't create many small helper functions in swissarmyhammer-entity or swissarmyhammer-fields. The context (EntityContext) is the right place to own behavior — keep logic inline in context methods rather than splitting into lots of one-use helpers in io.rs or types.rs.

**Why:** The user prefers cohesive context objects over scattered utility functions. Helper proliferation makes it harder to follow the flow.

**How to apply:** When adding new behavior to the entity or fields layer, implement it directly in the relevant context method. Only extract a helper if it's genuinely reused from multiple call sites.
