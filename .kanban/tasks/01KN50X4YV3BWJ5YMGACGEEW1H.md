---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffcc80
title: '[nit] detect_mime_type should use a crate like `mime_guess` instead of a hand-rolled map'
---
**File**: `swissarmyhammer-entity/src/io.rs:497-543`\n\n**What**: `detect_mime_type` is a hand-rolled match on ~30 extensions. This will need ongoing maintenance as users attach files with extensions not in the list (e.g. `.wasm`, `.avif`, `.heic`, `.pptx`, `.odt`).\n\n**Suggestion**: Consider using the `mime_guess` crate (`mime_guess::from_path(path).first()`) which covers hundreds of extensions. The current approach works fine for now but will accumulate tech debt." #review-finding