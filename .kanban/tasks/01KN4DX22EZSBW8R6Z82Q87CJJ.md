---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffc580
title: Add tests for ValidationEngine (JS validation, reference validation)
---
swissarmyhammer-fields/src/validation.rs\n\nCoverage: 71.7% (38/53 lines)\n\nUncovered functions:\n- run_js_validation (3 lines: 115-117)\n- default_reference_validation (4 lines: 136-167)\n- validate_entity (6 lines: 204-213)\n\nWhat to test: Construct a ValidationEngine, add rules, then validate entities. Test that reference validation catches dangling references. Test validate_entity aggregates errors from all rules. #coverage-gap