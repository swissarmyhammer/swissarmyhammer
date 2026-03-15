---
position_column: done
position_ordinal: '9680'
title: No unit tests for cli_gen or main dispatch logic
---
kanban-cli/src/cli_gen.rs, kanban-cli/src/main.rs\n\nThe cli_gen module has no tests — `build_commands_from_schema`, `extract_noun_verb_arguments`, and the arg extraction functions are all untested. The banner module has tests (good), but the core CLI generation and dispatch path has none.\n\nSuggestion: Add tests for build_commands_from_schema (verify noun-verb structure) and extract_noun_verb_arguments (verify round-trip from schema to args to JSON)."