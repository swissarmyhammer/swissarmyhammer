---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffec80
title: Test sem compute_semantic_diff end-to-end
---
File: crates/swissarmyhammer-sem/src/parser/differ.rs (0% coverage, 0/45 lines)\n\nEntirely untested. Single public function:\n- compute_semantic_diff() - takes FileChange list and ParserRegistry, extracts before/after entities per file in parallel, matches entities, counts change types (Added/Modified/Deleted/Moved/Renamed)\n\nThis is the core semantic diff engine. Test with synthetic FileChange inputs containing before_content and after_content for known file types (e.g. JSON, YAML, code). Verify correct change type counts and that parallel processing works." #coverage-gap