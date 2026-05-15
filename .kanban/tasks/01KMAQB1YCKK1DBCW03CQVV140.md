---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffbf80
title: Fix TypeScript error in entity-commands.test.ts - missing children prop in createElement call
---
src/lib/entity-commands.test.ts:53\n\nError: Argument of type '{ onInspect: Mock<Procedure>; onDismiss: () => boolean; }' is not assignable to parameter of type 'Attributes & InspectProviderProps'.\nProperty 'children' is missing.\n\nFix: add children to the props object in createElement call, or use a different signature.