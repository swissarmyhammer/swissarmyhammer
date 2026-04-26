---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffee80
title: Add tests for error.rs constructor helpers and severity impl
---
swissarmyhammer-common/src/error.rs:302-460\n\nCoverage: 57.6% (38/66 lines)\n\nUncovered lines: 302-303, 307-308, 312, 317, 322, 379, 388, 413-416, 420, 425-427, 436-437, 439-440, 442-445, 448, 459-460\n\nKey uncovered functions:\n- SwissArmyHammerError::directory_creation (line 302)\n- SwissArmyHammerError::directory_access (line 307)\n- SwissArmyHammerError::invalid_path (line 312)\n- SwissArmyHammerError::io_context (line 317)\n- SwissArmyHammerError::semantic (line 322)\n- Severity impl: several match arms in the severity() method for variants like WorkflowStepFailed, PromptExecution, Serialization, DeserializeYaml, DeserializeToml, Other, Semantic, IoContext\n\nThe constructor helpers are straightforward factory methods. Test each one creates the correct variant. For the Severity impl, test that each error variant returns the expected severity level. #coverage-gap