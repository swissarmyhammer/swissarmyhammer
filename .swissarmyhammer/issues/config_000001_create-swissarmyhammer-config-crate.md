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

After analyzing the existing configuration system in `sah_config` and `toml_config` modules, I'll implement a new `swissarmyhammer-config` crate using the `figment` library to replace the custom TOML parsing with support for multiple file formats.

### Current State Analysis

The existing system has:
- Custom TOML parsing in `toml_config/` with comprehensive validation
- Configuration loading in `sah_config/` with template integration
- Environment variable substitution and dot notation access
- Strong error handling and validation

### Implementation Plan

1. **Create Crate Structure**
   - New `swissarmyhammer-config/` directory with proper Cargo.toml
   - Basic `lib.rs`, `error.rs` modules
   - Follow existing workspace conventions (MIT/Apache license, same metadata)

2. **Dependencies Setup**
   - `figment` with `toml`, `yaml`, `json`, `env` features
   - `serde` for serialization
   - `thiserror` for error handling
   - `tracing` for logging
   - `dirs` for home directory access

3. **Error Types**
   - `ConfigError` enum covering file not found, parsing errors, validation errors
   - Proper error messages and source chaining compatible with existing error patterns

4. **Basic Library Structure**
   - Module declarations and re-exports
   - Documentation following existing patterns
   - Prepare for future integration with template system

5. **Workspace Integration**
   - Update root `Cargo.toml` workspace members
   - Ensure builds with existing test/build infrastructure

### Key Design Decisions

- **No backward compatibility**: Clean break from existing system as specified
- **Figment-first approach**: Leverage figment directly rather than creating abstraction layers
- **Consistent patterns**: Follow the same error handling, documentation, and code style as existing crates
- **Foundation-only**: This crate creation focuses on structure, not configuration loading logic yet

This approach will create a solid foundation for the figment-based configuration system while maintaining the high quality and consistency of the existing codebase.
## Implementation Completed

Successfully created the new `swissarmyhammer-config` crate with all requirements met:

### ✅ Completed Tasks

1. **Crate Structure Created**
   - `/swissarmyhammer-config/Cargo.toml` with proper workspace dependencies
   - `/swissarmyhammer-config/src/lib.rs` with comprehensive documentation
   - `/swissarmyhammer-config/src/error.rs` with robust error handling

2. **Dependencies Configured**
   - `figment` v0.10 with `toml`, `yaml`, `json`, `env` features
   - `serde` with derive features for serialization
   - `thiserror` for error handling
   - `tracing` for logging
   - `dirs` for directory access
   - `tempfile` for dev dependencies

3. **Error Types Implemented**
   - `ConfigError` enum covering all configuration error scenarios
   - Proper error messages with source chaining
   - `ConfigResult<T>` type alias for convenience

4. **Library Foundation**
   - Module declarations and re-exports
   - Comprehensive documentation with examples
   - Version constant export
   - Clear API design prepared for future iterations

5. **Workspace Integration**
   - Updated root `Cargo.toml` to include new member crate
   - Consistent metadata and licensing with existing crates
   - Proper workspace dependency usage

### ✅ Verification Complete

- **Build Success**: `cargo build` passes without errors
- **Code Quality**: `cargo fmt` and `cargo clippy` pass cleanly  
- **Tests**: All tests including doctests pass successfully

### 🚀 Foundation Ready

The `swissarmyhammer-config` crate is now ready for the next implementation phase where figment-based configuration loading logic will be added. The foundation provides:

- Robust error handling compatible with existing patterns
- Clear API design for future configuration features
- Full workspace integration with existing build infrastructure
- High-quality code following project standards

All acceptance criteria from the original issue have been met.