---
name: small-focused-modules
description: One thing per module, composition over configuration
severity: warn
---

# JavaScript/TypeScript Small Focused Modules

- A module does one thing, describable in a single sentence.
- If the README needs multiple `##` sections for different major behaviors, it may be two packages.
- **Composition over configuration.** Prefer smaller composable functions over one function with 12 options.
- Do not add features just because someone asked. If it belongs in a different module, say so.
