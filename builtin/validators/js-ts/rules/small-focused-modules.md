---
name: small-focused-modules
description: One thing per module, composition over configuration
---

# JavaScript/TypeScript Small Focused Modules

- A module must do one thing. You must describe this one thing in a single sentence.
- Check the README for the module. If the README needs multiple `##` sections for different major behaviors, this is a sign of two packages.
- **Use composition, not configuration.** Use several small, composable functions instead of one function with 12 options.
- Do not add a feature only because someone asked for it. If the feature belongs in a different module, say so.
