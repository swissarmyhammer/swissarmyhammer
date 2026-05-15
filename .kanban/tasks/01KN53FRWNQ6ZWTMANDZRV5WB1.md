---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffa880
title: 'store.rs: missing test for empty entity serialization (both YAML and frontmatter paths)'
---
swissarmyhammer-entity/src/store.rs:97-135 (serialize)\n\nThere are no tests for serializing an entity with zero fields. For the plain YAML path, this would produce an empty YAML document (likely `{}\n`). For the frontmatter path, this would produce `---\n{}\n---\n` with an empty body. Both are valid edge cases that should be verified.\n\nSuggestion: Add two tests: (1) serialize an entity with no fields via the plain YAML path and verify the output is valid YAML, (2) serialize an entity with only a body field via the frontmatter path and verify the output has empty frontmatter. Severity: nit. #review-finding