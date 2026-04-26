---
position_column: done
position_ordinal: ffffffbd80
title: Add warn_legacy_paths() function to swissarmyhammer-directory
---
Add a `warn_legacy_paths()` function to `swissarmyhammer-directory/src/directory.rs` that checks for old dot-directory paths and logs a warning if they exist. Re-export from lib.rs. Add test.