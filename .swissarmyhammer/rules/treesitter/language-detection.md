---
severity: error
tags:
- integration
- language-detection
---

# Tree-sitter: Language Auto-Detection

## Acceptance Criterion
**AC-2**: Language auto-detected from file extension using existing `detect_language()`

## What to Check
Language detection must:
- Use the existing `detect_language()` function from `swissarmyhammer-rules/src/language.rs`
- NOT reimplement language detection logic
- Automatically determine language from file extension
- Work for all 25+ supported languages

## Success Criteria
- Code imports and calls `detect_language()` from swissarmyhammer-rules crate
- No duplicate language detection implementation exists
- Language detection works for all extensions in supported languages table

## Reference
See specification/treesitter.md - explicitly requires reusing existing infrastructure