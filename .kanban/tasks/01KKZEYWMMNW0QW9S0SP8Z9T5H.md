---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffcb80
title: tools_override added to SpawnConfig builder but not related to version injection feature
---
**File:** `claude-agent/src/agent.rs:1287` and `claude-agent/src/agent.rs:1695`\n\n**What:** Two functions (`spawn_claude_for_new_session` and `build_mode_spawn_config`) gained a `.tools_override(self.config.claude.tools_override.clone())` call. This change is unrelated to the stated purpose of this PR (version number injection into skill/agent templates). The `tools_override` field is already defined in `SpawnConfig` and `AgentConfig`; these were simply missing call sites.\n\n**Why:** Unrelated changes bundled into a focused PR make the diff harder to review and the commit history harder to bisect. If `tools_override` was missing intentionally (feature flag, gradual rollout) the reason is not documented. If it was an oversight being fixed, it should be a separate commit.\n\n**Suggestion:** Either:\n- Split this into a separate commit with its own message explaining why `tools_override` was missing from these two builder invocations\n- Add a code comment at each site explaining the intent\n\n**Verification:** The two `.tools_override(...)` lines are the only substantive Rust change beyond the version string additions."