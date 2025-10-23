# Troubleshooting

This guide helps you diagnose and resolve common issues with SwissArmyHammer Tools.

## Common Issues

### Server Won't Start

**Symptoms:**
- Server fails to start
- "Address already in use" error (HTTP mode)
- Permission denied errors

**Solutions:**

1. **Check if port is already in use (HTTP mode):**
   ```bash
   lsof -i :3000  # Check port 3000
   sah serve --http --port 8080  # Use different port
   ```

2. **Verify installation:**
   ```bash
   sah --version  # Should show version number
   which sah  # Should show path to binary
   ```

3. **Check permissions:**
   ```bash
   ls -la ~/.swissarmyhammer/
   mkdir -p ~/.swissarmyhammer/
   chmod 755 ~/.swissarmyhammer/
   ```

4. **Enable debug logging:**
   ```bash
   SAH_LOG_LEVEL=debug sah serve
   ```

### Claude Code Can't Connect

**Symptoms:**
- Tools not available in Claude Code
- Connection timeout errors
- MCP server not responding

**Solutions:**

1. **Verify MCP configuration:**
   ```bash
   cat ~/.config/claude/mcp_settings.json
   ```

   Should contain:
   ```json
   {
     "mcpServers": {
       "sah": {
         "command": "sah",
         "args": ["serve"]
       }
     }
   }
   ```

2. **Check sah is in PATH:**
   ```bash
   which sah
   echo $PATH
   ```

3. **Restart Claude Code** after configuration changes

4. **Check Claude Code logs** for connection errors

5. **Test server manually:**
   ```bash
   sah serve  # Should start without errors
   ```

### Tools Not Available

**Symptoms:**
- Specific tools missing
- "Tool not found" errors
- Empty tool list

**Solutions:**

1. **Verify tool registration:**
   ```bash
   SAH_LOG_LEVEL=debug sah serve 2>&1 | grep "Registered"
   ```

   Should show: `Registered 28 tools`

2. **Check for registration errors:**
   ```bash
   SAH_LOG_LEVEL=debug sah serve 2>&1 | grep -i error
   ```

3. **Verify working directory:**
   ```bash
   sah --cwd /path/to/project serve
   ```

### File Operations Fail

**Symptoms:**
- "Permission denied" errors
- "File not found" errors
- "Invalid path" errors

**Solutions:**

1. **Check file permissions:**
   ```bash
   ls -la /path/to/file
   ```

2. **Verify path is correct:**
   - Use absolute paths or paths relative to working directory
   - Check for typos in filenames
   - Verify file exists

3. **Check path traversal protection:**
   - Server validates all paths for security
   - Paths must be within working directory or allowed directories

4. **Test with simple file:**
   ```bash
   # Create test file
   echo "test" > /tmp/test.txt

   # Start server with /tmp as working directory
   sah --cwd /tmp serve
   ```

### Semantic Search Issues

**Symptoms:**
- Search returns no results
- "Index not found" errors
- Very slow first search
- Out of memory errors

**Solutions:**

1. **Index files first:**
   ```
   Ask Claude: "Index all Rust files for semantic search"
   ```

2. **Check index exists:**
   ```bash
   ls -la .swissarmyhammer/search.db
   ```

3. **Rebuild index if corrupted:**
   ```bash
   rm .swissarmyhammer/search.db
   # Then reindex via Claude
   ```

4. **First search loads model (1-3 seconds):**
   - This is normal
   - Subsequent searches are faster (50-300ms)

5. **Large codebases may need more memory:**
   ```bash
   # Check memory usage
   top -p $(pgrep sah)
   ```

### Git Integration Problems

**Symptoms:**
- "Not a git repository" errors
- Branch detection fails
- Changes not tracked correctly

**Solutions:**

1. **Verify git repository:**
   ```bash
   git status  # Should work in project directory
   ```

2. **Check working directory:**
   ```bash
   sah --cwd /path/to/git/repo serve
   ```

3. **Verify branch exists:**
   ```bash
   git branch --list
   ```

4. **Ensure git is installed:**
   ```bash
   which git
   git --version
   ```

### Performance Issues

**Symptoms:**
- Slow tool execution
- High memory usage
- Timeouts

