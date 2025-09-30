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
