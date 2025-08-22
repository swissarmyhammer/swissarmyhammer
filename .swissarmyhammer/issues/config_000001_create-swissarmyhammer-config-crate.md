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