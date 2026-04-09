---
assignees:
- claude-code
position_column: todo
position_ordinal: cb80
title: '[warning] BoardClose handler in commands.rs closes windows without frontend consent'
---
kanban-app/src/commands.rs (dispatch_command_internal, BoardClose handler)

The new BoardClose logic counts how many windows show the board, then closes the requesting window if more than one visible window exists:

```rust
if visible_windows.len() > 1 {
    if let Some(win) = app.get_webview_window(&requesting_label) {
        let _ = win.close();
    }
}
```

This is a significant behavior change: previously, closing a board just cleared the board from the window. Now it closes the entire window. There are two concerns:

1. The window close happens from the Rust side without the frontend having a chance to save state or clean up React trees.
2. If the user has unsaved work in other panels of that window (inspector open, form in progress), it will be lost without warning.

This may be intentional UX (close board = close window in multi-window), but it is a potentially surprising behavior change.

Suggestion: Consider emitting a `board-close-requested` event to the frontend first, letting it clean up before the window is destroyed. Or document this as the intended behavior. #review-finding