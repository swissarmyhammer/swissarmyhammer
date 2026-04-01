---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffce80
title: ConfigurationDiscovery::resolve_directories_via_vfs creates a VFS it does not actually use
---
swissarmyhammer-config/src/discovery.rs:109-151\n\nThe `resolve_directories_via_vfs` method creates a `VirtualFileSystem<SwissarmyhammerConfig>`, adds search paths to it, then calls `get_directories()` only to check which of the already-known paths exist on disk. The VFS is not used for file loading, dot-directory resolution, or any other VFS feature.\n\nThis is equivalent to:\n```rust\nlet resolved_global = global_dir.filter(|d| d.exists() && d.is_dir());\nlet resolved_project = project_dir.filter(|d| d.exists() && d.is_dir());\n```\n\nThe VFS adds overhead (type parameterization, error mapping) without value. The method doc comment says \"The VFS is not used for file loading... only for directory resolution\" which confirms this.\n\nSuggestion: Either simplify to direct `exists()` checks (less code, same behavior), or use the VFS more fully (e.g. `use_dot_directory_paths()` like the other resolvers do, letting the VFS handle HOME/git-root resolution). #review-finding