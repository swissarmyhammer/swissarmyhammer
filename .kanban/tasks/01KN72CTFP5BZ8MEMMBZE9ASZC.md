---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff8480
title: 'WARNING: Enrichment silently swallows ectx.read() failures'
---
kanban-app/src/commands.rs:1473-1511\n\nThe enrichment step (step 5) reads entity data through EntityContext to populate computed fields on WatchEvents. When ectx.read() fails (e.g. file was deleted between flush_changes detecting it and enrichment trying to read it), the error is silently ignored:\n\n```rust\nif let Ok(entity) = ectx.read(entity_type, id).await {\n    *fields = entity.fields.into_iter()...\n}\n```\n\nFor EntityCreated events, this means the frontend receives an event with an empty fields HashMap -- the entity exists on disk but the frontend gets no field data, potentially rendering a blank row.\n\nFor EntityFieldChanged events, the fields remain None, which is the sentinel for \"raw watcher events\". The frontend may interpret this differently than \"enrichment failed\" -- it cannot distinguish between \"no fields available\" and \"enrichment was skipped\".\n\nSuggestion: At minimum, log a warning when enrichment fails. For EntityCreated, consider retrying once or falling back to raw file content. For EntityFieldChanged, consider leaving the event with changes=[] and fields=None and let the frontend request the full entity via get_entity if it needs the data.",
<parameter name="tags">["review-finding"] #review-finding