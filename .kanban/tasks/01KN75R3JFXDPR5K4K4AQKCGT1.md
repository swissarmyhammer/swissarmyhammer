---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffef80
title: 'Test directory file_loader: search paths, dot-directory loading, and stacking'
---
File: swissarmyhammer-directory/src/file_loader.rs (48.4%, 132 uncovered lines)

Uncovered functions:
- FileEntry::from_path_and_content() - alternate constructor
- FileLoader::get_stack() / get_source() - layered file access
- add_search_path() / use_dot_directory_paths()
- load_directory() / load_files_from_dir() - filesystem scanning
- load_all() - full loading pipeline
- get_directories() / get_search_paths()

File: swissarmyhammer-directory/src/yaml_expander.rs (55.8%, 42 uncovered lines):
- YamlExpander::load_all() - loading from search paths
- expand_value() deep recursion edge cases
- parse_yaml<T>() - generic deserialization path

File: swissarmyhammer-directory/src/directory.rs (71.3%, 25 uncovered lines):
- Directory resolution edge cases #coverage-gap