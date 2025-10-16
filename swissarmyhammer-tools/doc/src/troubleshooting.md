# Troubleshooting

This guide provides solutions to common problems encountered when using SwissArmyHammer Tools.

## Installation Issues

### Command Not Found After Installation

**Problem**: The `sah` command is not found after running `cargo install swissarmyhammer`.

**Solution**:

1. Ensure Cargo's bin directory is in your PATH:
   ```bash
   export PATH="$HOME/.cargo/bin:$PATH"
   ```

2. Add to your shell profile for persistence:
   ```bash
   # For bash
   echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc

   # For zsh
   echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
   ```

3. Verify installation:
   ```bash
   which sah
   sah --version
   ```

### Permission Denied on Unix Systems

**Problem**: Permission denied when trying to run `sah`.

**Solution**:

```bash
# Make the binary executable
chmod +x ~/.cargo/bin/sah

# Verify permissions
ls -la ~/.cargo/bin/sah
```

### Build Fails During Installation

**Problem**: Cargo build fails with compilation errors.

**Solution**:

1. Ensure you have Rust 1.70 or later:
   ```bash
   rustc --version
   rustup update
   ```

2. Clear build cache and retry:
   ```bash
   cargo clean
   cargo install swissarmyhammer --force
   ```

3. Check for missing system dependencies (Linux):
   ```bash
   # Ubuntu/Debian
   sudo apt-get install build-essential pkg-config libssl-dev

   # Fedora/RHEL
   sudo dnf install gcc openssl-devel
   ```

## Server Issues

### Server Fails to Start

**Problem**: MCP server fails to start with an error.

**Solutions**:

1. **Check for port conflicts (HTTP mode)**:
   ```bash
   # Find process using the port
   lsof -i :8080

   # Use a different port
   sah serve --http --port 8081
   ```

2. **Verify configuration file syntax**:
   ```bash
   # Check for YAML syntax errors
   cat sah.yaml

   # Validate YAML
   python -c "import yaml; yaml.safe_load(open('sah.yaml'))"
   ```

3. **Check permissions on storage directories**:
   ```bash
   # Ensure directories are writable
   ls -la .swissarmyhammer/
   chmod -R u+w .swissarmyhammer/
   ```

### Server Disconnects Unexpectedly

**Problem**: MCP server disconnects during operation.

**Solutions**:

1. **Check server logs**:
   ```bash
   # Run with debug logging
   RUST_LOG=debug sah serve
   ```

2. **Verify file system stability**:
   ```bash
   # Check for disk space
   df -h

   # Check for file system errors
   dmesg | grep -i error
   ```

3. **Increase resource limits**:
   ```bash
   # Check current limits
   ulimit -a

   # Increase file descriptor limit
   ulimit -n 4096
   ```

## Storage Issues

### Storage Directory Not Found

**Problem**: Tools report that storage directory does not exist.

**Solution**:

1. Create directories manually:
   ```bash
   mkdir -p .swissarmyhammer/issues
   mkdir -p .swissarmyhammer/memos
   mkdir -p .swissarmyhammer/workflows
   ```

2. Or let SwissArmyHammer create them automatically (default behavior).

3. Verify directory location:
   ```bash
   # Check if in git repository
   git rev-parse --show-toplevel

   # Storage should be in: <repo-root>/.swissarmyhammer/
   ```

### Cannot Write to Storage

**Problem**: Permission denied when creating issues or memos.

**Solution**:

```bash
# Fix permissions
chmod -R u+w .swissarmyhammer/

# Check ownership
ls -la .swissarmyhammer/

# If owned by different user
sudo chown -R $USER .swissarmyhammer/
```

### Git Repository Not Detected

**Problem**: SwissArmyHammer fails to detect git repository.

**Solutions**:

1. **Initialize git repository**:
   ```bash
   git init
   ```

2. **Use explicit storage paths**:
   ```bash
   export SWISSARMYHAMMER_MEMOS_DIR=/explicit/path/to/memos
   ```

3. **Check git installation**:
   ```bash
   git --version
   which git
   ```

## Prompt Issues

### Prompts Not Appearing in Client

**Problem**: Prompts don't show up in the MCP client (Claude Desktop, etc.).

**Solutions**:

1. **Verify prompt file format**:
   ```markdown
   ---
   name: my-prompt
   description: My prompt description
   arguments:
     - name: arg1
       description: First argument
   ---

   Prompt content here with {{arg1}} template variable.
   ```

