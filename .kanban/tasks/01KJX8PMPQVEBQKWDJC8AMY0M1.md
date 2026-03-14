---
position_column: done
position_ordinal: g6
title: 'Fix failing app-shell test: app.save command removed'
---
The test in `app-shell.test.tsx` line 66 expects a `cmd-app.save` command to exist, but the `app.save` command was removed during the refactor. The test suite has 1 failing test (`app-shell.test.tsx`). Either add back an `app.save` command or update the test to match the new command set. #blocker