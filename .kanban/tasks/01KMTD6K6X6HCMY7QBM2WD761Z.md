---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff9080
title: Add tests for FieldDef::effective_editor/display uncovered match arms
---
swissarmyhammer-fields/src/types.rs:134-141,155-162\n\nCoverage: 63.6% (21/33 lines)\n\nUncovered match arms in effective_editor() and effective_display() when editor/display is None:\n- FieldType::Text → Markdown / Text\n- FieldType::Markdown → Markdown / Markdown\n- FieldType::Color → ColorPalette / ColorSwatch\n- FieldType::Select → Select / Badge\n- FieldType::MultiSelect → MultiSelect / BadgeList\n- FieldType::Reference(multiple:true) → MultiSelect / BadgeList\n\nThe existing tests only cover Date, Number, Reference(single), and Computed. Need tests for Text, Markdown, Color, Select, MultiSelect, and Reference(multiple) field types with no explicit editor/display set. #coverage-gap