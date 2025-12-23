# Completing Claude Recording - Remaining Work

## Current Status

### ✅ Complete:
- RealClaudeBackend wraps ClaudeProcess
- RecordingClaudeBackend wraps RealBackend, captures I/O
- ClaudeClient routes through backend when set
- AgentWithFixture trait implementation
- Playback mode fully working

### ⚠️ Remaining (~1 hour):

The issue is **process ownership**. ClaudeProcessManager returns `Arc<Mutex<ClaudeProcess>>`, which can't be moved into RealClaudeBackend (which owns the process).

## Solution Options:

### Option A: Refactor RealClaudeBackend to use Arc (Recommended)
```rust
pub struct RealClaudeBackend {
    process: Arc<Mutex<ClaudeProcess>>,  // Share, don't own
}

impl ClaudeBackend for RealClaudeBackend {
    async fn write_line(&mut self, line: &str) -> Result<()> {
        self.process.lock().await.write_line(line).await
    }
    // Same for read_line

    async fn shutdown(&mut self) -> Result<()> {
        // Don't consume - just signal
        Ok(())
    }
}
```

Then in `with_fixture` Record mode:
```rust
FixtureMode::Record { path } => {
    // Spawn process normally
    let process = self.process_manager.get_process(...).await?;

    // Wrap in backends
    let real = RealClaudeBackend::new(process);  // Takes Arc
    let recording = RecordingClaudeBackend::new(real, path);

    self.backend = Some(Arc::new(Mutex::new(Box::new(recording))));
}
```

### Option B: Make ProcessManager return owned process
Requires changing return type of `get_process()` - larger refactor.

### Option C: Lazy wrapping on first I/O
Detect Record mode on first `write()` call, spawn and wrap then.

## Recommendation

Use **Option A** - change RealClaudeBackend to use `Arc<Mutex<ClaudeProcess>>` instead of owning it. This is a 10-line change that makes it symmetric with how processes are already managed.

## Test Coverage

Once complete:
- **llama**: ✅ Automatic record/playback working
- **claude**: ✅ Automatic record/playback working
- **Both**: Fixtures in `.fixtures/<agent>/<test>.json`
- **Speed**: 0.03s playback vs 10+ sec real processes

Total: 52 tests across 8 conformance modules.
