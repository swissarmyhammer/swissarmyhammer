---
assignees:
- claude-code
depends_on:
- 01KTC7JKNV1ZM5F6JFPK1X7E2G
position_column: todo
position_ordinal: '8580'
project: remove-prompts
title: Merge prompts render engine into swissarmyhammer-templating (full rename)
---
## What
DECISION (locked by user): MERGE the surviving render engine into the existing `swissarmyhammer-templating` crate, and FULLY rename the prompt-named types so the word "prompt" disappears from the API. The crate currently named `swissarmyhammer-prompts` is really the skill/agent Liquid renderer + partial library; after the prompt-only types are stripped (predecessor task), fold what remains into `swissarmyhammer-templating`.

Scope (mechanical but wide — do it as one atomic change; the compiler + test suite are the safety net):
- Move the surviving modules from `crates/swissarmyhammer-prompts/src/` into `crates/swissarmyhammer-templating/src/` (renderer, partial library / `get_builtin_partials`, partial adapter, frontmatter parsing if not already in templating). Reconcile any overlap with templating's existing Liquid engine — prefer templating's engine, keep only what's additive.
- Move the build script logic that generates `builtin_partials.rs` from `builtin/_partials/` into `swissarmyhammer-templating`'s `build.rs` (KEEP partials; the `builtin_prompts.rs` embed is already gone by this point).
- Delete the `crates/swissarmyhammer-prompts/` directory and remove it from the top-level `Cargo.toml` workspace `members` list.
- Update every dependent `Cargo.toml` to depend on `swissarmyhammer-templating` instead of `swissarmyhammer-prompts` (confirmed dependents: `crates/swissarmyhammer`, `crates/claude-agent`, `crates/swissarmyhammer-mcp-proxy`, `crates/swissarmyhammer-config`, `crates/swissarmyhammer-tools`, `crates/mirdan`, `crates/swissarmyhammer-agent`, `crates/llama-agent`, `crates/avp-common`, `apps/swissarmyhammer-cli`). Drop the dependency entirely where templating is already a dep.
- Rewrite all `use swissarmyhammer_prompts::...` imports to `swissarmyhammer_templating::...`.
- FULL type rename: `PromptLibrary` -> `TemplateLibrary` (or `RenderLibrary`), `PromptPartialAdapter` -> `PartialAdapter`, and any other `Prompt*` type/method names on the surviving engine (e.g. `render_text` keeps its name; rename anything containing "prompt"). Update all call sites across the ~11 crates — confirmed callers of `render_text`: `swissarmyhammer-tools/src/mcp/tools/skill/use_op.rs`, `.../agent/mod.rs`, `.../code_context/detect.rs`, `crates/claude-agent/src/agent.rs`, `crates/mirdan/src/install.rs`, `crates/llama-agent/src/acp/config.rs`.

Sizing note: this is intentionally a large mechanical change and may exceed the normal 500-LOC guideline because the user chose full rename + merge. If it becomes unmanageable in one pass, the ONLY allowed split is: (1) merge crate + import paths here, (2) a fast-follow task for the `Prompt*`→new-name type renames — but the end state must have zero `Prompt*` symbols on the engine.

## Acceptance Criteria
- [ ] `crates/swissarmyhammer-prompts/` no longer exists; not in workspace `members`.
- [ ] `grep -rn "swissarmyhammer[_-]prompts"` returns nothing outside historical comments/CHANGELOG.
- [ ] `grep -rn "PromptLibrary\|PromptPartialAdapter\|PromptLoader\|PromptFilter\|PromptSource"` returns nothing (prompt-only types fully gone/renamed).
- [ ] `builtin/_partials/` is still embedded and rendered (skills/agents still resolve `{% render %}`/`{% include %}` partials).
- [ ] `cargo build --workspace` succeeds; `cargo metadata` shows no `swissarmyhammer-prompts`.

## Tests
- [ ] The renderer/partial tests that lived in `swissarmyhammer-prompts` (e.g. `skills_rendering_test.rs`, `all_skills_render_test.rs`) move to `swissarmyhammer-templating` and pass under the new names.
- [ ] `cargo test --workspace` compiles and is green.
- [ ] A skill-render integration test (existing) still passes, proving partial resolution survived the merge.

## Workflow
- Mechanical merge+rename: rely on the compiler and existing suite. Run `cargo build --workspace` after moving the crate and again after each batch of import/type renames. Use `get rename_edits` / workspace search to catch every call site.