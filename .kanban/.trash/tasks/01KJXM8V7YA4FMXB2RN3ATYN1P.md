---
position_column: done
position_ordinal: f2
title: 'Fix swissarmyhammer-config test: test_agent_manager_load_project_models panic'
---
The test `model::tests::test_agent_manager_load_project_models` in swissarmyhammer-config intermittently panics at swissarmyhammer-config/src/model.rs:2602 with "Should handle project agent loading gracefully". This is a flaky test that fails when run in the full workspace but may pass in isolation.