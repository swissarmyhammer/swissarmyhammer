---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffbc80
title: ConfigurationDiscovery loses nested .sah/ directory merging from parent directories
---
swissarmyhammer-config/src/discovery.rs (whole file)\n\nThe old implementation had `find_all_project_config_dirs()` which walked UP the directory tree from CWD to git root, collecting every `.sah/` directory along the way. This allowed workspace-level config at `workspace/.sah/` to merge with project-level config at `workspace/project/.sah/`.\n\nThe new VFS-based implementation only resolves a single project directory (either git-root or CWD). The old test `test_find_all_project_config_dirs_nested` was removed, and the integration test `test_discovery_with_nested_project_structure` was updated to only look at the git-root level.\n\nThis is an intentional behavior change (the card says \"kept Figment for merging\"), but the old behavior was a documented feature with a dedicated test. If any users relied on workspace-level + project-level config merging, this is a silent regression.\n\nSuggestion: Confirm this behavior change is intentional and document it. If nested merging should be preserved, restore the walk-up logic inside `resolve_project_dir()` or add multiple VFS search paths for each `.sah/` ancestor. #review-finding