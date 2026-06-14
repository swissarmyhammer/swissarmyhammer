---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa580
project: claude-model-select
title: Add builtin claude-code-haiku model YAML
---
## What
Add a new builtin model definition that runs Claude Code pinned to Haiku via the generic args passthrough.

Create `builtin/models/claude-code-haiku.yaml`:
```yaml
---
description: "Claude Code (Haiku): faster/cheaper Claude, must be installed separately"
tags: [kanban]
---
quiet: false
executor:
  type: claude-code
  config:
    args: ["--model", "haiku"]
```

The build-time `BuiltinGenerator` (`crates/swissarmyhammer-config/build.rs`, source dir `../../builtin/models`) auto-discovers the file — no include list to edit. The model name is the filename stem: `claude-code-haiku`.

## Acceptance Criteria
- [ ] `builtin/models/claude-code-haiku.yaml` exists and parses via the existing model loader.
- [ ] `get_builtin_models()` includes a `claude-code-haiku` entry.
- [ ] Loading it produces a `ModelConfig` with a single `claude-code` executor whose `ClaudeCodeConfig.args == ["--model", "haiku"]`.
- [ ] `description` and `tags` (`kanban`) are parsed from frontmatter.

## Tests
- [ ] In `crates/swissarmyhammer-config/src/model.rs` tests: assert `ModelManager::load_builtin_models()` (or the builtin list) contains `claude-code-haiku`.
- [ ] A test parsing the `claude-code-haiku` builtin into `ModelConfig` and asserting the executor is `ClaudeCode` with `args == ["--model", "haiku"]`.
- [ ] Run: `cargo test -p swissarmyhammer-config` — all green.

## Workflow
- Use `/tdd` — write the failing loader/parse test first, then add the YAML to make it pass. #haiku