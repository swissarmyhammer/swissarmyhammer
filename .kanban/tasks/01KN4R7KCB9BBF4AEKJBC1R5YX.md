---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffaf80
title: SkillResolver lost extra_paths field but add_search_path still works via VFS delegation
---
swissarmyhammer-skills/src/skill_resolver.rs:57-59\n\nThe old SkillResolver had both a VFS and an `extra_paths: Vec<PathBuf>` field, with a separate loop for extra paths in `resolve_all()`. The new implementation correctly delegates `add_search_path` to `self.vfs.add_search_path(path, FileSource::Local)`, which means extra paths are now resolved by the VFS's `get_search_paths()` method.\n\nThis is correct and cleaner, but the hardcoded `FileSource::Local` means all extra paths are tagged as Local. If a caller wanted to add a User-precedence extra path, there is no way to do so. The AgentResolver kept the separate `extra_paths` field, creating an inconsistency between the two resolvers.\n\nSuggestion: Make both resolvers consistent. Either both should delegate to VFS (preferred, simpler), or both should keep `extra_paths`. If delegating to VFS, consider adding an `add_search_path_with_source` variant or making the existing method accept a FileSource parameter. #review-finding