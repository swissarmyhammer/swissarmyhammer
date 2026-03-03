---
position_column: done
position_ordinal: h3
title: Add field-name validation to update_entity_field IPC
---
**Done.** Added field-name validation against EntityDef.fields before allowing writes. Unknown fields now return a clear error message.\n\n- [x] Validate field_name against EntityDef before writing\n- [x] Return clear error: "field 'x' is not defined for entity type 'y'"\n- [x] Clippy clean