---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9280
title: 'Test kanban types: Operation and Position — 57-66% coverage'
---
Files:\n- swissarmyhammer-kanban/src/types/operation.rs: 66/100 (66%) — Operation::with_note, Operation::require_string untested\n- swissarmyhammer-kanban/src/types/position.rs: 22/38 (57.9%) — Position::new, Position::in_column, Ordinal::is_valid untested\n\nNeed unit tests for operation builder methods and position/ordinal validation logic.\n\n#coverage-gap #coverage-gap