---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffae80
title: Add tests for load_yaml_dir
---
registry.rs:198-221\n\n`load_yaml_dir(dir: &Path) -> Vec<(String, String)>` — loads YAML files from a directory.\n\nTest cases:\n1. Non-existent directory returns empty vec\n2. Directory with .yaml files returns (stem, content) pairs\n3. Non-.yaml files are skipped\n4. Empty directory returns empty vec