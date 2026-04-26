---
position_column: done
position_ordinal: ffffffaa80
title: 'warning: avp-common context.rs home_dir field is XDG data but field/method names say home_dir'
---
`avp-common/src/context.rs:115,287,397-419`\n\nThe field `home_dir: Option<ManagedDirectory<AvpConfig>>` and its public accessor `home_validators_dir()` are now populated from `xdg_data()` (line 287: `ManagedDirectory::<AvpConfig>::xdg_data().ok()`), but are still named `home_dir` / `home_validators_dir`. The name is now a lie — this is the XDG data directory, not necessarily `~/`.\n\nOn macOS with a default setup they happen to be the same, but if `XDG_DATA_HOME` is set to `/mnt/data`, `home_validators_dir()` will return `/mnt/data/avp/validators`, which is not in the home directory.\n\nSuggestion: Rename the field to `xdg_data_dir` and the methods to `xdg_validators_dir()` / `ensure_xdg_validators_dir()`. Or at minimum update the doc comments to say \"XDG data directory\" instead of \"user home\". #review-finding