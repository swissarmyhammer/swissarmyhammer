# Implement Rule Test Command with Rendering Display

Refer to ideas/rules.md

## Goal

Implement `sah rule test <rule> <file>` command that shows the full checking process including rendered templates.

## Context

The test command helps rule authors debug their rules by showing exactly what the LLM sees and how it responds.

## Implementation

1. In `test.rs`, define `TestCommand`:
```rust
pub struct TestCommand {
    pub rule_name: String,
    pub file_path: PathBuf,
}
```

2. Implement phases:
   - **Phase 1**: Validate rule
   - **Phase 2**: Read file and detect language
   - **Phase 3**: Show rendered rule template
   - **Phase 4**: Show rendered .check prompt
   - (Phase 5 in next step: Execute via LLM)

3. Display each phase clearly:
```rust
println!("1. Validating rule '{}'...", rule_name);
rule.validate()?;
println!("   ✓ Rule is valid\n");

println!("2. Reading file '{}'...", file_path.display());
let content = std::fs::read_to_string(file_path)?;
let language = detect_language(file_path, &content)?;
println!("   ✓ Detected language: {}\n", language);

println!("3. Rendering rule template...");
// Render rule with context variables
let rendered_rule = render_rule_template(...)?;
println!("   {}", "─".repeat(60));
println!("{}", rendered_rule);
println!("   {}\n", "─".repeat(60));

println!("4. Rendering .check prompt...");
// Render .check with rendered rule
let check_prompt = render_check_prompt(...)?;
println!("   {}", "─".repeat(60));
println!("{}", check_prompt);
println!("   {}\n", "─".repeat(60));
```

## Testing

- Test with valid rule
- Test with invalid rule
- Test with various file types
- Test rendering output

## Success Criteria

- [ ] TestCommand defined
- [ ] Phases 1-4 implemented
- [ ] Clear display output
- [ ] Tests passing



## Proposed Solution

After analyzing the existing codebase, I've identified the structure and implementation approach:

### Current State
- `TestCommand` struct exists in `cli.rs` with `rule_name`, `file`, and `code` fields
- `execute_test_command()` skeleton exists in `test.rs` but not implemented
- `RuleChecker` in `checker.rs` already has two-stage rendering:
  1. Stage 1: Render rule template with context (language, target_path, etc.)
  2. Stage 2: Render .check prompt with rendered rule content
- `RuleChecker::check_file()` already performs real LLM execution via `LlamaAgentExecutorWrapper`

### Implementation Plan

The test command should mirror what `check_file()` does but with diagnostic output at each stage:

1. **Load and validate rule** - Use `RuleResolver` to load the rule by name, then validate
2. **Read file or use code** - Handle both `--file` and `--code` options
3. **Detect language** - Use existing `detect_language()` function
4. **Display Stage 1 rendering** - Show rendered rule template with context variables
5. **Display Stage 2 rendering** - Show rendered .check prompt
6. **Execute via agent** - Make real LLM call (reuse existing agent infrastructure)
7. **Display and parse response** - Show full LLM response and parse result

### Key Design Decisions

1. **Real LLM Execution**: Not a dry-run - this executes actual agent calls so rule authors can verify behavior
2. **Reuse existing infrastructure**: 
   - `RuleChecker::initialize()` for agent setup
   - Same two-stage rendering logic from `checker.rs`
   - Same `LlamaAgentExecutorWrapper` and `AgentExecutionContext`
3. **Handle both file and code input**: Support `--file path.rs` or `--code "fn main() {}"`
4. **Display separators**: Use `─` characters (60 width) to clearly separate output sections
5. **TDD approach**: Write failing tests first, then implement

### Implementation Steps

1. Implement file/code input handling (determine which one to use)
2. Add rule loading via `RuleResolver`
3. Add language detection (reuse existing function)
4. Implement Stage 1 rendering display (rule template → rendered rule)
5. Implement Stage 2 rendering display (.check prompt rendering)
6. Add agent execution and response display
7. Add result parsing (PASS vs VIOLATION)
8. Write comprehensive tests for each phase

