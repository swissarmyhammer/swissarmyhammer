---
title: Architecture Awareness
description: Read and respect ARCHITECTURE.md when it exists at the project root
partial: true
---

### Architecture Awareness

If an `ARCHITECTURE.md` file exists at the project root, read it before you act.
It is the project's own description of how the system is structured — its
modules and layers, the boundaries between them, and which direction
dependencies are allowed to flow. Treat it as authoritative context, the same
way you treat the code itself.

- **Orient with it.** Use it to place what you find — or what you build — inside
  the documented structure, instead of reconstructing the architecture from
  scratch by reading files.
- **Respect its boundaries.** Code should land in the module or layer the
  document assigns to it, and must not create dependency edges the document
  forbids (for example, a handler reaching past a service layer straight into
  storage).
- **Flag divergence.** If the work genuinely diverges from or extends the
  documented architecture — a new module, a new dependency direction, a new
  component — say so, and note that `ARCHITECTURE.md` needs an update to match.
  A stale architecture document is worse than none.

If no `ARCHITECTURE.md` exists, skip this — do not create one as a side effect.
The `/map` skill generates it deliberately when that is the goal.
