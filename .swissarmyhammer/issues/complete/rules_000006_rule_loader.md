# Implement RuleLoader

Refer to ideas/rules.md

## Goal

Implement `RuleLoader` to load rules from files, copying the pattern from `PromptLoader`.

## Context

The RuleLoader scans directories for `.md` and `.md.liquid` files, parses frontmatter, and creates Rule instances.

## Implementation

1. Create `src/rule_loader.rs`
2. Copy loading logic from `swissarmyhammer-prompts/src/prompts.rs::PromptLoader`
3. Adapt for Rule type:
   - Parse rule-specific frontmatter (severity, auto_fix)
   - NO parameters field (rules don't have parameters)
   - Handle compound extensions (`.md`, `.md.liquid`, `.liquid.md`)
   
4. Key methods:
   - `load_from_directory()` - Scan directory for rule files
   - `load_from_file()` - Load single rule file
   - `parse_rule()` - Parse frontmatter and create Rule

## Testing

- Unit tests for directory scanning
- Unit tests for file loading
- Unit tests for frontmatter parsing
- Test with example rule files

## Success Criteria

- [ ] RuleLoader implementation complete
- [ ] Loads rules from directories
- [ ] Parses rule frontmatter correctly
- [ ] Unit tests passing



## Proposed Solution

I will implement `RuleLoader` by copying and adapting the pattern from `PromptLoader` in swissarmyhammer-prompts. The implementation will:

1. **Create `src/rule_loader.rs`** with:
   - `RuleLoader` struct for managing file loading
   - `load_directory()` - Scan directories for rule files
   - `load_file()` - Load single rule file
   - `load_from_string()` - Load rule from string content
   - Support for compound extensions: `.md`, `.md.liquid`, `.liquid.md`, `.markdown`, `.liquid`, etc.

2. **Key Adaptations from PromptLoader**:
   - Parse rule-specific frontmatter fields (severity, auto_fix)
   - **NO parameters field** - rules don't have parameters
   - Use existing `frontmatter::parse_frontmatter()` function
   - Handle partial templates with `{% partial %}` marker
   - Set default descriptions for partials

3. **File Detection Pattern**:
   - Support multiple extensions: `.md`, `.md.liquid`, `.liquid.md`, `.markdown`, `.markdown.liquid`, `.liquid`, `.liquid.markdown`
   - Extract rule name from filename, handling compound extensions properly
   - Support nested directory structures

4. **Frontmatter Parsing**:
   - Required fields: title, description (except for partials)
   - Parse severity (error/warning/info/hint)
   - Parse auto_fix boolean
   - Parse category, tags
   - Store additional metadata

5. **Testing**:
   - Unit tests for directory scanning
   - Unit tests for file loading with various extensions
   - Unit tests for frontmatter parsing
   - Tests with example rule files
   - Tests for partial template detection

This follows the proven pattern from `PromptLoader` while removing parameter-specific logic and adding rule-specific fields.


## Implementation Complete

Successfully implemented `RuleLoader` following the pattern from `PromptLoader`.

### What Was Implemented

1. **Created `src/rule_loader.rs`** with:
   - `RuleLoader` struct with configurable file extensions
   - `load_directory()` - Recursively scans directories for rule files
   - `load_file()` and `load_file_with_base()` - Load single rule files with proper path handling
   - `load_from_string()` - Load rules from string content (useful for testing)
   - Helper methods: `is_rule_file()`, `extract_rule_name()`, `extract_rule_name_with_base()`, `is_likely_partial()`

2. **Supported Extensions**:
   - `.md`, `.md.liquid`, `.liquid.md`
   - `.markdown`, `.markdown.liquid`, `.liquid.markdown`
   - `.liquid`

3. **Frontmatter Parsing**:
   - Uses existing `frontmatter::parse_frontmatter()` function
   - Parses rule-specific fields: `severity`, `auto_fix`
   - Handles `title`, `description`, `category`, `tags`
   - NO parameters field (rules don't have parameters)
   - Stores additional metadata in metadata HashMap

4. **Partial Template Support**:
   - Detects partials via `{% partial %}` marker
   - Detects partials via naming conventions (`_prefix`, contains "partial")
   - Sets default description for partials
   - Uses `Rule::is_partial()` method

5. **Testing**:
   - Comprehensive unit tests for all methods
   - Tests for extension stripping
   - Tests for file detection
   - Tests for frontmatter parsing
   - Tests for partial detection
   - All 52 tests passing

### Key Adaptations from PromptLoader

- Removed all parameter-related logic
- Added severity field parsing with proper enum handling
- Added auto_fix boolean field
- Severity parsing uses `Severity::from_str()` to handle both capitalized and lowercase strings

### Issue Encountered and Resolved

Found a bug in `FileStorage::get()` in storage.rs - it was trying to match severity as capitalized strings ("Error", "Warning", "Info") but the Severity enum serializes as lowercase ("error", "warning", "info"). Fixed by using `s.parse::<Severity>()` which handles both cases correctly via the FromStr implementation.

### Files Modified

- Created: `/Users/wballard/github/sah/swissarmyhammer-rules/src/rule_loader.rs`
- Modified: `/Users/wballard/github/sah/swissarmyhammer-rules/src/lib.rs` (added module declaration and export)
- Fixed: `/Users/wballard/github/sah/swissarmyhammer-rules/src/storage.rs` (severity parsing bug)

### Test Results

```
Summary [   0.029s] 52 tests run: 52 passed, 0 skipped
```

All tests passing including the new RuleLoader tests and the fixed storage tests.


## Code Review Fixes

Fixed all formatting issues identified in the code review:

- Ran `cargo fmt --all` to fix formatting in:
  - `swissarmyhammer-rules/src/frontmatter.rs` - Removed trailing whitespace, reformatted long assertion chains
  - `swissarmyhammer-rules/src/storage.rs` - Removed trailing whitespace, reformatted long lines

All 52 tests in the rules crate continue to pass after formatting changes.

Changes made:
- Trailing whitespace removed from multiple test functions
- Long assertion chains reformatted for better readability
- Multi-line function calls properly formatted
- Rule::new() calls formatted consistently across tests