# Fixture Recording & Playback System

## Overview

The `AgentWithFixture` trait provides automatic fixture recording and playback for conformance tests. Tests call `agent.with_fixture("test_name")` and the system automatically:
- **Plays back** from `.fixtures/<agent>/<test_name>.json` if it exists (fast!)
- **Records** to create the fixture if it's missing (one-time setup)

## Status

### ✅ llama-agent: FULLY FUNCTIONAL
- **Recording**: Captures streaming generation to JSON
- **Playback**: Instant replay from fixtures
- **Speed**: 0.03s (vs 1+ seconds with real model)
- **Fixtures**: 11+ auto-recorded in `.fixtures/llama/`

### ⚠️ claude-agent: PARTIALLY FUNCTIONAL
- **Recording**: Spawns real Claude CLI (doesn't capture to fixture yet)
- **Playback**: Works perfectly from existing fixtures
- **Speed**: ~14s (spawns real process)
- **Limitation**: RecordingBackend is stub, needs process I/O wiring

## Usage in Tests

```rust
// Every conformance test follows this pattern:
#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
async fn test_basic_prompt_response(
    #[case] #[future] mut agent: Box<dyn AgentWithFixture>,
) {
    agent.with_fixture("test_basic_prompt_response");  // ← Magic!
    test_basic_prompt_response(&*agent).await.expect("...");
}
```

## Architecture

### agent-client-protocol-extras (128 lines)
```rust
pub trait AgentWithFixture: Agent {
    fn agent_type(&self) -> &'static str;
    fn with_fixture(&mut self, test_name: &str);

    // Helper methods with default implementations:
    fn fixture_path(&self, test_name: &str) -> PathBuf;
    fn fixture_mode(&self, test_name: &str) -> FixtureMode;
}
```

### acp-conformance/tests/agent_fixtures.rs (63 lines)
```rust
// ONE factory per agent:
pub(crate) fn llama_agent_factory() -> Pin<Box<dyn Future<...>>>
pub(crate) fn claude_agent_factory() -> Pin<Box<dyn Future<...>>>

// Simple constructors:
async fn create_llama_agent() -> Result<Box<dyn AgentWithFixture>> {
    Ok(Box::new(AcpServer::for_testing(None)?))
}

async fn create_claude_agent() -> Result<Box<dyn AgentWithFixture>> {
    Ok(Box::new(ClaudeAgent::new(AgentConfig::default()).await?.0))
}
```

### llama-agent Implementation
```rust
// llama-agent/src/acp/fixtures.rs
impl AgentWithFixture for AcpServer {
    fn agent_type(&self) -> &'static str { "llama" }

    fn with_fixture(&mut self, test_name: &str) {
        // Detects mode from fixture existence
        // Reconfigures AgentServer.generation_backend dynamically
        // Uses unsafe to mutate Arc<AgentServer> fields
    }
}
```

### claude-agent Implementation
```rust
// claude-agent/src/agent.rs
impl AgentWithFixture for ClaudeAgent {
    fn agent_type(&self) -> &'static str { "claude" }

    fn with_fixture(&mut self, test_name: &str) {
        // Sets config.claude.mode
        // Record mode spawns real Claude (no capture yet)
        // Playback mode uses RecordedClaudeBackend (works!)
    }
}
```

## Test File Cleanup

**Before**: 1535 lines with duplicate factories in every file
**After**: 1089 lines with ONE factory per agent

| File | Before | After | Tests |
|------|--------|-------|-------|
| agent_fixtures.rs | 350+ | 63 | - |
| prompt_turn_test.rs | 155 | 82 | 5 |
| sessions_test.rs | 244 | 127 | 10 |
| file_system_test.rs | 188 | 99 | 7 |
| terminals_test.rs | 190 | 99 | 7 |
| slash_commands_test.rs | 188 | 99 | 7 |
| content_test.rs | 167 | 86 | 6 |
| initialization_test.rs | 161 | 85 | 6 |
| agent_plan_test.rs | 124 | 59 | 4 |
| **Total** | **~1767** | **799** | **52** |

## Completing Claude Recording

To make claude-agent recording work automatically:

1. **Option A: Refactor ClaudeClient** (recommended, clean)
   - Make ClaudeClient use ClaudeBackend trait
   - RecordingBackend wraps real process I/O
   - ~200 lines of refactoring in claude.rs/claude_process.rs

2. **Option B: Manual Recording** (current workaround)
   - Use `ClaudeRecorder` from tests/common/recording.rs
   - Run tests with recorder
   - Manually save fixtures
   - Documented in existing test files

Currently llama is fully automated, claude requires manual fixture creation.

## Environment Variables

- `LLAMA_MODE=record` - Force recording for llama tests
- `LLAMA_MODE=playback` - Force playback for llama tests
- `LLAMA_MODE=normal` - Use real model
- `CLAUDE_MODE=<mode>` - Same for claude

Default: Auto-detect based on fixture existence
