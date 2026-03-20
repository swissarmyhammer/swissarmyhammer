---
assignees:
- claude-code
position_column: todo
position_ordinal: '8480'
title: '[blocker] Closing last inspector panel doesn''t update UI'
---
## `kanban-app/ui/src/App.tsx:120-148`\n\nWhen the inspector stack has one panel and `ui.inspector.close` is called, `UIState::inspector_close()` returns `None` (stack was non-empty → now empty). The Rust command serializes `None` as `null`. The frontend's `parsePanelStack` receives `{ result: null }`, returns `null`, and `setPanelStack` is never called — the last panel stays visible.\n\nSame issue with `ui.inspector.close_all` on an already-empty stack (though that's less likely to be triggered).\n\n## Fix\nIn `parsePanelStack`, treat a `null` result as an empty stack:\n```ts\nif (result?.result === null) return [];\n```\n\nOr in the `.then()` handlers:\n```ts\nconst stack = parsePanelStack(res) ?? [];\nsetPanelStack(stack);\n```\n\nAlternatively, fix the Rust side: `inspector_close` could return `Some(InspectorStack(vec![]))` when popping the last entry, instead of `None`.\n\n## Subtasks\n- [ ] Fix parsePanelStack or the .then() handler to clear panels when result is null\n- [ ] Add test: close last panel → panelStack becomes empty\n- [ ] Verify fix works"