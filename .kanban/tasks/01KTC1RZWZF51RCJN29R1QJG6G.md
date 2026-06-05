---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff680
project: plugin-arch
title: Fix plugin e2e test compile break (new_with_work_dir 4-arg vs 3-arg) — suite can't build
---
D4 from the runtime-integration audit. The swissarmyhammer-plugin TEST target does not compile: `McpServer::new_with_work_dir` is called with 4 args in tests but the production signature takes 3 (`library, work_dir, model_override`).

Call sites (4-arg, broken): crates/swissarmyhammer-plugin/tests/support/mod.rs:255, crates/swissarmyhammer-plugin/tests/callback_e2e.rs:316, crates/swissarmyhammer-tools/tests/plugin_module_exposure_test.rs:50. The shared `support/mod.rs::build_mcp_server` helper poisons ~8 plugin e2e targets (operation_meta_e2e, cli_server_e2e, file_notes_e2e, server_name_collision_e2e, callback_e2e, multi_plugin_shared_server_e2e, cli_echo_e2e, example_layering_e2e) with E0061.

Production libs compile clean — this is test-only. But it means the plugin e2e suite (including the real callback path that would regression-guard the D1 task-local seam) can't run. Fix: align the call sites to the 3-arg signature (drop the extra arg) — confirm the 3-arg signature is the intended one (server.rs:271-275). Then `cargo test -p swissarmyhammer-plugin` builds + runs green. Recommended to fix BEFORE D1 so a real-path callback test can regression-guard the seam.