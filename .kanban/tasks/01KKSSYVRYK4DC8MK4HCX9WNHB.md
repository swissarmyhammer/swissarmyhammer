---
assignees:
- claude-code
position_column: done
position_ordinal: fffffff480
title: Fix quick-capture window border artifact
---
The Tauri window behind the quick-capture popup card still shows a visible border/frame artifact. The window is configured with `decorations: false` and `transparent: true` in tauri.conf.json, but there's still an ugly border visible around the transparent region.

This likely needs a fix on the Rust/Tauri side — possibly adjusting the window background color, shadow settings, or platform-specific transparency handling (e.g., macOS vibrancy/titlebar settings).

- [ ] Investigate what's causing the border artifact (shadow, background, platform chrome)
- [ ] Fix the transparent window to have no visible border or frame
- [ ] Verify fix on macOS