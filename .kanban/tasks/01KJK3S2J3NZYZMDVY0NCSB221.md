---
title: 'Fix TS2353: ''created'' property does not exist on Task type in task-card.test.tsx'
position:
  column: done
  ordinal: a4
---
TypeScript type check (tsc --noEmit) fails with TS2353 in src/components/task-card.test.tsx line 25: "Object literal may only specify known properties, and 'created' does not exist in type 'Task'." The test fixture object includes a 'created' field that is not part of the Task type definition. Either add 'created' to the Task type or remove it from the test fixture.