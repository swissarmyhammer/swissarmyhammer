---
assignees:
- claude-code
position_column: todo
position_ordinal: ae80
title: Fix entity-card.test.tsx failures (2 tests)
---
Two failures in src/components/entity-card.test.tsx:\n1. saving edited title calls dispatch_command with correct params\n2. entity.inspect command includes target moniker in context menu\n\n#test-failure #test-failure