---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffae80
title: Add tests for env variable validation edge cases
---
security.rs:288-329\n\nCoverage: ~50% (several branches uncovered)\n\nUncovered lines: 300-305, 312-314, 319-321\n\n```rust\npub fn validate_environment_variables(...)\n```\n\nMissing coverage for:\n- Value exceeding max_env_value_length (lines 300-305)\n- Value containing null bytes (lines 312-314)\n- Value containing newlines (lines 319-321)\n\nTest each invalid case and verify the correct ShellSecurityError variant is returned. #coverage-gap