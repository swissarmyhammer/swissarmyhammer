---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffff680
title: 'Coverage: enrich_location in layered_context.rs'
---
crates/code-context/src/layered_context.rs

Coverage: 15% (3/20 lines)

Core enrichment logic that reads source text and attaches it to a location. Test with: a valid file and line range, a file that doesn't exist, and a range that exceeds file length. Verify source text is correctly extracted and attached to the output. #coverage-gap