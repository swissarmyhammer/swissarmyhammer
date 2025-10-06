# Implement full frontmatter parsing for prompts

## Location
`swissarmyhammer-prompts/src/storage.rs:201`

## Current State
```rust
// For now, return a simple prompt until frontmatter module is ready
```

## Description
Prompt loading currently returns simple prompts without proper frontmatter parsing. A frontmatter module should be implemented to support metadata in prompt files.

## Requirements
- Implement YAML frontmatter parsing for prompt files
- Support standard frontmatter fields (title, description, parameters, etc.)
- Handle prompts with and without frontmatter
- Add validation for frontmatter structure
- Add tests for various frontmatter scenarios

## Impact
- Limited metadata support for prompts
- Cannot define parameters in prompt files
- Reduces flexibility in prompt organization