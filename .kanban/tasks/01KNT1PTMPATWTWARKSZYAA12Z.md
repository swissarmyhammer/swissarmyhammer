---
assignees:
- claude-code
depends_on:
- 01KNT1PDFJHP3SAYVQKFFK39GW
position_column: todo
position_ordinal: '8280'
project: null
title: Implement incremental invalidation for symbol tracking
---
When files change, only re-index the changed files and their dependents instead of full workspace invalidation. Track which symbols/definitions were affected and cascade invalidation appropriately.