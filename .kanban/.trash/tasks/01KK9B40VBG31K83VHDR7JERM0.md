---
position_column: done
position_ordinal: t5
title: 'CODE-CONTEXT-FIX-0: Review and update existing CODE-CONTEXT cards after DB/TS-writes are done'
---
After FIX-1 and FIX-2 are complete (DATABASE and TREE-SITTER WRITES), evaluate the existing CODE-CONTEXT cards (A, DB, LEADER, WATCHER, QUERIES, SYMBOLS, DUPLICATES, MGMT, SKILL) because many of them have acceptance criteria that assume:

1. Database already exists and is populated ❌
2. Tree-sitter writes are complete ❌  
3. Doctor command CLI integration is done ❌
4. File watcher is implemented ❌

**What to do with each:**
- CODE-CONTEXT-A: Keep - still needed
- CODE-CONTEXT-DB: Modify - DB schema work moves to FIX-1, but startup_cleanup still needs work
- CODE-CONTEXT-LEADER: Keep - still needed
- CODE-CONTEXT-WATCHER: Keep - still needed
- CODE-CONTEXT-QUERIES: Modify - operations already dispatch, but they'll now have actual data to query
- CODE-CONTEXT-SYMBOLS: Modify - same as QUERIES
- CODE-CONTEXT-DUPLICATES: Modify - same as QUERIES  
- CODE-CONTEXT-MGMT: Modify - doctor CLI integration clarification needed
- CODE-CONTEXT-SKILL: Keep or mark done if already exists

After FIX-1 and FIX-2, the QUERIES/SYMBOLS/DUPLICATES cards should just need verification testing since the operations already dispatch correctly.

**This is a meta-task:** Do this review AFTER FIX-1 and FIX-2 are merged, not before.