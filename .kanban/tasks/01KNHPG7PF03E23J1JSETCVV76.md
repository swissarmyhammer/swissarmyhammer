---
assignees:
- claude-code
position_column: todo
position_ordinal: b580
title: 'Coverage: yaml_loader.rs fallback loading path'
---
## What

`swissarmyhammer-lsp/src/yaml_loader.rs` — the secondary/fallback loading path (when individual YAML files need loading) is entirely uncovered. This is the path that loads YAML specs from a custom directory.

## Acceptance Criteria
- [ ] Test exercises the fallback YAML loading path
- [ ] Test uses a custom YAML directory with known spec files
- [ ] Loaded specs match expected values from the YAML files

## Tests
- [ ] Add test in `swissarmyhammer-lsp/src/yaml_loader.rs` (or `tests/`) that creates a temp directory with valid YAML LSP spec files, invokes the fallback loader, and asserts specs are loaded correctly
- [ ] `cargo test -p swissarmyhammer-lsp yaml_loader` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #coverage-gap