**Solutions:**

1. **Enable performance logging:**
   ```bash
   SAH_LOG_LEVEL=debug sah serve 2>&1 | grep duration
   ```

2. **Check file sizes:**
   - Large files (>10MB) may be slow
   - Configure max file size if needed

3. **Limit concurrent tools:**
   ```toml
   # ~/.swissarmyhammer/sah.toml
   [performance]
   max_concurrent_tools = 5
   ```

4. **Monitor resource usage:**
   ```bash
   top -p $(pgrep sah)
   htop  # If available
   ```

5. **Check disk I/O:**
   ```bash
   iostat -x 1  # Linux
   sudo fs_usage -w | grep sah  # macOS
   ```

## Debugging Techniques

### Enable Debug Logging

```bash
# Maximum verbosity
SAH_LOG_LEVEL=trace sah serve

# Specific modules
RUST_LOG="swissarmyhammer_tools=debug,swissarmyhammer_search=trace" sah serve
```

### Check Server Status

```bash
# HTTP mode health check
curl http://localhost:3000/health

# List tools
curl http://localhost:3000/tools
```

### Test Tools Individually

Create a test script:

```bash
#!/bin/bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | sah serve
```

### Capture Full Logs

```bash
SAH_LOG_LEVEL=debug sah serve 2>&1 | tee sah-server.log
```

### Verify Configuration

```bash
# Show effective configuration
cat ~/.swissarmyhammer/sah.toml
cat ./.swissarmyhammer/sah.toml

# Check environment
env | grep SAH_
```

## Error Messages

### "Address already in use"

**Cause:** HTTP server port is already bound

**Solution:**
```bash
# Find process using port
lsof -i :3000

# Use different port
sah serve --http --port 8080

# Or kill conflicting process
kill <PID>
```

### "Permission denied"

**Cause:** Insufficient file system permissions

**Solution:**
```bash
# Check permissions
ls -la /path/to/file

# Fix permissions
chmod 644 /path/to/file  # For files
chmod 755 /path/to/dir   # For directories
```

### "Tool not found"

**Cause:** Tool not registered or incorrect name

**Solution:**
- Verify tool name is correct (case-sensitive)
- Check tool list: Ask Claude "What SwissArmyHammer tools are available?"
- Review server logs for registration errors

### "Invalid params"

**Cause:** Tool parameters don't match schema

**Solution:**
- Check required parameters are provided
- Verify parameter types match schema
- Review tool documentation for parameter format

### "Timeout"

**Cause:** Tool execution exceeded timeout

**Solution:**
```bash
# Increase timeout
SAH_TOOL_TIMEOUT=600 sah serve  # 10 minutes
```

Or in configuration:
```toml
[performance]
tool_timeout_seconds = 600
```

## Getting Help

### 1. Check Documentation

- [Getting Started](getting-started.md) - Installation and setup
- [Configuration](configuration.md) - Configuration options
- [Features](features.md) - Tool documentation

### 2. Enable Debug Logging

```bash
SAH_LOG_LEVEL=debug sah serve 2>&1 | tee debug.log
```

### 3. Collect Information

When reporting issues, include:
- SwissArmyHammer version (`sah --version`)
- Operating system and version
- Error messages and logs
- Configuration files
- Steps to reproduce

### 4. Report Issues

- GitHub Issues: https://github.com/swissarmyhammer/swissarmyhammer-tools/issues
- Include debug logs and reproduction steps

## Advanced Debugging

### Memory Profiling

```bash
# Linux: valgrind
valgrind --tool=massif sah serve

# macOS: Instruments
instruments -t Allocations -D sah-memory.trace sah serve
```

### Performance Profiling

```bash
# Linux: perf
perf record -F 99 -g sah serve
perf report

# macOS: sample
sample sah 10 -file sah-profile.txt
```

### Network Debugging

```bash
# HTTP mode: capture traffic
tcpdump -i lo -A port 3000

# Stdio mode: log JSON-RPC
sah serve 2>&1 | tee -a stdio.log
```

## Next Steps

- **[Debugging Guide](troubleshooting/debugging.md)** - Advanced debugging techniques
- **[Performance Guide](troubleshooting/performance.md)** - Optimization strategies
