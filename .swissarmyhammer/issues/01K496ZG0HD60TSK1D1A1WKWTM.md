start_in_process_mcp_server makes a fake http mcp server, it needs to actually use start_http_server, allocating a random port.

the tests should be updated to actually test that we are running our mcp tools like files_read, all the faking and mocking proves nothing.


What is there now is *really bad* -- it is fake, doesn't use our tools or have any chance of doing real work, and ignores the fact that we have rmcp and a real MCP server already implemented.
