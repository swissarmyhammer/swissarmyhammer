---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff8f80
title: '[nit] parseFieldMoniker error message says \"no entity id\" when it means \"no entity type separator\"'
---
File: kanban-app/ui/src/lib/moniker.ts (parseFieldMoniker)\n\nWhen the id portion of a field moniker has no colon (e.g., \"field:abc.title\"), the error thrown says \"no entity id\" but the actual problem is that the entityType:entityId separator is missing. A more precise message would help debugging.\n\nSuggestion: Change message to \"Invalid field moniker (missing entity type:id separator): ...\" #review-finding