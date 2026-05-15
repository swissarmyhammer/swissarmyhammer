---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffe980
title: 'Test unified_server: server startup, HTTP/stdio modes, and logging'
---
File: swissarmyhammer-tools/src/mcp/unified_server.rs (56.4%, 95 uncovered lines)

Uncovered functions:
- configure_mcp_logging() - tracing subscriber setup
- start_mcp_server_with_options() - full server startup with config
- resolve_port() - port resolution logic
- initialize_mcp_server() - server init sequence
- create_mcp_router() - axum router construction
- parse_socket_addr() / bind_tcp_listener()
- setup_http_server_for_stdio() / spawn_stdio_server_task()
- start_stdio_server() - stdio transport mode
- spawn_http_server_task() / create_tcp_listener()
- wait_for_server_ready() / start_http_server()

Many of these are infrastructure that may need integration-level tests or careful mocking of network I/O. #coverage-gap