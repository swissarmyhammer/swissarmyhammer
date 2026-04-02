---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffce80
title: Add tests for FileEntry name extraction edge cases
---
src/file_loader.rs:153-154, 299-300, 338-339\n\nCoverage: partial\n\nUncovered lines: 153-154 (remove_compound_extensions fallback), 299-300 (get_source, list)\n\n```rust\nfn remove_compound_extensions(path: &Path) -> &str  // fallback branch\npub fn get_source(&self, name: &str) -> Option<&FileSource>\npub fn list(&self) -> Vec<&FileEntry>\n```\n\nNeed tests for:\n1. File with no recognized extension (hits line 153-154 fallback)\n2. get_source() for existing and missing files\n3. Verify list() returns correct count #coverage-gap