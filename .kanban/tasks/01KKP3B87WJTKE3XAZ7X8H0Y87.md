---
position_column: done
position_ordinal: '9480'
title: dispatch.rs missing attachment operations
---
swissarmyhammer-kanban/src/dispatch.rs\n\nThe `KANBAN_OPERATIONS` list in schema.rs includes attachment operations (AddAttachment, GetAttachment, UpdateAttachment, DeleteAttachment, ListAttachments), and the schema's `op` enum will include them, but `dispatch::execute_operation` has no match arms for `(Verb::Add, Noun::Attachment)` etc. Running `kanban attachment add ...` will hit the catch-all `_ =>` arm and return \"unsupported operation\".\n\nSuggestion: Add attachment dispatch arms matching the MCP tool's implementation, or remove attachment operations from the schema's operations list."