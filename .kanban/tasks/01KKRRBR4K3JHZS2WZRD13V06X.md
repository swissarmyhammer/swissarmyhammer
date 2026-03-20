---
assignees:
- claude-code
depends_on:
- 01KKRRB7864W4BQTEZFDCS084V
position_column: done
position_ordinal: ffffffff8180
title: Render project-type partials through PromptLibrary in detect.rs
---
## What

Replace the 10 `include_str!` calls and manual `strip_frontmatter()` in `swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs` with Liquid template rendering via `PromptLibrary::render_text()`.

**Current (broken):**
- 10 static `GUIDELINES_*` strings embedded at compile time via `include_str!`
- `guidelines_for_type()` maps ProjectType → raw string
- `strip_frontmatter()` duplicated locally (also exists in `markdowndown`)
- `format_detected_projects()` concatenates raw markdown — any `{% include %}` or `{{ variable }}` in partials is dead text

**Target:**
- `execute_detect()` gets `PromptLibrary` from `ToolContext.prompt_library`
- Build a Liquid template string in Rust based on detected types, e.g.:
  ```
  ## Project Guidelines\n\n{% include "_partials/project-types/rust" %}\n\n{% include "_partials/project-types/nodejs" %}
  ```
- Call `prompt_lib.render_text(&template, &ctx)` to render with full partial resolution
- Delete: all 10 `GUIDELINES_*` statics, `guidelines_for_type()`, local `strip_frontmatter()`
- Keep: `project_type_name()`, `project_type_key()`, `make_relative()`, `resolve_workspace_path()`

**Key discovery — partial name resolution:**
- Builtins loaded in `prompt_resolver.rs:143` as `_partials/{name}` where name = `project-types/rust`
- So `{% include "_partials/project-types/rust" %}` will resolve correctly
- The normalize_partial_name() function also tries `.md` and `.liquid` suffixes

**Signature change:** `format_detected_projects()` gains an `Option<&PromptLibrary>` parameter. When `None`, skip guidelines section entirely (graceful degradation).

**Files:**
- `swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs` — main rewrite

## Acceptance Criteria
- [ ] No `include_str!` for project-type partials in detect.rs
- [ ] No local `strip_frontmatter()` in detect.rs
- [ ] `detect projects` output includes rendered guidelines with Liquid includes resolved
- [ ] Graceful fallback: if `ToolContext.prompt_library` is `None`, output omits guidelines section

## Tests
- [ ] **New test: `test_render_project_guidelines_through_liquid`** — creates a `PromptLibrary::default()`, builds the template string for Rust type, renders it, asserts output contains "Rust Project Guidelines" and formatting section content. This is THE critical test — it proves the rendering pipeline works end-to-end.
- [ ] **New test: `test_format_without_prompt_library`** — calls `format_detected_projects` with `None` prompt library, verifies output has project listing but no guidelines section.
- [ ] **Update `test_format_with_guidelines`** — now passes a real `PromptLibrary::default()` and verifies rendered output contains expected content. This catches regressions if a partial name changes.
- [ ] **Update `test_all_project_types_have_guidelines`** — for each project type, build the include string `{% include "_partials/project-types/{type}" %}`, render it through `PromptLibrary::default()`, assert non-empty output. This catches missing partials at test time instead of at runtime.
- [ ] **Delete** `test_strip_frontmatter_*` (4 tests) — covered by `markdowndown` crate tests
- [ ] **Update `test_execute_detect_with_rust_project`** — needs a ToolContext with prompt_library set to verify guidelines appear in output
- [ ] `cargo nextest run -p swissarmyhammer-tools`