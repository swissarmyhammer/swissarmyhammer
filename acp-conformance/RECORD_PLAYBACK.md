# Record/Playback System for ACP Conformance Tests

## Overview

The conformance test suite supports both `llama-agent` and `claude-agent`. Since claude-agent interacts with the real Claude API, we have a record/playback system to avoid API calls during test runs.

## Record/Playback Design

### Modes

**Normal Mode** (default)
- Interacts with real Claude API
- Used for production and manual testing

**Record Mode**
```rust
ClaudeAgentMode::Record { 
    output_path: "tests/fixtures/session_name.json" 
}
```
- Calls real Claude API
- Saves all I/O exchanges to JSON file
- Controlled via `CLAUDE_AGENT_MODE=record` environment variable

**Playback Mode**
```rust
ClaudeAgentMode::Playback { 
    input_path: "tests/fixtures/session_name.json" 
}
```
- Replays from recorded JSON fixture
- No API calls, 100-1000x faster
- Controlled via `CLAUDE_AGENT_MODE=playback` environment variable (default for tests)

### Fixture Format

```json
{
  "exchanges": [
    {
      "input": "{\"type\":\"user\",\"message\":{...}}",
      "outputs": [
        "{\"type\":\"assistant\",\"message\":{...}}",
        "{\"type\":\"result\",\"status\":\"success\"}"
      ]
    }
  ]
}
```

## Implementation Notes

### When Record/Playback is Needed

Record/playback is **only needed** for tests that call `agent.prompt()`, which triggers Claude API interaction. Current conformance tests don't need it.

### Future Work

When adding conformance tests that call `prompt()`:

1. **Wire up mode in ClaudeClient**
   - Currently ClaudeClient always uses real ClaudeProcessManager
   - Need to switch to RecordedClaudeBackend when mode is Playback
   - Need to wrap with ClaudeRecorder when mode is Record

2. **Create fixtures**
   - Run test with `CLAUDE_AGENT_MODE=record` once
   - Captures API interactions to fixture file
   - Commit fixture to repo

3. **Run tests in playback**
   - Default mode is playback (via agent_fixtures.rs)
   - Tests read from fixture, no API calls
   - Fast and deterministic

### Example Test Flow

```bash
# One-time: Record a new test
CLAUDE_AGENT_MODE=record cargo nextest run test_complex_prompt

# Normal test runs: Use playback
cargo nextest run test_complex_prompt  # Fast, no API calls
```

## References

- Existing recorded tests: `claude-agent/tests/test_*_recorded.rs`
- Recording utilities: `claude-agent/tests/common/recording.rs`
- Backend trait: `claude-agent/src/claude_backend.rs`
- Mode configuration: `claude-agent/src/config.rs` (ClaudeAgentMode enum)
