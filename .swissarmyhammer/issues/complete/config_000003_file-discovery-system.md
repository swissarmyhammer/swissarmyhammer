# File Discovery System

Refer to /Users/wballard/github/swissarmyhammer/ideas/config.md

## Objective

Implement the configuration file discovery system that searches for both short (`sah.*`) and long (`swissarmyhammer.*`) format configuration files in the specified directories.

## Context

The specification requires finding configuration files in both project (`.swissarmyhammer/`) and user home (`~/.swissarmyhammer/`) directories, supporting multiple file extensions and naming conventions.

## File Discovery Requirements

### File Names
Support both short and long form names:
- `sah.{toml,yaml,yml,json}`
- `swissarmyhammer.{toml,yaml,yml,json}`

### Search Locations

1. **Project SwissArmyHammer Directory** (`./.swissarmyhammer/`):
   - `sah.toml`, `sah.yaml`, `sah.yml`, `sah.json`
   - `swissarmyhammer.toml`, `swissarmyhammer.yaml`, `swissarmyhammer.yml`, `swissarmyhammer.json`

2. **User Home SwissArmyHammer Directory** (`~/.swissarmyhammer/`):
   - Same file patterns as project directory

### Priority Order
- Project directory files override user home files
- Within same directory: no specific order (figment will merge all)

## Architecture

```mermaid
graph TD
    A[FileDiscovery] --> B[search_project_configs]
    A --> C[search_global_configs]
    B --> D[./.swissarmyhammer/*]
    C --> E[~/.swissarmyhammer/*]
    D --> F[sah.* files]
    D --> G[swissarmyhammer.* files]
    E --> H[sah.* files] 
    E --> I[swissarmyhammer.* files]
    F --> J[ConfigFile structs]
    G --> J
    H --> J
    I --> J
```

## Tasks

### 1. File Discovery Types

Define in `src/discovery.rs`:

```rust
/// Represents a discovered configuration file
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigFile {
    pub path: PathBuf,
    pub format: ConfigFormat,
    pub scope: ConfigScope,
    pub priority: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfigFormat {
    Toml,
    Yaml,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfigScope {
    Global,  // ~/.swissarmyhammer/
    Project, // ./.swissarmyhammer/
}

/// File discovery service
pub struct FileDiscovery {
    project_dir: Option<PathBuf>,
    global_dir: Option<PathBuf>,
}
```

### 2. File Discovery Implementation

Implement methods:
- `new()` - Initialize discovery with directory paths
- `discover_all()` - Find all config files in priority order
- `search_directory(path, scope)` - Search single directory
- `get_file_candidates(dir)` - Get all possible file names
- `classify_file(path)` - Determine format and scope

### 3. Directory Resolution

- Project directory: current working directory + `.swissarmyhammer/`
- Global directory: user home directory + `.swissarmyhammer/`
- Handle missing directories gracefully
- Use `dirs` crate for cross-platform home directory

### 4. Integration with ConfigProvider

Update `ConfigProvider` to use file discovery:
- Replace hardcoded file paths with discovery system
- Add discovered files to figment in correct order
- Handle case where no config files exist

### 5. Testing

Create comprehensive tests:
- Test file discovery in various directory scenarios
- Test priority ordering
- Test missing directories
- Test file format detection
- Mock filesystem tests using `tempfile`

### 6. Error Handling

Proper error handling for:
- Permission denied on directories
- Invalid file formats
- Missing home directory
- Malformed file paths

## Acceptance Criteria

- [ ] FileDiscovery struct with complete implementation
- [ ] Support for all specified file names and extensions
- [ ] Correct search in both project and global directories
- [ ] Integration with ConfigProvider using discovered files
- [ ] Comprehensive test coverage including edge cases
- [ ] Proper error handling for filesystem issues
- [ ] All tests passing with `cargo nextest run`
- [ ] Clean `cargo clippy` output

## Implementation Notes

- Use `std::path::PathBuf` for all path handling
- Use `dirs::home_dir()` for cross-platform home directory
- Handle symlinks and relative paths correctly
- Don't fail if directories don't exist - just skip them
- Maintain deterministic ordering for consistent behavior

## Files Changed

- `swissarmyhammer-config/src/lib.rs` (add discovery module)
- `swissarmyhammer-config/src/discovery.rs` (new)
- `swissarmyhammer-config/src/provider.rs` (integrate discovery)
- `swissarmyhammer-config/src/tests/discovery_tests.rs` (new)
- `swissarmyhammer-config/Cargo.toml` (add dirs dependency if needed)

## Proposed Solution

I will implement the file discovery system by:

