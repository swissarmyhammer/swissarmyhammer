---
position_column: done
position_ordinal: ffe280
title: handleUpdateField hardcodes entity_type to "task"
---
In `App.tsx` line 143, `handleUpdateField` always passes `entity_type: "task"` to `update_entity_field`. If this callback is ever wired to a non-task entity (e.g. tag or column), it will silently update the wrong entity type. The entity type should be derived from the entity being updated, or the callback should accept it as a parameter. Currently TagInspector has its own separate `updateField` that correctly passes `entity_type: "tag"`, so this is not actively buggy — but it is a latent defect waiting to happen as the code evolves. #warning