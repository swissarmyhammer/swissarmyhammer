---
position_column: done
position_ordinal: ffce80
title: '[WARNING] TypeScript ViewDef interface fields lack readonly modifiers'
---
In ui/src/types/kanban.ts, the ViewDef, ViewCommand, and ViewCommandKeys interfaces have mutable properties. Since these are deserialized from the backend and should not be mutated on the frontend, all fields should be marked `readonly` per the Sindre Sorhus TS style guidelines. Same applies to the existing Entity, FieldDef, and other response-only interfaces. #warning