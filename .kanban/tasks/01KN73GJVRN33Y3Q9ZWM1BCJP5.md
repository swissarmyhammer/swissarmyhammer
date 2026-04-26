---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9f80
title: Add tests for ModelManager::validate_config_file_path and check_directory_writable
---
swissarmyhammer-config/src/model.rs:1627-1702\n\nUncovered lines: 1631, 1640, 1648-1650, 1660-1661, 1668, 1670, 1674, 1678, 1683, 1685, 1688, 1693\n\n```rust\nfn check_directory_writable(path: &Path) -> Result<(), ModelError>\nfn validate_config_file_path(path: &Path) -> Result<PathBuf, ModelError>\n```\n\ncheck_directory_writable validates path is a dir and checks Unix write permissions. validate_config_file_path checks for empty path, overly long path, suspicious patterns, canonicalization of existing files, and non-file paths. Test: empty path, path exceeding 4096 chars, path with suspicious patterns, path pointing to a directory instead of a file, and a valid file path. #Coverage_Gap