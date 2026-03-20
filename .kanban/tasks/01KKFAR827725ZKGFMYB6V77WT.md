---
position_column: done
position_ordinal: ffb680
title: '[warning] config overlay replaces rather than merges'
---
**Severity: warning**\n**File:** swissarmyhammer-statusline/src/config.rs:441, 449\n\nThe `load_config()` function loads the builtin YAML first, then if a user or project override file exists, it completely replaces the config (`config = overlay`). This means a user who sets a single field in their override file will lose all other defaults not specified in that file. With `#[serde(default)]`, missing fields fall back to `Default::default()`, which may differ from the builtin YAML values. This could cause surprising behavior." #review-finding