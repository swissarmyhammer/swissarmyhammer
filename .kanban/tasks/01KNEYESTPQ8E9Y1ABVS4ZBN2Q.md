---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffa780
title: 'NIT: VirtualTagRegistry missing Debug, Default derive'
---
**File:** swissarmyhammer-kanban/src/virtual_tags.rs (VirtualTagRegistry)\n\n**What:** `VirtualTagRegistry` has a manual `Default` impl but no `Debug` impl. Per Rust API guidelines and the project's own review checklist, public types should implement `Debug`. The struct contains `Box<dyn VirtualTagStrategy>` which makes derived Debug impossible, but a manual impl that prints the registered slugs would be useful for diagnostics.\n\n**Suggestion:** Add a manual `Debug` implementation that formats the registered slug names, e.g.:\n```rust\nimpl std::fmt::Debug for VirtualTagRegistry {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        f.debug_struct(\"VirtualTagRegistry\")\n            .field(\"strategies\", &self.order)\n            .finish()\n    }\n}\n```\n\n**Verification:** cargo test -p swissarmyhammer-kanban --lib virtual_tags" #review-finding