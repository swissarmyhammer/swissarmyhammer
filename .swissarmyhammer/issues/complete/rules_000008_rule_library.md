# Implement RuleLibrary and Build Script

Refer to ideas/rules.md

## Goal

Implement `RuleLibrary` for rule collection management and create build script to embed builtin rules.

## Context

RuleLibrary manages a collection of rules with add/get/list/search operations. The build script embeds builtin rules in the binary.

## Implementation

1. In `src/rules.rs`, implement `RuleLibrary`:
   - Collection management (add/get/list/remove)
   - Search and filtering
   - NO rendering (rules don't render themselves)
   
2. Create `build.rs`:
   - Copy pattern from `swissarmyhammer-prompts/build.rs`
   - Embed `builtin/rules/` directory
   - Generate code to include builtin rules

3. Create `builtin/rules/` directory structure:
   - `builtin/rules/security/`
   - `builtin/rules/code-quality/`
   - `builtin/rules/_partials/`

4. Add basic README explaining builtin rules

## Testing

- Unit tests for library operations
- Test builtin rules are embedded correctly
- Integration test loading builtin rules

## Success Criteria

- [ ] RuleLibrary implementation complete
- [ ] build.rs embeds builtin rules
- [ ] builtin/rules/ directory created
- [ ] Library operations tested
- [ ] Builtin rules accessible



## Proposed Solution

Based on analysis of the existing rules crate, the following components are already implemented:
- ✅ Rule struct with all required fields (name, template, severity, etc.)
- ✅ RuleBuilder for constructing rules
- ✅ Severity enum (Error, Warning, Info, Hint)
- ✅ Storage backends (MemoryStorage, FileStorage)
- ✅ Frontmatter parsing
- ✅ RuleLoader for loading rules from files
- ✅ RuleResolver for hierarchical loading
- ✅ build.rs script for embedding builtin rules (already exists and follows prompt pattern)

**What needs to be implemented:**

### 1. RuleLibrary Implementation (src/rules.rs)

Add to the existing `rules.rs` file:
- `RuleLibrary` struct with HashMap<String, Rule> for collection management
- Methods:
  - `new()` - create empty library
  - `add(rule: Rule)` - add rule to collection
  - `get(name: &str)` - retrieve rule by name
  - `list()` - list all rules
  - `remove(name: &str)` - remove rule from collection
  - `list_filtered(filter: &RuleFilter, sources: &HashMap<String, RuleSource>)` - filtered listing
  - Search/filtering capabilities matching PromptLibrary pattern

### 2. RuleFilter Implementation (new file: src/rule_filter.rs)

Create rule filtering capabilities:
- Filter by source (Builtin, User, Local)
- Filter by category
- Filter by severity
- Filter by tags
- Follow the pattern from PromptFilter

### 3. Builtin Rules Directory Structure

Create the directory structure (currently empty):
```
builtin/rules/
├── security/
│   └── (security rules - to be added later)
├── code-quality/
│   └── (code quality rules - to be added later)
└── _partials/
    └── (partial templates - to be added later)
```

Add a README.md explaining the structure.

### 4. Integration with RuleResolver

Ensure RuleResolver can load builtin rules via the build.rs-generated code:
- The build.rs already exists and generates `get_builtin_rules()`
- RuleResolver needs to use this function to load embedded rules
- Follow the pattern from PromptResolver

### 5. Testing

- Unit tests for RuleLibrary operations (add, get, list, remove, search)
- Integration test verifying builtin rules are embedded and loadable
- Test filtering operations

**Key Design Decisions:**
- NO rendering in RuleLibrary (rules don't render themselves, they're rendered by .check prompt)
- Follow the exact pattern from swissarmyhammer-prompts for consistency
- RuleLibrary manages collection only, not execution
- All filtering logic in RuleFilter struct




## Implementation Notes

### Completed Components

#### 1. RuleLibrary Implementation (src/rules.rs)
Added complete RuleLibrary struct with:
- `new()` - creates empty library with in-memory storage
- `with_storage()` - creates library with custom storage backend
- `add(rule)` - adds single rule to collection
- `add_directory(path)` - loads all rules from directory
- `get(name)` - retrieves rule by name
- `list()` - lists all rules
- `list_names()` - lists all rule names
- `remove(name)` - removes rule from collection
- `search(pattern)` - searches rules by name pattern
- `list_filtered(filter, sources)` - filtered listing with RuleFilter

#### 2. RuleFilter Implementation (new file: src/rule_filter.rs)
Complete filtering capabilities:
- Filter by name pattern (glob support)
- Filter by category
- Filter by tags (any match)
- Filter by source (Builtin, User, Local)
- Filter by severity (Error, Warning, Info, Hint)
- Filter partials (include/exclude)
- Combined filtering support

Follows the exact pattern from swissarmyhammer-prompts/src/prompt_filter.rs

#### 3. Builtin Rules Directory Structure
Created:
```
builtin/rules/
├── README.md           # Comprehensive documentation
├── security/           # Security rules (empty, ready for content)
├── code-quality/       # Code quality rules (empty, ready for content)
└── _partials/          # Partial templates (empty, ready for content)
```

#### 4. README.md Documentation
Added comprehensive documentation explaining:
- Directory structure
- Rule file format
- Available context variables
- Severity levels
- Categories
- How to add new rules
- Partial templates usage

#### 5. Integration Tests
Created comprehensive integration test file:
- tests/rule_library_integration_test.rs
- Tests all RuleLibrary operations
- Tests filtering by all criteria
- Tests source-based filtering
- Tests partial filtering
- Tests validation
- Tests error cases

### Test Results
✅ All 80 tests passing
- Unit tests for Rule, RuleBuilder, validation
- Unit tests for RuleLibrary operations
- Unit tests for RuleFilter
- Integration tests for complete functionality

### Key Design Decisions

1. **NO Rendering in RuleLibrary** - Rules don't render themselves, they're rendered by the .check prompt (will be implemented in future phases)

2. **Storage Backend Pattern** - Uses existing StorageBackend trait (MemoryStorage, FileStorage) for flexibility

3. **Follows Prompts Pattern** - RuleLibrary mirrors PromptLibrary structure for consistency

4. **Comprehensive Filtering** - RuleFilter supports all criteria: name, category, tags, source, severity, partials

5. **Build Script Ready** - build.rs already exists and will embed builtin rules (when added)

### What's Ready for Next Phase

- ✅ RuleLibrary can load and manage rules
- ✅ RuleResolver can use RuleLibrary (already implemented in previous issue)
- ✅ build.rs will embed builtin rules when they're added
- ✅ Directory structure ready for builtin rules
- ✅ All filtering and search capabilities working
- ✅ Integration with existing storage backends

### Future Work (Not in This Issue)

- Add actual builtin rule content files (security, code-quality rules)
- Implement .check prompt (Phase 1 in rules.md)
- Implement RuleChecker with agent integration (Phase 5)
- Add CLI commands (Phase 6-7)

