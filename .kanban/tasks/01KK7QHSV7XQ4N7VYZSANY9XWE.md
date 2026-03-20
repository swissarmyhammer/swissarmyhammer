---
position_column: done
position_ordinal: ffffff8180
title: 'swissarmyhammer-tools: 7 unified_server tests fail with tempdir/port errors'
---
Tests in mcp::unified_server::tests fail due to tempdir creation failures and port-in-use errors in the test environment. Affected: test_http_server_creation_and_info, test_http_server_invalid_port, test_http_server_port_in_use_error, test_server_info_*, test_server_shutdown_idempotency, test_server_with_custom_library, test_stdio_server_task_completion #test-failure