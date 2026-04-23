---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8280
project: spatial-nav
title: 'Focus bar: make the indicator thinner (4px → 2px)'
---
The focus-bar indicator (the vertical pill painted on every `[data-focused]` element via `::before`) has been thinned from 4px to 2px (`width: 0.125rem`). 

✅ CSS change complete in kanban-app/ui/src/index.css:159
✅ All 1362 frontend tests pass (no regression)
✅ Visual verification: focus-bar appears thinner on cards, grid cells, row selectors, tabs, and inspector rows
✅ No test assertions needed (pure visual change)