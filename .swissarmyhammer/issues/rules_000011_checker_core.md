# Implement RuleChecker with Two-Stage Rendering

Refer to ideas/rules.md

## Goal

Implement the core `RuleChecker` that performs two-stage rendering and executes checks via LLM agent.

## Context

RuleChecker is the heart of the rules system. It:
1. Renders rule templates with context
2. Renders the .check prompt with the rendered rule
3. Executes via LlamaAgent
4. Parses responses and fails fast on violations

## Implementation

1. Create `src/checker.rs` module
2. Define `RuleChecker` struct:
```rust
pub struct RuleChecker {
    agent: Arc<LlamaAgentExecutor>,
    prompt_library: PromptLibrary,
}
```

3. Implement `new()` that loads PromptLibrary with .check prompt

4. Implement two-stage rendering:
   - Stage 1: Render rule template with `{{language}}`, `{{target_path}}`, etc.
   - Stage 2: Render .check prompt with `{{rule_content}}`, `{{target_content}}`, etc.

5. Use `swissarmyhammer-templating` for rendering

6. Integration with `LlamaAgentExecutor` from `swissarmyhammer-workflow::agents`

## Testing

- Unit tests for rendering stages
- Integration test with mock agent
- Test fail-fast behavior

## Success Criteria

- [ ] RuleChecker struct defined
- [ ] Two-stage rendering implemented
- [ ] Agent integration working
- [ ] Unit tests passing
