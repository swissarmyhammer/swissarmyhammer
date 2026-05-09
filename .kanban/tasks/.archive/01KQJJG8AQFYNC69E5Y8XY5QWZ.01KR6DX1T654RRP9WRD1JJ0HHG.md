---
assignees:
- claude-code
position_column: todo
position_ordinal: b980
title: 'Fix inspectors-container: opening second panel — z.moniker is undefined in registeredZones filter'
---
**Pre-existing failure** (verified against HEAD without 01KQJDYJ4SDKK2G8FTAQ348ZHG changes).

File: `kanban-app/ui/src/components/inspectors-container.test.tsx:463`

Error:
```
TypeError: Cannot read properties of undefined (reading 'startsWith')
  const panelZones = registeredZones().filter((z) =>
    z.moniker.startsWith("panel:"),  // ← z.moniker is undefined
  );
```

The mock or registry is returning zone objects whose `moniker` property is undefined. Either the test's `registeredZones()` helper has fallen out of sync with the zone-registration shape (segment vs moniker rename?), or one of the registered zones genuinely has no moniker. Either way the test needs to be updated to match the current registration shape. #test-failure