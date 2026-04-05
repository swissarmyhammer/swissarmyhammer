---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffe480
title: Add tests for TemplateContext::get_all_agent_configs
---
swissarmyhammer-config/src/template_context.rs:628-654\n\nUncovered lines: 633, 640, 648-649\n\n```rust\npub fn get_all_agent_configs(&self) -> HashMap<String, ModelConfig>\n```\n\nCollects agent configs from three sources: agent.default, flat keys starting with 'agent.configs.', and nested object 'agent.configs'. Uncovered: the flat key iteration path and the nested object iteration path. Test: set agent.default, add agent.configs.workflow1 as a flat key, add agent.configs as a nested Value::Object, and verify all three are collected. #Coverage_Gap