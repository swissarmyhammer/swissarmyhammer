---
assignees:
- assistant
position_column: done
position_ordinal: e080
title: Add icon field to OwnedLspServerSpec and update YAML files
---
1. Add pub icon: Option<String> to OwnedLspServerSpec in types.rs\n2. Add icon field to all 5 existing YAML files\n3. Create 7 new YAML files for missing languages\n4. Add all_servers() and servers_for_extensions() to registry.rs\n5. Add tests\n6. Update yaml_loader fallback"