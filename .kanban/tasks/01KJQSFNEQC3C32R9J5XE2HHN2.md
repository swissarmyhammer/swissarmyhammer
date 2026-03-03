---
position_column: done
position_ordinal: e8
title: 'WARNING: consider replacing custom diff apply/reverse with similar crate'
---
**Resolution:** Already done. The custom `apply_unified_diff` and `reverse_unified_diff` functions no longer exist. The code now uses `diffy` for everything:\n- `diffy::create_patch(old, new)` for forward patches\n- `diffy::create_patch(new, old)` for reverse patches (both computed at diff time)\n- `diffy::Patch::from_str` + `diffy::apply` for application\n- `reverse_changes` just swaps forward/reverse patches\n\nNo custom parsing or reversal of unified diff text. No code changes needed.\n\n- [x] Evaluate whether similar/diffy API supports direct patch application — yes, diffy::apply\n- [x] Store old+new text hashes — not needed, diffy validates context lines\n- [x] Prototype clean reversal — already in place via dual create_patch at diff time