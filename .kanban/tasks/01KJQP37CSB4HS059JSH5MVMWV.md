---
title: Fix derive name mismatch between YAML definitions and ComputeEngine registrations
position:
  column: done
  ordinal: a0
---
**Files:** `swissarmyhammer-kanban/builtin/fields/definitions/attachment_mime_type.yaml:6`, `attachment_size.yaml:6`, `swissarmyhammer-kanban/src/defaults.rs:97-106`

**What:** The YAML definitions use `derive: attachment-mime-type` and `derive: attachment-file-size`, but `kanban_compute_engine()` registers `detect-mime-type` and `compute-file-size`. When `ComputeEngine::derive()` is called for these fields, it will return `Err(ComputeError { message: "unregistered derivation: attachment-mime-type" })`.

**Why:** This is a latent correctness bug. It won't crash today because the compute engine isn't wired into entity read paths yet, but it will break as soon as Card 6+ integrates computed field derivation into entity I/O.

**Fix:** Either rename the YAML `derive` values to match the registered names, or rename the registered names to match the YAML. The YAML names (`attachment-mime-type`, `attachment-file-size`) are more descriptive — recommend renaming the registrations.

- [ ] Decide on canonical names (YAML or Rust registration)
- [ ] Update whichever side is wrong to match
- [ ] Add a test that verifies all computed field `derive` names in builtin YAML have matching registrations in `kanban_compute_engine()`
- [ ] Run `cargo nextest run --workspace` to verify #Blocker