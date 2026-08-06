---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8d80
title: 'ARCHITECTURE.md: document the fact-producer doctor path'
---
Found during ^2hk89aj. The Doctor Pattern section of ARCHITECTURE.md says: do not add health checks outside the `Doctorable` trait. That statement predates the `mirdan::status` fact-producer pattern. Two shipped modules now use the fact-producer path: `mirdan::status::check_install_stack` and `swissarmyhammer-validators::doctor::check_review_engine`.

Work:
- Update the Doctor Pattern section to describe both paths and when each applies:
  - `Doctorable` trait: a component checks itself.
  - Fact producer: a library produces status structs; `to_checks()` converts them to doctor `Check` rows; the CLI wires a thin loader with a `_with` test seam.
- Name the two fact-producer examples by module path.

Acceptance:
- The section no longer forbids the pattern two shipped modules use.

#tool-validators