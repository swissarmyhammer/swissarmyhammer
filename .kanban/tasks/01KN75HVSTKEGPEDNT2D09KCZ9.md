---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff8e80
title: Test agents operations (list/search/use) — 15-40% coverage
---
Files:\n- swissarmyhammer-agents/src/operations/list_agent.rs: 2/12 (16.7%) — Execute impl untested\n- swissarmyhammer-agents/src/operations/search_agent.rs: 2/13 (15.4%) — Execute impl untested\n- swissarmyhammer-agents/src/operations/use_agent.rs: 2/5 (40%) — Execute impl untested\n\nAll three operation Execute impls nearly untested. Need integration tests with AgentContext.\n\n#coverage-gap #coverage-gap