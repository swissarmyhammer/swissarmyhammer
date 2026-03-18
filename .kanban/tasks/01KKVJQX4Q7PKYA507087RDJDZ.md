---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffb280
title: 'NIT: IS_QUICK_CAPTURE parsed twice from URLSearchParams'
---
File: kanban-app/ui/src/App.tsx lines 33-34 — `new URLSearchParams(window.location.search)` is constructed twice to check and then get the same `window` parameter. This is a minor clarity issue.\n\nSuggestion: construct the params once:\n```ts\nconst _params = new URLSearchParams(window.location.search);\nconst IS_QUICK_CAPTURE = _params.get(\"window\") === \"quick-capture\";\n```\n\nVerification step: confirm the two URLSearchParams constructions are redundant and can be collapsed to one." #review-finding