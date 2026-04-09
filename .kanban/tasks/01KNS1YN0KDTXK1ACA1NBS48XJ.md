---
assignees:
- claude-code
depends_on:
- 01KNS1Y49EX2CZJQ38WGM23954
position_column: todo
position_ordinal: '8780'
project: code-context-cli
title: Create code-context builtin skill and implement skill command
---
## What
Two pieces of work:

### 1. Create `builtin/skills/code-context/SKILL.md`
Write the skill that teaches Claude Code how to use the code-context MCP tool effectively.

Content structure (mirror `builtin/skills/shell/SKILL.md` in format):
- Frontmatter: `name`, `description`, `allowed-tools` (if needed)
- Body: explain what the tool does, when to use each operation, worked examples
- Cover the key operations: `get symbol`, `search symbol`, `get callgraph`, `get blastradius`, `grep code`, `search code`, `get status`, `lsp status`, `detect projects`
- Emphasize: use `get blastradius` before making changes to understand impact, use `get callgraph` to understand dependencies

### 2. Create `code-context-cli/src/skill.rs`
Implement the `skill` subcommand that deploys the code-context skill to agent `.skills/` directories.

**Deployment pattern** (from `ShellExecuteTool::init` in `swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`):
1. Resolve builtin skill via `SkillResolver::resolve_builtins().get("code-context")`
2. Render templates with `TemplateEngine` (expand `{{version}}` etc.)
3. Write SKILL.md to a tempdir with frontmatter
4. Call `mirdan::install::deploy_skill_to_agents("code-context", &skill_dir, None, false)`

```rust
pub fn run_skill() -> i32 {
    let resolver = swissarmyhammer_skills::SkillResolver::new();
    let builtins = resolver.resolve_builtins();
    let skill = match builtins.get("code-context") {
        Some(s) => s.clone(),
        None => { eprintln!("Error: builtin 'code-context' skill not found"); return 1; }
    };

    // Render templates
    let engine = swissarmyhammer_templating::TemplateEngine::new();
    let mut vars = std::collections::HashMap::new();
    vars.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
    let rendered = engine.render(&skill.instructions, &vars).unwrap_or(skill.instructions.clone());

    // Write to tempdir
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let skill_dir = temp_dir.path().join("code-context");
    std::fs::create_dir_all(&skill_dir).expect("mkdir");
    // Format SKILL.md with frontmatter
    let mut content = String::from("---\n");
    content.push_str(&format!("name: {}\n", skill.name));
    content.push_str(&format!("description: {}\n", skill.description));
    content.push_str("---\n\n");
    content.push_str(&rendered);
    std::fs::write(skill_dir.join("SKILL.md"), &content).expect("write SKILL.md");

    // Deploy
    match mirdan::install::deploy_skill_to_agents("code-context", &skill_dir, None, false) {
        Ok(targets) => { println!("Deployed code-context skill to {}", targets.join(", ")); 0 }
        Err(e) => { eprintln!("Error: {}", e); 1 }
    }
}
```

**Dependencies needed in Cargo.toml**: `swissarmyhammer-skills`, `swissarmyhammer-templating`, `tempfile`. Add these to the scaffolding card.

## Acceptance Criteria
- [ ] `builtin/skills/code-context/SKILL.md` exists with frontmatter and body >= 50 lines
- [ ] `cargo check -p code-context-cli` passes
- [ ] `run_skill()` returns 0 on success

## Tests
- [ ] `test_skill_exists_in_builtins` — `SkillResolver::new().resolve_builtins().get("code-context")` is `Some`
- [ ] `test_run_skill_returns_valid_exit_code` — `run_skill()` returns 0 or 1
- [ ] Run `cargo test -p code-context-cli skill` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.