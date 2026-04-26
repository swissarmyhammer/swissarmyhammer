---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffb680
title: claude_settings_path() delegation in settings.rs lacks doc on why it is kept
---
swissarmyhammer-cli/src/commands/install/settings.rs:130-132\n\nThe function `claude_settings_path()` now delegates to `InitScope::Project.claude_settings_path()`. The module-level doc comment says it is deprecated, but this function still has callers. The function doc says \"Delegates to InitScope::Project\" but does not explain why this wrapper still exists (backward compatibility? re-export convenience?).\n\nSuggestion: Either add a `#[deprecated]` attribute so the compiler warns callers, or add a one-line comment explaining the plan (e.g., \"// TODO: migrate callers to InitScope::Project.claude_settings_path() directly\"). #review-finding