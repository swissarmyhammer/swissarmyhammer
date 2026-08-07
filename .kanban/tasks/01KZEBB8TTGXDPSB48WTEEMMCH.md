---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9380
title: 'dead-code tools: evaluate narrow deterministic checks'
---
Evaluate deterministic dead-code tools. For each tool, decide: add a rule, or reject it and record why.

Do NOT supersede the `dead-code` prompt rule. Its carve-outs (entry points, exported public API, work-in-process scaffolding) need judgment, and the `callers` probe already gives that rule machine facts. A tool that supersedes it would flag staged work as dead.

Candidates for NEW narrow tool rules with no `supersedes`:
- Rust: `cargo machete` — unused dependencies. Low false-positive rate. Workspace scope.
- Go: `staticcheck -checks U1000` — unused code the compiler misses.
- JS/TS: `knip` — unused files and exports. Check the zero-config behavior first; a tool that demands per-project config does not fit the temporary-config contract.
- Python: `vulture` — known high false-positive rate. Reject it unless a confidence threshold makes it run clean on real code.
- Swift: `periphery` — needs a full project build. Likely too heavy for a review pass; reject if so.

Acceptance for each accepted tool: it runs clean on this repository, or every finding it reports is a real defect. Record each rejection in the code-hygiene VALIDATOR.md.

#tool-validators