2. **Check prompt file location**:
   ```bash
   # Should be in one of these locations:
   # 1. ~/.swissarmyhammer/prompts/
   # 2. .swissarmyhammer/prompts/
   # 3. Bundled in swissarmyhammer-prompts crate

   ls -la ~/.swissarmyhammer/prompts/
   ls -la .swissarmyhammer/prompts/
   ```

3. **Verify prompts are not partial templates**:
   - Partial templates (prefixed with `_`) are not exposed via MCP
   - Rename file if accidentally marked as partial

4. **Check server logs for loading errors**:
   ```bash
   RUST_LOG=debug sah serve 2>&1 | grep prompt
   ```

5. **Restart server to reload prompts**:
   - Prompts are loaded at startup
   - File watching should auto-reload, but manual restart helps

### Prompt Rendering Fails

**Problem**: Prompt template fails to render with variables.

**Solutions**:

1. **Check template syntax**:
   ```liquid
   # Correct Liquid syntax
   Hello {{name}}!

   # Not valid
   Hello ${name}!
   ```

2. **Verify argument names match**:
   - Frontmatter argument names must match template variables
   - Case-sensitive

3. **Provide all required arguments**:
   - Check which arguments are required in prompt frontmatter
   - Supply values when getting prompt

## Tool Execution Issues

### Tool Not Found

**Problem**: MCP client reports tool not found.

**Solutions**:

1. **List available tools**:
   ```bash
   # Via MCP client or check server logs
   RUST_LOG=info sah serve
   # Look for "Registered tool: ..." messages
   ```

2. **Verify tool name spelling**:
   - Tool names use snake_case: `memo_create`, not `memoCreate`
   - Check exact name in error message

3. **Check tool registration**:
   - All tools should register at startup
   - Look for registration errors in logs

### Tool Execution Fails

**Problem**: Tool execution returns an error.

**Solutions**:

1. **Validate tool arguments**:
   ```json
   {
     "path": "/absolute/path/to/file",  // Must be absolute
     "content": "file content"          // All required fields
   }
   ```

2. **Check argument types**:
   - Strings must be quoted
   - Numbers must not be quoted
   - Booleans are `true` or `false`

3. **Review error message**:
   - Error messages indicate which parameter is invalid
   - Check parameter requirements in tool description

4. **Verify file paths**:
   ```bash
   # Tool expects absolute paths
   # Relative paths may fail

   # Good
   /Users/name/project/file.txt

   # May fail
   ./file.txt
   ../file.txt
   ```

## Search Issues

### Semantic Search Returns No Results

**Problem**: Search query returns no results even though code exists.

**Solutions**:

1. **Verify files are indexed**:
   ```bash
   # Check for search database
   ls -la .swissarmyhammer/search.db
   ```

2. **Re-index files**:
   - Use `search_index` tool with `force: true`
   - Indexes only changed files by default

3. **Broaden search query**:
   - Try more general terms
   - Semantic search works best with natural language

4. **Check indexed languages**:
   - Currently supports: Rust, Python, TypeScript, JavaScript, Dart
   - Other languages not indexed

### Search Index Is Large

**Problem**: `.swissarmyhammer/search.db` is very large.

**Solutions**:

1. **Clear and rebuild index**:
   ```bash
   rm .swissarmyhammer/search.db
   # Then re-index with search_index tool
   ```

2. **Index only necessary files**:
   ```bash
   # Instead of indexing everything
   # Index specific directories
   search_index(patterns=["src/**/*.rs"])
   ```

3. **Add `.swissarmyhammer/search.db` to `.gitignore`**:
   - Index database doesn't need to be committed
   - Each developer can build their own

## File Operation Issues

### File Read Fails

**Problem**: Cannot read file with `files_read` tool.

**Solutions**:

1. **Verify file exists**:
   ```bash
   ls -la /path/to/file
   ```

2. **Check file permissions**:
   ```bash
   # Ensure file is readable
   chmod +r /path/to/file
   ```

3. **Use absolute path**:
   - Convert relative to absolute:
   ```bash
   realpath ./relative/path
   ```

4. **Check file size**:
   - Files over 100MB are rejected by default
   - Use `offset` and `limit` for large files

### File Edit Fails with "String Not Found"

**Problem**: `files_edit` reports old_string not found.

**Solutions**:

1. **Verify exact string match**:
   - Must match exactly including whitespace
   - Case-sensitive

