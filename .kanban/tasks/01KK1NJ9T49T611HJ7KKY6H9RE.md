---
position_column: done
position_ordinal: ffb780
title: 'W8: Duplicate load_yaml_dir implementations in commands and views crates'
---
The function `load_yaml_dir` is implemented identically in both `swissarmyhammer-commands/src/registry.rs:168-191` and `swissarmyhammer-views/src/context.rs:225-248`. Both read `.yaml` files from a directory into `Vec<(String, String)>` with the same logic. This should be extracted to a shared utility in swissarmyhammer-common or similar.\n\nFile: swissarmyhammer-commands/src/registry.rs:168, swissarmyhammer-views/src/context.rs:225 #review-finding #warning