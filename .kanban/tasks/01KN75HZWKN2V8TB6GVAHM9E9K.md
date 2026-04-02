---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9480
title: Test agent_library and agent_resolver — 58-76% coverage
---
Files:\n- swissarmyhammer-agents/src/agent_library.rs: 14/24 (58.3%) — load_with_resolver, len, names untested\n- swissarmyhammer-agents/src/agent_loader.rs: 32/42 (76.2%) — partial coverage on loading paths\n- swissarmyhammer-agents/src/agent_resolver.rs: 34/46 (73.9%) — Default impl and some resolver paths untested\n- swissarmyhammer-agents/src/agent.rs: 11/20 (55%) — Display impls untested\n- swissarmyhammer-agents/src/parse.rs: 23/35 (65.7%) — partial parse coverage\n\nNeed tests for library loading, agent listing/counting, and resolver edge cases.\n\n#coverage-gap #coverage-gap