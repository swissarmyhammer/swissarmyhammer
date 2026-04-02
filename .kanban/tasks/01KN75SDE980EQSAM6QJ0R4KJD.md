---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff8d80
title: Test agent, ralph, questions, and skill tools
---
Smaller gaps across specialized tools:

- tools/agent/mod.rs (53.3%, 28 uncovered) - agent tool dispatch and execution
- tools/ralph/execute/mod.rs (60.4%, 44 uncovered) - ralph execution flow
- tools/questions/ask/mod.rs (48.1%, 14 uncovered) - question asking
- tools/questions/persistence.rs (56.2%, 14 uncovered) - question storage
- tools/questions/mod.rs (74.2%, 8 uncovered) - question tool dispatch
- tools/questions/summary/mod.rs (64.7%, 6 uncovered) - summary generation
- tools/skill/mod.rs (63.1%, 31 uncovered) - skill tool dispatch
- tools/skill/use_op.rs (69.0%, 9 uncovered) - skill use operation
- tools/git/changes/mod.rs (56.8%, 35 uncovered) - git changes tool

Total: ~189 uncovered lines across these smaller tools. #coverage-gap