---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffb480
title: 'nit: board-progress.tsx still guesses done from last column (will be fixed by card 3)'
---
**board-progress.tsx:24-27**\n\nThe docstring says \"Shows done tasks (last column) / total tasks\" and the code still computes done from `board.columns[board.columns.length - 1]`. This is the known-wrong logic that card 3 will fix. The `getNum` import will also become unused after that fix.\n\nNo action needed now — card 01KKSW6MAPB8MAFY8M42224GNB addresses this.\n\n- [ ] Verify this is resolved after card 3 is implemented