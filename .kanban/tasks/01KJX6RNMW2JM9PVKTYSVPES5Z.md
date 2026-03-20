---
position_column: done
position_ordinal: ff9e80
title: 'Card 1: Generic list_entities and get_entity Tauri commands'
---
Add backend commands that return raw entity bags via Entity::to_json(). For tasks, inject computed fields (tags, progress, ready, blocked_by, blocks) into the fields map before serialization. No field renaming, no nesting.\n\nNew function: enrich_task_entity() in task_helpers.rs — injects computed fields into entity.fields.\nNew commands: list_entities(entity_type) and get_entity(entity_type, id) in commands.rs.\nRegister in main.rs.