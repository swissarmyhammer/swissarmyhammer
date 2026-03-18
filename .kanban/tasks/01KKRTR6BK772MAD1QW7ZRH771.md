---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffa080
title: Fix Enter/Escape in quick-capture text input
---
## What

Enter and Escape do not work in the quick-capture "What needs to be done?" input. Enter adds a newline instead of submitting. Escape does nothing instead of dismissing.

The `FieldPlaceholderEditor` uses `buildSubmitCancelExtensions` which was changed to use DOM-level event handlers. The issue needs investigation — likely the markdown extension or vim mode is intercepting events before the submit/cancel handlers fire.

## Acceptance Criteria
- [ ] Enter in the quick-capture input submits the task and hides the window
- [ ] Escape in the quick-capture input hides the window
- [ ] Both work in vim mode (Enter in normal mode submits, Escape in normal mode dismisses)
- [ ] Both work in CUA/emacs mode
- [ ] Existing field editing in inspector and grid is not broken

## Tests
- [ ] Manual: type text, press Enter → task created, window hides
- [ ] Manual: press Escape → window hides, no task created
- [ ] Manual: edit a field in the inspector → Enter/Escape still work correctly