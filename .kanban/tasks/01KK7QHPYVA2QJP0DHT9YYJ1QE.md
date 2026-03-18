---
position_column: done
position_ordinal: ffffff8580
title: 'swissarmyhammer-tools: 46 shell execute tests fail with tempdir panic'
---
All shell execute tests panic at mod.rs:1716 with 'Failed to initialize shell state: No such file or directory'. This is an infrastructure issue with tempdir creation in the test environment. Affected test modules: mcp::tools::shell::execute::tests::*, mcp::tools::shell::tests::* #test-failure