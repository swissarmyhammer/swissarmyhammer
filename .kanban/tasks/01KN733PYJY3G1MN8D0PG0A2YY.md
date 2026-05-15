---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffbf80
title: Add tests for VFS get_directories and get_search_paths
---
src/file_loader.rs:501-655\n\nCoverage: 0% for both methods\n\nUncovered lines: 503-504, 507-510, 515-518, 520-523, 527, 530, 537, 541, 544-547, 552-555, 557-561, 566, 577-578, 580-581, 584-589, 595-600, 603-608, 612, 614-619, 624-629, 632-638, 648-650, 654\n\n```rust\npub fn get_directories(&self) -> Result<Vec<PathBuf>>\npub fn get_search_paths(&self) -> Vec<SearchPath>\n```\n\nBoth methods mirror load_all's three modes but return paths instead of loading files. Need tests for:\n1. Dot-dir mode with existing/missing directories\n2. Explicit search paths (existing and missing)\n3. Default managed mode with XDG + git root\n4. Fallback to current dir when not in git repo #coverage-gap