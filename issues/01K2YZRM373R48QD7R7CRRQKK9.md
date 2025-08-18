the workflow parser appears to heavily duplicate the prompt parser for frontmatter yaml
## Proposed Solution

After analyzing the code, I found the following duplication between workflow parser (`workflow/parser.rs:197-292`) and prompt parser (`prompts.rs:1591-1614`):

### Duplication Found

1. **YAML Frontmatter Extraction**: Both parsers manually split content on "---\n" markers
2. **YAML Parsing**: Both use `serde_yaml::from_str()` to parse frontmatter
3. **Content Separation**: Both separate frontmatter from main content using similar logic
4. **Error Handling**: Similar error patterns for invalid YAML

### Specific Code Duplication

**Workflow Parser (`extract_parameters_from_frontmatter`)**:
- Lines 207-219: Splits on "---\n", parses YAML
- Uses `serde_yaml::Value` and manual field extraction

**Prompt Parser (`parse_front_matter`)**:
- Lines 1599-1614: Nearly identical splitting and parsing logic  
- Converts to `serde_json::Value` after parsing YAML

### Implementation Plan

1. **Create shared `frontmatter` module** in `swissarmyhammer/src/frontmatter.rs`
   - Extract common frontmatter parsing logic
   - Support both workflow parameters and prompt metadata
   - Use generic return types for flexibility

2. **Refactor workflow parser** to use shared module
   - Replace `extract_parameters_from_frontmatter` with shared function
   - Maintain existing parameter parsing logic
   - Preserve all tests

3. **Refactor prompt parser** to use shared module  
   - Replace `parse_front_matter` with shared function
   - Maintain metadata extraction logic
   - Preserve all tests

4. **Benefits**:
   - Eliminates ~50 lines of duplicated code
   - Single source of truth for frontmatter parsing
   - Consistent error handling across parsers
   - Easier maintenance and bug fixes
## Implementation Complete

Successfully eliminated the frontmatter parsing duplication between workflow and prompt parsers by creating a shared module.

### Changes Made

1. **Created `swissarmyhammer/src/frontmatter.rs`**
   - Shared `parse_frontmatter()` function with consistent error handling
   - Supports both YAML frontmatter and partial templates
   - Complete test coverage with 9 passing tests
   - Proper documentation with usage examples

2. **Refactored `workflow/parser.rs:extract_parameters_from_frontmatter`**
   - Replaced ~95 lines of duplicate parsing logic with 14 lines
   - Now uses shared `parse_frontmatter()` function
   - Updated from `serde_yaml::Value` to `serde_json::Value` for consistency
   - All existing tests still pass (16/16)

3. **Refactored `prompts.rs:parse_front_matter`**  
   - Replaced ~22 lines of duplicate parsing logic with 3 lines
   - Direct delegation to shared function
   - Maintained exact same API and behavior
   - All existing tests still pass (18/18)

4. **Updated `lib.rs`**
   - Exposed new `frontmatter` module in public API

### Results

- **Code Elimination**: Removed ~95 lines of duplicated frontmatter parsing logic
- **Single Source of Truth**: All frontmatter parsing now goes through one function
- **Consistency**: Unified error handling and behavior across parsers
- **Maintainability**: Future frontmatter improvements only need to be made in one place
- **Test Coverage**: 43/43 related tests passing
- **Code Quality**: Passes clippy linting and cargo fmt

### Verified Working

✅ Workflow parameter parsing (`test_parse_workflow_with_parameters`)  
✅ Prompt frontmatter parsing (`test_prompt_loader_loads_only_valid_prompts`)  
✅ Shared frontmatter module (`9 frontmatter tests`)  
✅ All existing functionality preserved  
✅ Code formatting and linting clean