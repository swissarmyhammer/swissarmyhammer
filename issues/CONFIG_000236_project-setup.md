# CONFIG_000236: Project Setup and Research - sah.toml Configuration

Refer to ./specification/config.md

## Goal

Set up the foundation for implementing sah.toml configuration support in SwissArmyHammer. This includes understanding the existing template system, researching TOML parsing libraries, and defining the integration points.

## Tasks

1. **Research Current Template System**
   - Examine existing liquid template engine in `swissarmyhammer/src/template.rs`
   - Understand how variables are currently passed to templates
   - Review workflow variable context handling
   - Identify integration points for configuration variables

2. **Research TOML Libraries**
   - Evaluate `toml` crate for TOML parsing
   - Check compatibility with existing serde deserialization patterns
   - Verify support for nested structures and environment variable substitution
   - Review error handling capabilities

3. **Define Configuration Structure**
   - Create basic ConfigValue enum for TOML value types
   - Design Configuration struct with HashMap for variables
   - Plan integration with existing TemplateEngine
   - Define error types for configuration loading

4. **Set Up Module Structure**
   - Create `swissarmyhammer/src/config/` module directory
   - Add module declarations in `lib.rs`
   - Set up basic file structure for configuration components

## Acceptance Criteria

- [ ] Understanding of current template variable system documented
- [ ] TOML parsing library selected and added to Cargo.toml
- [ ] Basic configuration module structure created
- [ ] Integration points with template engine identified
- [ ] Error handling strategy defined

## Files to Examine

- `swissarmyhammer/src/template.rs` - Current template engine
- `swissarmyhammer/src/workflow/execution.rs` - Variable context handling
- `swissarmyhammer/Cargo.toml` - Add toml dependency

## Next Steps

After completion, proceed to CONFIG_000237_core-data-structures for implementing the basic configuration parsing infrastructure.