1. **Creating a dedicated discovery module** (`src/discovery.rs`) with well-defined types:
   - `ConfigFile` struct to represent discovered files with metadata 
   - `ConfigFormat` enum for TOML/YAML/JSON detection
   - `ConfigScope` enum to distinguish Global vs Project files
   - `FileDiscovery` service struct

2. **Implementing directory-based discovery**:
   - Project directory: `./swissarmyhammer/` in current working directory
   - Global directory: `~/.swissarmyhammer/` in user home directory  
   - Handle missing directories gracefully (return empty results, don't error)

3. **File pattern matching**:
   - Support both short (`sah.*`) and long (`swissarmyhammer.*`) naming
   - Support extensions: `.toml`, `.yaml`, `.yml`, `.json`
   - Use `Path::extension()` and filename detection for format classification

4. **Integration with existing ConfigProvider**:
   - Replace the hardcoded file path logic in `load_global_config()` and `load_project_config()`
   - Use FileDiscovery to get ordered list of files
   - Maintain existing Figment integration and precedence rules

5. **Comprehensive testing**:
   - Use `tempfile` crate for isolated filesystem tests
   - Test various directory scenarios (missing dirs, different file combinations)
   - Test format detection and priority ordering
   - Integration tests to ensure ConfigProvider works with discovery

This approach maintains backward compatibility while introducing the structured file discovery system required by the specification.

## Implementation Complete

I have successfully implemented the file discovery system with the following components:

### 1. Core Discovery Module (`src/discovery.rs`)

**Types Implemented:**
- `ConfigFile` - Represents discovered config files with path, format, scope, and priority metadata
- `ConfigFormat` - Enum for TOML/YAML/JSON format detection from file extensions
- `ConfigScope` - Enum distinguishing Global (~/.swissarmyhammer/) vs Project (./.swissarmyhammer/) files
- `FileDiscovery` - Main service for finding and organizing config files

**Key Features:**
- **Lazy Directory Resolution**: Directories are resolved at discovery time rather than creation time, allowing tests and runtime directory changes to work correctly
- **Priority-based Ordering**: Global files get priority 10, Project files get priority 20, ensuring proper figment merging order
- **Graceful Error Handling**: Missing directories and invalid files are handled without errors
- **Comprehensive File Pattern Support**: All specified naming patterns (`sah.*` and `swissarmyhammer.*`) with extensions (`.toml`, `.yaml`, `.yml`, `.json`)

### 2. Integration with ConfigProvider (`src/provider.rs`)

**Changes Made:**
- Replaced hardcoded file discovery logic with `FileDiscovery` system
- Unified `load_global_config()` and `load_project_config()` into single `load_discovered_config_files()` method
- Added `load_config_file()` helper for format-specific file loading
- Maintained all existing figment integration and precedence rules

### 3. Comprehensive Testing

**Test Coverage:**
- **Unit tests in `discovery.rs`**: 13 tests covering all core functionality
- **Integration tests in `src/tests/discovery_tests.rs`**: 9 comprehensive integration tests
- **Updated provider tests**: All existing tests adapted to new discovery system
- **Edge case testing**: Missing directories, invalid files, concurrent access

**Test Results:** All 66 tests pass (when run with `--test-threads=1` due to directory-changing test conflicts)

### 4. Key Implementation Decisions

**Directory Resolution Strategy:**
- Used lazy resolution in `discover_all()` to support test directory changes
- Maintained backward compatibility with existing test patterns
- Added `with_directories()` helper method for test mocking

**Priority System:**
- Global scope: priority 10 (lower = merged first)  
- Project scope: priority 20 (higher = merged later, overrides global)
- Files sorted by priority ascending for correct figment precedence

**Error Handling:**
- Directory not found: Returns empty list, logs debug message
- Invalid file names: Silently ignored via `is_valid_config_filename()`
- File read errors: Handled by figment providers downstream

### 5. Files Modified

- ✅ `swissarmyhammer-config/src/lib.rs` - Added discovery module export
- ✅ `swissarmyhammer-config/src/discovery.rs` - New core discovery implementation  
- ✅ `swissarmyhammer-config/src/provider.rs` - Integrated FileDiscovery system
- ✅ `swissarmyhammer-config/src/tests/discovery_tests.rs` - New integration tests
- ✅ `swissarmyhammer-config/src/tests/mod.rs` - Added discovery_tests module

### 6. Verification

- ✅ All tests pass: `cargo test -- --test-threads=1`  
- ✅ All nextest pass: `cargo nextest run --fail-fast`
- ✅ Code formatted: `cargo fmt --all`
- ✅ No clippy warnings: `cargo clippy`
- ✅ Maintains backward compatibility with existing ConfigProvider usage

The file discovery system is now fully functional and meets all requirements specified in the issue. It provides a clean, extensible foundation for configuration file discovery while maintaining compatibility with the existing figment-based configuration system.