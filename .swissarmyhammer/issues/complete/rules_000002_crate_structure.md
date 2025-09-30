# Create Rules Crate Structure and Basic Types

Refer to ideas/rules.md

## Goal

Create the `swissarmyhammer-rules` crate with basic structure and type definitions.

## Context

The rules crate will mirror the prompts crate structure but with rule-specific types. This is the foundation for all rule functionality.

## Implementation

1. Create `swissarmyhammer-rules/` directory
2. Create `Cargo.toml` copying workspace patterns from prompts
3. Create `src/lib.rs` with public API exports
4. Create basic module structure:
   - `lib.rs` - Public API
   - `rules.rs` - Rule struct and core types
   - `severity.rs` - Severity enum

5. Define `Severity` enum in `severity.rs`:
```rust
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}
```

6. Add crate to workspace `Cargo.toml` members list

## Testing

- Verify crate builds with `cargo build -p swissarmyhammer-rules`
- Verify crate is in workspace

## Success Criteria

- [ ] Crate directory and Cargo.toml created
- [ ] Basic module structure exists
- [ ] Severity enum defined
- [ ] Crate builds successfully
- [ ] Crate added to workspace



## Proposed Solution

Based on reviewing `ideas/rules.md` and the existing `swissarmyhammer-prompts` crate structure, I will:

1. **Create crate directory structure**:
   - `swissarmyhammer-rules/` at workspace root
   - Copy `Cargo.toml` pattern from prompts crate
   - Update workspace members list

2. **Define module structure** mirroring prompts:
   - `lib.rs` - Public API with re-exports
   - `severity.rs` - Severity enum (Error/Warning/Info/Hint)
   - Basic structure only, no complete implementation yet

3. **Key decisions**:
   - Follow exact same patterns as `swissarmyhammer-prompts`
   - Use same dependency versions from workspace
   - Keep it minimal - just foundation, no full implementation
   - Add to workspace Cargo.toml members list

4. **Testing approach**:
   - Verify crate builds with `cargo build -p swissarmyhammer-rules`
   - Verify workspace includes the crate
   - No functional tests yet (no functionality to test)



## Implementation Notes

The `swissarmyhammer-rules` crate provides:

### Structure

1. **swissarmyhammer-rules/Cargo.toml**: 
   - Minimal dependencies for basic types
   - Workspace integration

2. **swissarmyhammer-rules/src/lib.rs**:
   - Public API with re-exports
   - `RuleSource` enum (Builtin/User/Local)
   - Conversion from `FileSource` to `RuleSource`
   - Tests for source mapping

3. **swissarmyhammer-rules/src/severity.rs**:
   - `Severity` enum with four levels: Error, Warning, Info, Hint
   - Display and FromStr implementations
   - Serde support with lowercase serialization
   - Comprehensive unit tests

### Verification

- Crate builds: `cargo build -p swissarmyhammer-rules`
- Tests pass: `cargo nextest run -p swissarmyhammer-rules`
- Workspace member
- Follows patterns from swissarmyhammer-prompts crate
