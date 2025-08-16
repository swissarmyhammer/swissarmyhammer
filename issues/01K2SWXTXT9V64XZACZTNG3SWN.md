Set the user agent like in
https://github.com/nickclyde/duckduckgo-mcp-server/blob/main/src/duckduckgo_mcp_server/server.py a

Use the UserAgent Rotator.

Only use user agents that appear to be web browsers. Use the user agents in a ring from get_next_user_agent -- don't backstop with the 'not real' user agent of SwissArmyHammer.
## Proposed Solution

The issue is in the User-Agent rotation fallback behavior in `swissarmyhammer-tools/src/mcp/tools/web_search/privacy.rs` at line 150. Currently, when no user agents are configured, the system falls back to `"SwissArmyHammer/1.0 (Privacy-Focused Web Search)"` which looks like a bot user agent.

### Analysis
The current implementation already has:
1. A comprehensive User-Agent rotation system with realistic browser user agents 
2. Proper integration with the DuckDuckGo client that applies user agents via `privacy_manager.get_user_agent()`
3. A `default_user_agents()` function that provides realistic browser user agents

### The Problem
The fallback occurs when `self.user_agents.is_empty()` in the `get_next_user_agent()` method. However, this should never happen in normal operation because:
- The `UserAgentRotator::new()` method always populates `user_agents` with either custom agents or default agents
- The only way for `user_agents` to be empty is if someone explicitly passes `custom_user_agents: Some(vec![])`

### Solution
1. **Remove the fallback to "SwissArmyHammer" user agent** - Since the issue states we should only use browser user agents and never fall back to the "not real" SwissArmyHammer agent
2. **Ensure we always have browser user agents** - Instead of returning the bot user agent, we should ensure default browser user agents are always available
3. **Update the logic** to use default browser user agents when custom agents list is empty

### Implementation Steps
1. Modify the `get_next_user_agent()` method to use default browser agents instead of the SwissArmyHammer fallback
2. Update the `UserAgentRotator::new()` constructor to handle the empty custom agents case properly
3. Add tests to ensure the fallback behavior works correctly with browser user agents

This aligns with the requirement to "Only use user agents that appear to be web browsers" and avoid the "not real" SwissArmyHammer user agent.
## Implementation Complete

### Changes Made

**File: `swissarmyhammer-tools/src/mcp/tools/web_search/privacy.rs`**

1. **Fixed `get_next_user_agent()` method (lines 147-165)**:
   - Removed the fallback to `"SwissArmyHammer/1.0 (Privacy-Focused Web Search)"` 
   - Instead, now falls back to `Self::default_user_agents()` when `user_agents` is empty
   - This ensures only browser user agents are used, never the "not real" SwissArmyHammer agent

2. **Enhanced `UserAgentRotator::new()` constructor (lines 115-128)**:
   - Added `.filter(|agents| !agents.is_empty())` to prevent creating a rotator with empty custom agents
   - This ensures that if someone passes `custom_user_agents: Some(vec![])`, it falls back to default browser agents

3. **Updated test `test_user_agent_rotator_empty_agents` (lines 554-566)**:
   - Changed assertion to verify fallback returns browser user agents (contains "Mozilla")
   - Added negative assertion to ensure it never contains "SwissArmyHammer"

### Verification

- ✅ All 15 privacy tests pass
- ✅ All 49 web search tests pass  
- ✅ Code formatted with `cargo fmt`
- ✅ No clippy warnings
- ✅ Verified that the system now only uses realistic browser user agents

### How It Works

The User-Agent rotation system now ensures that:
1. **Default behavior**: Uses realistic browser user agents from major browsers (Chrome, Firefox, Safari on Windows/macOS/Linux)
2. **Custom agents**: Only uses custom agents if the list is non-empty 
3. **Fallback safety**: Never falls back to bot-like user agents - always uses browser user agents
4. **Ring rotation**: Continues to use the existing `get_next_user_agent()` ring/random selection logic

This aligns perfectly with the issue requirement to "Only use user agents that appear to be web browsers" and to never use the "not real" SwissArmyHammer user agent.