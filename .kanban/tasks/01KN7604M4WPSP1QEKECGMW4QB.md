---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9580
title: Test sem code entity_extractor uncovered branches (class/method/trait extraction)
---
File: crates/swissarmyhammer-sem/src/parser/plugins/code/entity_extractor.rs (70.4% coverage, 131/186 lines)\n\nUncovered code (~55 lines):\n- Class/struct extraction with nested methods (lines 164-191)\n- Trait/interface extraction (lines 200-235)\n- Method extraction within class bodies (lines 266-292)\n- Error handling branches for malformed AST nodes (lines 353-355, 397-400)\n- Various node kind matching fallbacks (lines 419-458)\n\nThe covered code handles function extraction well but class hierarchies and trait definitions lack tests. Create test source files in Rust/Python/TypeScript with classes, traits, and methods." #coverage-gap