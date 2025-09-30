# Add Real LLM Execution to Test Command

Refer to ideas/rules.md

## Goal

Complete the test command by adding real LLM execution (Phase 5) and result parsing (Phase 6).

## Context

This is the critical part that actually executes the check via LLM so rule authors can see real results.

## Implementation

1. Add Phase 5 - LLM execution:
```rust
println!("5. Executing check via LLM agent...");
let agent = create_agent_from_config()?;
let response = agent.execute(check_prompt).await?;
println!("   LLM Response:");
println!("   {}", "─".repeat(60));
println!("{}", response);
println!("   {}\n", "─".repeat(60));
```

2. Add Phase 6 - Result parsing:
```rust
println!("6. Parsing result...");
if response.trim() == "PASS" {
    println!("   ✓ No violations found");
    Ok(())
} else {
    println!("   ✗ Violation detected");
    // Parse and display violation details
    Ok(())
}
```

3. Handle errors gracefully:
   - Agent connection failures
   - LLM timeout
   - Invalid responses

4. Make this a REAL LLM call (not mocked)

## Testing

- Test with rule that should pass
- Test with rule that should fail
- Test with LLM errors
- Integration test with real agent

## Success Criteria

- [ ] Phase 5 and 6 implemented
- [ ] Real LLM execution works
- [ ] Response parsing works
- [ ] Error handling robust
- [ ] Integration tests passing
