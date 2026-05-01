---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffbb80
title: ConfigurationDiscovery added swissarmyhammer-directory dependency but only uses it as a passthrough
---
swissarmyhammer-config/Cargo.toml:19\n\nThe new `swissarmyhammer-directory` dependency was added to Cargo.toml. The discovery module imports `DirectoryConfig`, `FileSource`, `SwissarmyhammerConfig`, and `VirtualFileSystem` from it. However, as noted in finding #5, the VFS is only used to check directory existence -- it is not used for its actual file-loading capabilities.\n\nThis adds a dependency edge that may not be justified. The `SwissarmyhammerConfig::DIR_NAME` constant and `find_git_repository_root()` are the only truly useful imports. Consider whether these should live in `swissarmyhammer-common` instead of pulling in the full directory crate.\n\nSuggestion: This is a minor dependency hygiene issue. If the VFS usage is made more meaningful (per finding #5), the dependency is justified. Otherwise consider importing only the needed items from common. #review-finding