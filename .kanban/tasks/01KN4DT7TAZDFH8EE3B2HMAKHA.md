---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffc080
title: Add tests for FieldDef effective_editor/display/sort
---
swissarmyhammer-fields/src/types.rs\n\nCoverage: 0.0% (0/35 lines)\n\nUncovered functions:\n- FieldDef::effective_editor (13 lines: 126-142)\n- FieldDef::effective_display (13 lines: 148-164)\n- FieldDef::effective_sort (7 lines: 170-178)\n- is_false (2 lines: serde helper)\n\nWhat to test: Construct FieldDef with various type/editor/display/sort combinations. Verify effective_editor returns the explicit editor when set, otherwise the type-based default. Same pattern for display and sort. Test is_false with true/false inputs. #coverage-gap