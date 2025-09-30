# Implement Language Detection Using Tree-sitter

Refer to ideas/rules.md

## Goal

Implement language detection from file paths and content using tree-sitter.

## Context

Rules need to know what programming language a file is to provide context to the LLM. Tree-sitter is already a dependency.

## Implementation

1. Create `src/language.rs` module
2. Implement `detect_language(path: &Path, content: &str) -> Result<String>`:
   - Map file extensions to language names
   - Support all languages tree-sitter knows:
     - Rust (.rs)
     - Python (.py)
     - JavaScript (.js)
     - TypeScript (.ts, .tsx)
     - Dart (.dart)
     - Go (.go)
     - Java (.java)
     - C/C++ (.c, .h, .cpp, .hpp)
     - And others...
   - Return "unknown" for unrecognized extensions
   
3. Add comprehensive extension-to-language mapping

## Testing

- Unit tests for all supported languages
- Unit tests for unknown extensions
- Test with sample files

## Success Criteria

- [ ] Language detection implemented
- [ ] All major languages supported
- [ ] Unit tests passing
- [ ] Works with unknown file types



## Proposed Solution

I'll implement language detection using a simple file extension mapping approach. Based on the ideas/rules.md document, this needs to:

1. Create `src/language.rs` module in the `swissarmyhammer-rules` crate
2. Implement a `detect_language(path: &Path, content: &str) -> Result<String>` function
3. Map file extensions to language names for all languages supported by tree-sitter
4. Return "unknown" for unrecognized extensions

The implementation will:
- Use a straightforward match statement on file extensions
- Support all major languages mentioned in the design doc (Rust, Python, JavaScript, TypeScript, Dart, Go, Java, C/C++, and others)
- Not use tree-sitter parsing (the design doc shows a simple extension mapping approach is sufficient)
- Be usable by the RuleChecker when it's implemented in Phase 5

### Implementation Steps

1. Create `swissarmyhammer-rules/src/language.rs`
2. Add the module declaration to `lib.rs`
3. Implement the `detect_language` function with comprehensive extension mapping
4. Write unit tests for all supported languages
5. Test with unknown extensions
6. Format and build to verify compilation



## Implementation Notes

Successfully implemented language detection for the rules crate:

### What Was Built

1. **Created `src/language.rs` module**:
   - Implemented `detect_language(path: &Path, content: &str) -> Result<String>` function
   - Uses simple file extension mapping (content parameter reserved for future use)
   - Supports 60+ file extensions covering all major programming languages
   - Returns "unknown" for unrecognized extensions

2. **Language Support**:
   - Programming languages: Rust, Python, JavaScript, TypeScript, Go, Java, C/C++, Dart, Kotlin, C#, Ruby, PHP, Swift, Scala, Lua, Haskell, Elixir, and many more
   - Scripting: Shell (sh/bash/zsh), PowerShell, Batch
   - Markup: HTML, XML, CSS, SCSS, Markdown
   - Data formats: JSON, YAML, TOML, SQL, GraphQL, Protobuf

3. **Module Integration**:
   - Added module declaration to `lib.rs`
   - Exported `detect_language` function publicly
   - Follows crate pattern for public API

4. **Testing**:
   - Comprehensive unit tests covering all language categories
   - Tests for multiple extensions per language (e.g., .ts, .tsx, .mts, .cts for TypeScript)
   - Tests for unknown extensions and files without extensions
   - All 96 tests passing in the rules crate

### Design Decisions

- **Simple approach**: Extension-based detection is sufficient for Phase 5 requirements
- **Content parameter**: Included but unused, allowing for future content-based detection if needed
- **Comprehensive coverage**: Covered all languages mentioned in the design doc plus many more
- **Consistent naming**: Language names match tree-sitter conventions (lowercase, hyphen-separated where needed)

### Ready for Phase 5

This language detection module is now ready to be used by `RuleChecker` in Phase 5 when implementing rule checking functionality. The module provides the language context needed for rules to understand what type of code they're analyzing.
