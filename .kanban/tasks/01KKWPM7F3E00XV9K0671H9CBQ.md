---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffff8780
title: overlayRef in CrossWindowDropOverlay is unused
---
**kanban-app/ui/src/components/cross-window-drop-overlay.tsx:26**\n\n`const overlayRef = useRef<HTMLDivElement>(null)` is declared and attached to the root div but never read. Dead code.\n\n**Suggestion:** Remove `overlayRef` and the `ref={overlayRef}` attribute.