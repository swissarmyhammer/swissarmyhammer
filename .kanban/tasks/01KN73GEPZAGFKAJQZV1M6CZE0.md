---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffb080
title: Add tests for ModelManager::load_or_create_config and save_config
---
swissarmyhammer-config/src/model.rs:1881-2025\n\nUncovered lines: 1890, 1892, 1894, 1896-1899, 1904, 1907, 1909, 1912, 1919, 1926, 1937, 1947, 1956-1958, 1989, 2000, 2002, 2005-2008, 2012-2013, 2015, 2018\n\n```rust\nfn load_or_create_config(config_path: &Path) -> Result<serde_yaml_ng::Value, ModelError>\nfn save_config(config_path: &Path, config: &serde_yaml_ng::Value) -> Result<(), ModelError>\nfn check_file_readable(path: &Path) -> Result<(), ModelError>\nfn update_config_with_agent(config: &mut serde_yaml_ng::Value, agent_name: &str) -> Result<(), ModelError>\n```\n\nThese handle config file I/O with security checks. Uncovered: file-too-large check, file read errors, null YAML normalization, non-file path rejection, permission checks, config serialization size guard, and write errors. Test with tempdir: write a valid config then load it, write an empty YAML, test the oversized config guard, and test save_config round-trip. #Coverage_Gap