2. **Read file first to confirm content**:
   - Use `files_read` to see actual content
   - Copy exact string to replace

3. **String appears multiple times**:
   - Use `replace_all: true` to replace all occurrences
   - Or provide more context to make string unique

4. **Check for invisible characters**:
   - Tabs vs spaces
   - Line ending differences (LF vs CRLF)

## Git Integration Issues

### Git Operations Fail

**Problem**: Git-related tools fail with errors.

**Solutions**:

1. **Verify git is installed**:
   ```bash
   git --version
   ```

2. **Check git configuration**:
   ```bash
   git config --list
   ```

3. **Ensure repository is initialized**:
   ```bash
   git status
   ```

4. **Check for git errors**:
   ```bash
   # Look for .git corruption
   git fsck
   ```

### Branch Changes Not Detected

**Problem**: `git_changes` tool doesn't show expected files.

**Solutions**:

1. **Verify current branch**:
   ```bash
   git branch --show-current
   ```

2. **Check for uncommitted changes**:
   ```bash
   git status
   ```

3. **Verify parent branch detection**:
   - For `issue/*` branches, parent is auto-detected
   - For other branches, may need explicit parent

## Performance Issues

### Server Responds Slowly

**Problem**: Tool execution takes a long time.

**Solutions**:

1. **Check for large files**:
   - Large file operations are slow
   - Use offset/limit for partial reads

2. **Reduce search result counts**:
   - Limit search results with `limit` parameter
   - Default is usually appropriate

3. **Monitor resource usage**:
   ```bash
   # Check CPU and memory
   top -p $(pgrep sah)

   # Check I/O
   iostat -x 1
   ```

4. **Optimize semantic search**:
   - Rebuild search index if very large
   - Index only necessary files

### High Memory Usage

**Problem**: Server uses excessive memory.

**Solutions**:

1. **Limit concurrent operations**:
   - Execute tools sequentially when possible
   - Avoid too many simultaneous operations

2. **Clear search index**:
   ```bash
   rm .swissarmyhammer/search.db
   ```

3. **Restart server periodically**:
   - Server process accumulates memory over time
   - Periodic restarts help

## Claude Desktop Integration Issues

### SwissArmyHammer Tools Not Available

**Problem**: Tools don't appear in Claude Desktop.

**Solutions**:

1. **Verify MCP server configuration**:
   ```json
   {
     "mcpServers": {
       "swissarmyhammer": {
         "command": "sah",
         "args": ["serve"]
       }
     }
   }
   ```

2. **Check command path**:
   ```json
   {
     "mcpServers": {
       "swissarmyhammer": {
         "command": "/Users/name/.cargo/bin/sah",
         "args": ["serve"]
       }
     }
   }
   ```

3. **Restart Claude Desktop**:
   - Changes to MCP configuration require restart
   - Completely quit and reopen application

4. **Check Claude Desktop logs**:
   - Logs location varies by OS
   - Look for MCP server connection errors

### Tools Execute But Return Errors

**Problem**: Tools are available but fail when used.

**Solutions**:

1. **Check working directory**:
   - Claude Desktop may start server in different directory
   - Use absolute paths in tool arguments

2. **Verify permissions**:
   - Server process needs read/write access to project
   - Check file system permissions

3. **Review server logs**:
   ```bash
   # Run server separately to see logs
   sah serve
   ```

## Getting Help

If you encounter issues not covered here:

1. **Check server logs**: Run with `RUST_LOG=debug` for detailed information

2. **Search existing issues**: Visit [GitHub Issues](https://github.com/swissarmyhammer/swissarmyhammer/issues)

3. **Report a bug**: Create a new issue with:
   - SwissArmyHammer version (`sah --version`)
   - Operating system and version
   - Rust version (`rustc --version`)
   - Steps to reproduce
   - Error messages and logs

4. **Join the community**: Discussion forums and community support

## Debug Checklist

When troubleshooting any issue:

- [ ] Check SwissArmyHammer version is up to date
- [ ] Verify Rust version is 1.70 or later
- [ ] Check server logs with `RUST_LOG=debug`
- [ ] Verify file permissions on storage directories
- [ ] Ensure git repository is initialized (if using git features)
- [ ] Check for disk space issues
- [ ] Verify configuration file syntax
- [ ] Try restarting the server
- [ ] Check for conflicting processes or ports
- [ ] Review recent changes that might have caused the issue
