---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffdd80
title: Add tests for ConfigurationDiscovery::for_cli and validate_file_security
---
swissarmyhammer-config/src/discovery.rs:52-239\n\nCoverage: 71.8% (51/71 lines)\n\nUncovered lines: 59-60, 87, 94, 120-127, 181, 219, 221, 227-229, 238-239\n\n```rust\npub fn for_cli() -> ConfigurationResult<Self>\npub fn paths(&self) -> &DiscoveryPaths\nfn validate_file_security(&self, path: &Path) -> ConfigurationResult<()>\nfn resolve_directories() -> (Option<PathBuf>, Vec<PathBuf>)  // debug branch\n```\n\nfor_cli creates a discovery with security validation disabled. validate_file_security checks metadata and readonly permissions. Uncovered: for_cli constructor, paths() accessor, the security validation filter branch in discover_config_files, resolve_directories debug logging when both global and project dirs exist, and the readonly file rejection. Test for_cli returns a valid discovery, test that security validation rejects readonly files, and test the paths accessor. #Coverage_Gap