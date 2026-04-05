---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffd480
title: Add tests for file_types.rs functions
---
swissarmyhammer-common/src/file_types.rs:23-98\n\nCoverage: 0% (0/52 lines)\n\nUncovered lines: 23-27, 29, 34-36, 38, 42-43, 47-49, 51, 55-58, 63-66, 71, 75-77, 79, 83-86, 91-94, 98, 120-121, 124-126, 131-136, 139, 143-144\n\nFunctions: is_prompt_file(), has_compound_extension(), is_any_prompt_file(), extract_base_name(), get_prompt_extension(), ExtensionMatcher::new/for_prompts/matches/filter_files\n\nThe file has tests (lines 147-284) but tarpaulin reports 0% on the production code. This may be a coverage instrumentation issue (tests exist but the source lines aren't being traced). Verify by running tests first; if they pass, the fix may be adjusting tarpaulin flags. If coverage is genuinely missing, add unit tests exercising each function with simple/compound extensions, edge cases (no extension, hidden files, case sensitivity). #coverage-gap