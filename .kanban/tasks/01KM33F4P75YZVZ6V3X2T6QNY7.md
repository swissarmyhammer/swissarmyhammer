---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffff8180
title: 'nit: save_tool_config returns std::io::Error for a YAML serialization failure'
---
swissarmyhammer-tools/src/mcp/tool_config.rs:96-103\n\n`save_tool_config` returns `std::io::Result<()>` but uses `std::io::Error::other(...)` to wrap a YAML serialization error. The `io::Error` type is semantically wrong for a serialization failure — callers that match on `io::ErrorKind` will never see `ErrorKind::Other` distinguished from other IO errors.\n\nThis conflates two distinct failure modes (serialization and I/O) into one error type, making it impossible for callers to distinguish why the save failed.\n\nSuggestion: Define a local `SaveError` enum or use `anyhow::Error` for the return type, since this is application (not library) code. Alternatively, if `std::io::Result` must be kept for API consistency, document that `ErrorKind::Other` signals serialization failure.\n\nVerification: Check all callers — the CLI already discards the distinction with `eprintln!(\"Failed to save config: {}\", e)` so a simple `anyhow` return is lowest-risk.\n\n#review-finding #review-finding