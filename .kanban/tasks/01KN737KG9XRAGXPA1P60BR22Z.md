---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffd680
title: Add tests for frontmatter.rs uncovered parsing paths
---
swissarmyhammer-common/src/frontmatter.rs:104-154\n\nCoverage: 62.5% (20/32 lines)\n\nUncovered lines: 104, 108, 125, 138-142, 149, 152-154\n\nKey uncovered areas:\n- Frontmatter extraction when no closing delimiter found (line 104)\n- Empty frontmatter handling (line 108)\n- Content-after-frontmatter extraction (line 125)\n- TOML frontmatter parsing path (lines 138-142)\n- Error cases in frontmatter parsing (lines 149, 152-154)\n\nTest edge cases: document with opening but no closing frontmatter delimiter, empty frontmatter block, TOML-style frontmatter (+++), and malformed frontmatter that fails to parse. #coverage-gap