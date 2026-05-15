---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffdb80
title: 'Phase 1: Strip old undo/redo from EntityContext'
---
Remove changelog_index, transaction_index, current_transaction, undo_stack fields. Simplify write() and delete() to delegate to StoreHandle or fallback. Remove all undo/redo/transaction methods. Update tests.