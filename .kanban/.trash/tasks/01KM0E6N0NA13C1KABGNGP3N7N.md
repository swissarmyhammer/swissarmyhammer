---
assignees:
- claude-code
depends_on:
- 01KM0C2NDHKEB7QXG4XEAQ0KR5
position_column: todo
position_ordinal: 7f8180
title: Fix drag initiation lag — eliminate delay before ghost appears
---
## What
There is a ~1 second delay between starting a drag and the first visible draggable ghost appearing. This makes the drag feel broken and unresponsive.

**Root causes to investigate:**
1. **dom-to-image-more capture latency** — `captureElementAsPng()` is async and may take hundreds of ms to render the DOM to a canvas. If `start_os_drag` waits for this before initiating the OS drag, the ghost is delayed.
2. **`start_os_drag` round-trip** — the frontend calls `invoke('start_os_drag', ...)` which goes through Tauri IPC to Rust, starts the session, then calls `run_on_main_thread` for the OS drag. Each hop adds latency.
3. **@dnd-kit DragOverlay** — the existing DragOverlay should appear instantly (it's React state), but if `handleDragStart` is blocked waiting for the OS drag setup, the React render may be delayed.

**Approach:**
- The @dnd-kit DragOverlay should appear **immediately** on drag start (it already does via `setActiveTask`)
- The OS drag (`start_os_drag`) should be fire-and-forget — don't await it in `handleDragStart`
- Card image capture should happen **in parallel** or be skipped on first frame (use placeholder, then upgrade)
- Consider pre-capturing card images on hover or on idle, so they're ready when drag starts
- Profile the actual bottleneck before fixing — add `performance.now()` markers

## Acceptance Criteria
- [ ] Drag ghost appears within 100ms of pointer activation (5px movement)
- [ ] No perceptible lag between mouse movement and ghost following
- [ ] Card preview image quality is not sacrificed (can be async upgrade)

## Tests
- [ ] Manual test: drag a card — ghost appears instantly, no perceptible delay
- [ ] Manual test: rapid drag-and-drop of multiple cards — each feels snappy
- [ ] `cargo nextest run` — no regressions