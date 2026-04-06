---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9b80
title: 'Test prompts: PromptLibrary rendering, search, and management'
---
File: swissarmyhammer-prompts/src/prompts.rs (43.2%, 192 uncovered lines)

Uncovered functions:
- PromptLibrary::with_storage() - constructor with custom storage backend
- add_directory() - loading prompts from filesystem directories
- render() / render_text() - template rendering with Tera context
- search() - prompt search by query string
- list_filtered() - filtered listing with category/tag criteria
- add() / remove() - programmatic prompt management
- PromptLoader::load_directory() / load_file() / load_from_string() - file loading pipeline

File: swissarmyhammer-prompts/src/storage.rs (21.1%, 86 uncovered lines):
- FileSystemStorage: all methods (new, get_all, insert)
- The entire persistence layer is untested

File: swissarmyhammer-prompts/src/prompt_filter.rs (77.3%, 15 uncovered lines):
- Edge cases in filter matching logic #coverage-gap