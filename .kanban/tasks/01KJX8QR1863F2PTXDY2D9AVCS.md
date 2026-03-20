---
position_column: done
position_ordinal: ffe680
title: Missing error context on Rust .map_err(|e| e.to_string()) chains
---
Per the Rust review guidelines, bare `.map_err(|e| e.to_string())` loses error context. In `commands.rs`, nearly every command uses this pattern (e.g. lines 54, 123, 144, 175, etc.). When these errors reach the frontend, the user sees raw error messages like "No such file or directory" with no context about which operation failed.

The Tauri command boundary is the application boundary, so `anyhow` with `.context()` would be appropriate here. However, since Tauri commands require `Result<T, String>`, a pragmatic fix is to use `.map_err(|e| format!("failed to {}: {}", description, e))` for key operations. #warning