---
assignees:
- claude-code
position_column: todo
position_ordinal: c080
title: 'Pre-existing: example_layering_e2e::committed_examples_coload_across_layers fails (file-notes load() note not written)'
---
Discovered while verifying the D1–D4 bridge/substrate fixes. `cargo test -p swissarmyhammer-plugin --test example_layering_e2e committed_examples_coload_across_layers` fails deterministically:

```
panicked at crates/swissarmyhammer-plugin/tests/example_layering_e2e.rs:373
the file-notes hello note must exist at <cwd>/notes/hello.txt: No such file or directory (os error 2)
```

## Confirmed pre-existing (NOT caused by the D1–D4 work)
Reverted host.rs/lib.rs/error.rs/in_process.rs to clean HEAD (4b09a40cb) via `git stash` and the test failed IDENTICALLY (same panic, same line 373). So this is independent of the bridge-call-scope / error-masking / focus-scope-chain fixes.

## What passes vs fails inside the test
- Effect 1 PASSES: the builtin probe loads, registers `{rust:"kanban"}`, and a real `init board` kanban call through it creates `.kanban` on disk.
- `loaded.len() == 2` PASSES: both the probe and the project-layer `file-notes` bundle load in one discovery pass.
- Effect 2 FAILS: the `file-notes` plugin's `load()` is supposed to write `notes/hello.txt` and the echo note against relative paths resolved at the pinned process CWD (test holds a `CurrentDirGuard` + is `#[serial]`). Neither note appears.

## Likely area
`file-notes` example plugin `load()` file-write side effect — the relative-path write resolved at the pinned CWD is not landing (the plugin's `load()` returns Ok so the write is failing/ swallowed, or resolving against a different working directory than the test's CurrentDirGuard pins). Investigate the file-notes example `load()` and how its host file-write binding resolves relative paths vs the process CWD the test pins.

## Acceptance
- `cargo test -p swissarmyhammer-plugin --test example_layering_e2e` is green (both tests), including the file-notes note assertions.
- Rest of the `swissarmyhammer-plugin` suite stays green (currently 88+ tests pass; this is the only failure).