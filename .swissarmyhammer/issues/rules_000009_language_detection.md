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
