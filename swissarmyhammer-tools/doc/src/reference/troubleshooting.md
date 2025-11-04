# Troubleshooting

Common issues and solutions for SwissArmyHammer Tools.

## Installation Issues

### Command Not Found

**Problem**: `sah` command not found after installation.

**Solution**:
1. Check that `~/.cargo/bin` is in your PATH
2. Restart your terminal
3. Verify installation: `cargo install --list | grep swissarmyhammer`

### Permission Denied

**Problem**: Permission denied when running `sah`.

**Solution**:
```bash
chmod +x ~/.cargo/bin/sah
```

## MCP Server Issues

### Server Won't Start

**Problem**: MCP server fails to start.

**Solution**:
1. Check the logs: `RUST_LOG=debug sah serve`
2. Verify working directory permissions
3. Check for conflicting processes

### Claude Code Can't Connect

**Problem**: Claude Code cannot connect to the MCP server.

**Solution**:
1. Verify MCP configuration: `claude mcp list`
2. Check that `sah` is in PATH
3. Restart Claude Code
4. Check logs for error messages

## Next Steps

- [FAQ](faq.md) - Frequently asked questions
- [Configuration](configuration.md) - Configuration options
