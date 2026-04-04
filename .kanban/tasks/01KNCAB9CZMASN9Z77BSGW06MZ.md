---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffd380
title: Fix entity-card.test.tsx failures (2 tests)
---
Two failures in `src/components/entity-card.test.tsx`:\n- saving edited title calls dispatch_command with correct params\n- entity.inspect command includes target moniker in context menu\n\nLikely broken by recent dispatch/inspect/moniker refactors.\n\n#test-failure #test-failure