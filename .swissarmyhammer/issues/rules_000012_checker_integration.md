# Integrate RuleChecker with LlamaAgentExecutor

Refer to ideas/rules.md

## Goal

Complete RuleChecker implementation with full LlamaAgent integration and fail-fast checking.

## Context

This completes the RuleChecker by adding the full check_all method that checks rules against files and fails fast on first violation.

## Implementation

1. Implement `check_all(rules: Vec<Rule>, targets: Vec<PathBuf>)` method:
   - Iterate every rule against every target
   - For each combination:
     - Read target file content
     - Detect language
     - Render rule template (stage 1)
     - Render .check prompt (stage 2)
     - Execute via agent
     - Parse response
     - Fail fast if violation found
   
2. Response parsing:
   - "PASS" = no violation
   - Anything else = violation (return RuleError::Violation)
   
3. Add comprehensive logging with tracing

## Testing

- Integration test with real .check prompt
- Test fail-fast behavior
- Test with multiple rules and files
- Test LLM response parsing

## Success Criteria

- [ ] check_all method complete
- [ ] LlamaAgent integration working
- [ ] Fail-fast implemented correctly
- [ ] Response parsing works
- [ ] Integration tests passing
