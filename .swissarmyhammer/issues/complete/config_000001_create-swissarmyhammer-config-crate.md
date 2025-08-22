# Create SwissArmyHammer Config Crate

Refer to /Users/wballard/github/swissarmyhammer/ideas/config.md

## Objective

Create a new `swissarmyhammer-config` crate that will house the figment-based configuration system, replacing the existing custom TOML parsing in `sah_config` and `toml_config` modules.

## Context

The current configuration system uses custom TOML parsing and has limited file format support. The specification calls for using the `figment` crate to support multiple configuration file formats (TOML, YAML, JSON) with a clear precedence order.

## Tasks

### 1. Create the Crate Structure

Create the new crate directory and files:
- `swissarmyhammer-config/Cargo.toml`
- `swissarmyhammer-config/src/lib.rs`
- `swissarmyhammer-config/src/error.rs`
- Update workspace `Cargo.toml` to include the new member

### 2. Define Dependencies

Add required dependencies to the new crate:
```toml
figment = { version = "0.10", features = ["toml", "yaml", "json", "env"] }
serde = { version = "1.0", features = ["derive"] }
thiserror = "1.0"
tracing = "0.1"
dirs = "5.0"  # For home directory access
```

### 3. Basic Error Types

Create error types in `error.rs`:
- `ConfigError` enum for configuration-related errors
- File not found, parsing errors, validation errors
- Proper error messages and source chaining

### 4. Library Structure  

Set up initial `lib.rs` with:
- Module declarations
- Re-exports of main types and functions
- Basic documentation

### 5. Integration with Workspace

- Add the new crate as a workspace member
- Ensure it builds with `cargo build` 
- Add basic integration with existing crates (swissarmyhammer, swissarmyhammer-cli)

## Acceptance Criteria

- [ ] New `swissarmyhammer-config` crate exists in workspace
- [ ] Figment and required dependencies are properly configured
- [ ] Basic error types are defined
- [ ] Crate builds successfully with `cargo build`
- [ ] Workspace properly includes the new member crate

## Implementation Notes

- Keep this step focused on crate creation only
- No configuration loading logic yet - that comes in the next step
- Ensure the crate follows the same coding standards as the rest of the project
- Use the same license and metadata as other workspace crates

## Proposed Solution

Based on my analysis of the current configuration system (sah_config and toml_config modules) and the specification, I will:

### 1. Create New Crate Structure
- Create `swissarmyhammer-config/` directory with proper Rust crate structure
- Configure `Cargo.toml` with figment dependencies and required features
- Set up workspace integration

### 2. Design Philosophy
- Use figment directly rather than creating wrapper abstractions
- Keep it simple and focused on the core requirement: providing template variables
- No caching - read config fresh each time for immediate updates
- Support multiple file formats (TOML, YAML, JSON) and precedence order

### 3. Implementation Steps
1. Create crate directory and basic structure
2. Define error types for configuration operations
3. Set up module structure in lib.rs
4. Configure dependencies (figment with toml, yaml, json, env features)
5. Update workspace to include new crate member
6. Verify build succeeds

### 4. Key Dependencies
```toml
figment = { version = "0.10", features = ["toml", "yaml", "json", "env"] }
serde = { version = "1.0", features = ["derive"] }
thiserror = "1.0"
tracing = "0.1"
dirs = "5.0"
```

This first step focuses only on crate creation and basic structure. The actual configuration loading logic and figment integration will come in subsequent issues.

## Implementation Complete

Successfully created the `swissarmyhammer-config` crate with the following structure:

### Created Files
- `swissarmyhammer-config/Cargo.toml` - Crate configuration with figment dependencies
- `swissarmyhammer-config/src/lib.rs` - Library entry point with documentation
- `swissarmyhammer-config/src/error.rs` - Comprehensive error types for configuration operations

### Dependencies Added
```toml
figment = { version = "0.10", features = ["toml", "yaml", "json", "env"] }
serde = { workspace = true }
thiserror = "1.0"
tracing = { workspace = true }
dirs = { workspace = true }
```

### Workspace Integration
- Updated workspace `Cargo.toml` to include new crate member
- All builds pass successfully: `cargo build`, `cargo clippy`, `cargo fmt`

### Error Types Defined
- `ConfigError` enum covering all expected configuration scenarios
- Proper error chaining with `thiserror`
- Integration with figment error types
- Support for file not found, parsing, validation, and path resolution errors

### Validation Results
- ✅ Crate builds successfully with `cargo build -p swissarmyhammer-config`
- ✅ Full workspace builds without conflicts
- ✅ No clippy warnings or errors
- ✅ Code properly formatted with rustfmt
- ✅ Dependencies correctly configured with features

## Next Steps
This crate is now ready for the next phase: implementing the actual configuration loading logic with figment, file discovery, and template context integration.