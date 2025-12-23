# Fixture System - Current Status

## ✅ WORKING: Test Infrastructure Complete

All 52 conformance tests across 8 modules now use `AgentWithFixture` trait:
- Tests receive `mut agent: Box<dyn AgentWithFixture>`
- Call `agent.with_fixture("test_name")` to configure
- Auto-detects: playback if `.fixtures/<agent>/<test>.json` exists, record if not

**Test Pattern (all 52 tests):**
```rust
#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
async fn test_foo(#[case] #[future] mut agent: Box<dyn AgentWithFixture>) {
    agent.with_fixture("test_foo");  // Auto-configures
    test_foo(&*agent).await.expect("...");
}
```

## ✅ llama-agent: FULLY AUTOMATED

**Recording:**
```
1st run: agent.with_fixture("test") → Detects missing → Configures Record mode
         → RecordingGenerationBackend wraps RealBackend
         → Captures streaming chunks
         → Saves to .fixtures/llama/test.json on drop
```

**Playback:**
```
2nd run: agent.with_fixture("test") → Detects exists → Configures Playback mode
         → RecordedGenerationBackend loads fixture
         → Returns pre-recorded chunks
         → 50x faster (0.03s vs 1+ sec)
```

**Implementation:**
- `with_fixture()` uses unsafe to reconfigure `Arc<AgentServer>` fields
- Recreates `generation_backend` with correct mode
- 11+ fixtures auto-recorded

## ⚠️ claude-agent: INFRASTRUCTURE COMPLETE, WIRING IN PROGRESS

**Backend Classes Implemented:**
- ✅ `RealClaudeBackend` - wraps ClaudeProcess
- ✅ `RecordedClaudeBackend` - plays back from JSON (working!)
- ✅ `RecordingClaudeBackend` - captures I/O to JSON (implemented)

**What's Missing:**
- ClaudeClient needs to USE backends instead of process_manager directly
- with_fixture needs to spawn process with RecordingBackend in Record mode
- ~100 lines of wiring in claude.rs to route through backend

**Current Behavior:**
- Playback: Works (when manually created fixtures exist)
- Record: Spawns real Claude, runs test, but doesn't save fixture
- Tests pass but take 14s (real process)

**Next Steps:**
1. Make ClaudeClient check `self.backend` before using `process_manager`
2. In Record mode, spawn ClaudeProcess → wrap in RealBackend → wrap in RecordingBackend
3. Set `self.backend = Some(recording_backend)`
4. All I/O goes through backend, gets captured

## File Sizes

**Test Infrastructure:**
- agent-client-protocol-extras: 128 lines (shared trait)
- agent_fixtures.rs: 63 lines (3 factories, clean!)
- 8 test files: 799 lines total (52 tests, no duplication)

**Reduction:** -446 lines from removing duplicate factories

## Summary

**Working Today:**
- ✅ AgentWithFixture trait - both agents implement it
- ✅ Unified test pattern across 52 tests
- ✅ llama: Full automatic record/playback
- ✅ claude: Playback works, tests pass
- ⚠️ claude: Recording needs ClaudeClient refactoring

**Estimated to Complete Claude Recording:** ~2 hours
- Modify ClaudeClient to use backend when set
- Update spawn logic to create RecordingBackend
- Test end-to-end recording and playback
