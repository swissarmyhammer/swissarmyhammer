# Troubleshooting

Common issues and solutions for SwissArmyHammer installation, configuration, and usage.

## Installation Issues

### Binary Not Found

**Problem**: `sah: command not found`

**Solutions**:
```bash
# Check if sah is in PATH
which sah
echo $PATH

# Add to PATH (add to ~/.bashrc or ~/.zshrc)
export PATH="$PATH:/usr/local/bin"

# Or install to user directory
cargo install swissarmyhammer-cli --root ~/.local
export PATH="$PATH:~/.local/bin"

# Verify installation
sah --version
```

### Permission Denied

**Problem**: Permission errors when running sah

**Solutions**:
```bash
# Fix binary permissions
chmod +x $(which sah)

# Fix directory permissions
chmod -R 755 ~/.swissarmyhammer
chmod 600 ~/.swissarmyhammer/sah.toml
```

### Build Failures

**Problem**: Compilation errors when building from source

**Solutions**:
```bash
# Update Rust toolchain
rustup update

# Clear cargo cache
cargo clean

# Install with specific features
cargo install swissarmyhammer-cli --no-default-features --features basic

# Check system dependencies
# On Ubuntu/Debian:
sudo apt-get update
sudo apt-get install build-essential pkg-config libssl-dev

# On macOS:
xcode-select --install
brew install openssl pkg-config
```

## Configuration Issues

### Configuration Not Loading

**Problem**: `sah doctor` shows configuration errors

**Solutions**:
```bash
# Check configuration file syntax
sah config show --format json

# Validate configuration
sah validate --config

# Reset to defaults
mv ~/.swissarmyhammer/sah.toml ~/.swissarmyhammer/sah.toml.backup
sah doctor --fix

# Check file permissions
ls -la ~/.swissarmyhammer/sah.toml
chmod 644 ~/.swissarmyhammer/sah.toml
```

### Directory Structure Issues

**Problem**: Prompts or workflows not found

**Solutions**:
```bash
# Check directory structure
sah doctor --check directories

# Create missing directories
mkdir -p ~/.swissarmyhammer/{prompts,workflows,memoranda,issues}

# Check file permissions
find ~/.swissarmyhammer -type d -exec chmod 755 {} \;
find ~/.swissarmyhammer -type f -exec chmod 644 {} \;

# List prompt sources
sah prompt list --format table
```

### Environment Variables

**Problem**: Environment variables not recognized

**Solutions**:
```bash
# Check environment variables
env | grep SAH_

# Set in shell profile
echo 'export SAH_HOME="$HOME/.swissarmyhammer"' >> ~/.bashrc
echo 'export SAH_LOG_LEVEL="info"' >> ~/.bashrc
source ~/.bashrc

# Test with explicit environment
SAH_LOG_LEVEL=debug sah doctor
```

## MCP Integration Issues

### Claude Code Connection Failed

**Problem**: MCP server not connecting to Claude Code

**Solutions**:
```bash
# Test MCP server directly
sah serve --stdio
# Type: {"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
# Should get initialization response

# Check MCP configuration
claude mcp list
claude mcp status sah

# Reconfigure MCP
claude mcp remove sah
claude mcp add --scope user sah sah serve

# Check logs
tail -f ~/.config/claude-code/logs/mcp.log
```

### MCP Tools Not Available

**Problem**: SwissArmyHammer tools not showing up in Claude Code

**Solutions**:
```bash
# Check tool registration
sah serve --stdio
# Look for tools in initialize response

# Enable specific tools
sah config set mcp.enable_tools '["issues","memoranda","search","abort"]'

# Restart Claude Code
# Kill Claude Code process and restart

# Check tool permissions
sah config show | grep mcp
```

### MCP Timeout Issues

**Problem**: MCP requests timing out

**Solutions**:
```bash
# Increase timeout
sah config set mcp.timeout_ms 60000

# Check system performance
sah doctor --check performance

# Monitor MCP server
SAH_LOG_LEVEL=debug sah serve

# Reduce concurrent requests
sah config set mcp.max_concurrent_requests 5
```

## Prompt Issues

### Prompt Not Found

**Problem**: `sah prompt test my-prompt` returns "not found"

**Solutions**:
```bash
# List available prompts
sah prompt list

# Check specific source
sah prompt list --source user
sah prompt list --source local
sah prompt list --source builtin

# Check file location and name
ls -la ~/.swissarmyhammer/prompts/
ls -la ./.swissarmyhammer/prompts/

# Validate prompt syntax
sah prompt validate my-prompt
```

### Template Rendering Errors

**Problem**: Liquid template errors during prompt rendering

**Solutions**:
```bash
# Check template syntax
sah prompt validate my-prompt --strict

# Test with verbose logging
SAH_LOG_LEVEL=debug sah prompt test my-prompt --var key=value

# Check argument requirements
sah prompt show my-prompt

# Test incrementally
sah prompt test my-prompt --var required_arg=value
```

### Variable Substitution Issues

**Problem**: Variables not being substituted correctly

**Solutions**:
```bash
# Check argument names and types
sah prompt show my-prompt --format json

# Use correct variable format
sah prompt test my-prompt --var "name=value with spaces"

# Check for typos in template
sah prompt show my-prompt --raw | grep -n "{{.*}}"

# Test with variables file
echo '{"name":"value"}' > vars.json
sah prompt test my-prompt --vars-file vars.json
```

## Workflow Issues

### Workflow Execution Failures

**Problem**: Workflows failing to execute

**Solutions**:
```bash
# Validate workflow syntax
sah flow validate my-workflow --strict

# Test with dry run
sah flow run my-workflow --dry-run

# Check state machine
sah flow show my-workflow --diagram

# Run with verbose logging
SAH_LOG_LEVEL=debug sah flow run my-workflow
```

### Shell Action Failures

**Problem**: Shell actions in workflows failing

**Solutions**:
```bash
# Check allowed commands
sah config show | grep security.allowed_commands

# Add required commands
sah config set 'security.allowed_commands ["git","npm","cargo","python"]'

# Check working directory
pwd
ls -la

# Test shell commands manually
git status
npm --version

# Increase shell timeout
sah config set workflow.actions.shell.timeout_ms 120000
```

### State Transition Issues

**Problem**: Workflow stuck in specific state

**Solutions**:
```bash
# Check state definitions
sah flow show my-workflow

# Validate transition logic
sah flow validate my-workflow --check-cycles

# Start from different state
sah flow run my-workflow --start-state next-state

# Check workflow logs
tail -f ~/.swissarmyhammer/logs/workflow.log
```

## Search Issues

### Indexing Failures

**Problem**: `sah search index` fails or crashes

**Solutions**:
```bash
# Check available disk space
df -h ~/.swissarmyhammer

# Check file permissions
ls -la ~/.swissarmyhammer/search.db

# Clear existing index
rm ~/.swissarmyhammer/search.db
sah search index "**/*.rs"

# Index smaller batches
sah search index "src/**/*.rs"
sah search index "tests/**/*.rs"

# Check memory usage
sah config set search.max_file_size 524288  # 512KB
```

### Search Results Empty

**Problem**: Search queries return no results

**Solutions**:
```bash
# Check if files are indexed
ls -la ~/.swissarmyhammer/search.db

# Verify indexed patterns
sah search index "**/*.rs" --force

# Test different queries
sah search query "function"
sah search query "struct"

# Lower similarity threshold
sah search query "my query" --threshold 0.3

# Check indexed file types
sah doctor --check search
```

### Embedding Model Issues

**Problem**: Embedding model download or loading failures

**Solutions**:
```bash
# Check internet connection
curl -I https://huggingface.co

# Clear model cache
rm -rf ~/.swissarmyhammer/models

# Use different model
sah config set search.embedding_model "all-MiniLM-L6-v2"

# Increase download timeout
sah config set search.model_download_timeout 600000

# Check available storage
df -h ~/.swissarmyhammer
```

## Issue Management Problems

### Git Integration Failures

**Problem**: Issue commands failing with git errors

**Solutions**:
```bash
# Check git repository
git status
git remote -v

# Configure git if needed
git config --global user.name "Your Name"
git config --global user.email "your.email@example.com"

# Check branch permissions
git branch -a
git checkout main

# Fix branch issues
git checkout main
git branch -D issue/problem-branch
sah issue work problem-issue
```

### Branch Creation Issues

**Problem**: Cannot create branches for issues

**Solutions**:
```bash
# Check current git status
git status
git stash  # if needed

# Check branch naming
sah config show | grep issues.branch_pattern

# Use custom branch pattern
sah config set 'issues.branch_pattern "feature/{{name}}"'

# Manual branch creation
git checkout -b issue/my-issue
sah issue work my-issue
```

### Issue File Corruption

**Problem**: Issue files corrupted or unreadable

**Solutions**:
```bash
# Check file encoding
file ~/.swissarmyhammer/issues/my-issue.md

# Validate issue files
sah validate --strict

# Backup and restore
cp ~/.swissarmyhammer/issues/my-issue.md ~/.swissarmyhammer/issues/my-issue.md.backup
editor ~/.swissarmyhammer/issues/my-issue.md

# Check file permissions
chmod 644 ~/.swissarmyhammer/issues/*.md
```

## Performance Issues

### Slow Startup

**Problem**: SwissArmyHammer takes long to start

**Solutions**:
```bash
# Profile startup time
time sah --version

# Disable file watching
sah config set general.auto_reload false

# Clear caches
rm -rf ~/.swissarmyhammer/cache/
rm -rf ~/.swissarmyhammer/models/

# Reduce search index
rm ~/.swissarmyhammer/search.db
# Re-index only important files
```

### Memory Usage Issues

**Problem**: High memory usage or out-of-memory errors

**Solutions**:
```bash
# Check memory limits
sah config show | grep security.max_memory_mb

# Reduce memory limits
sah config set security.max_memory_mb 256

# Limit file indexing
sah config set search.max_file_size 524288

# Reduce cache sizes
sah config set template.cache_size 100
sah config set workflow.cache_dir "/tmp/sah-cache"
```

### Disk Usage Issues

**Problem**: SwissArmyHammer using too much disk space

**Solutions**:
```bash
# Check disk usage
du -sh ~/.swissarmyhammer/*

# Clean up caches
rm -rf ~/.swissarmyhammer/cache/
rm -rf ~/.swissarmyhammer/workflow_cache/

# Reduce search index size
sah config set search.max_file_size 262144  # 256KB
rm ~/.swissarmyhammer/search.db
sah search index "**/*.{rs,py}" --exclude "**/target/**"

# Set disk usage limits
sah config set security.max_disk_usage_mb 512
```

## Network Issues

### Firewall/Proxy Issues

**Problem**: Network requests failing behind firewall/proxy

**Solutions**:
```bash
# Configure proxy
export HTTP_PROXY=http://proxy.example.com:8080
export HTTPS_PROXY=http://proxy.example.com:8080

# Disable network features if needed
sah config set security.allow_network false

# Use offline models
# Place models manually in ~/.swissarmyhammer/models/

# Test network connectivity
curl -I https://huggingface.co/nomic-ai/nomic-embed-text-v1.5
```

## Debugging

### Enable Debug Logging

```bash
# Temporary debug mode
SAH_LOG_LEVEL=debug sah command

# Persistent debug mode
sah config set logging.level debug
sah config set logging.file ~/.swissarmyhammer/debug.log

# Trace level for deep debugging
SAH_LOG_LEVEL=trace sah command 2>&1 | tee debug-output.log
```

### Collect Diagnostic Information

```bash
# Full system check
sah doctor --verbose --format json > diagnosis.json

# Configuration dump
sah config show --format json > config.json

# Environment information
env | grep SAH_ > environment.txt
sah --version > version.txt

# File system state
find ~/.swissarmyhammer -ls > filesystem.txt
```

### Common Log Messages

| Message | Meaning | Solution |
|---------|---------|----------|
| `Failed to load prompt` | Prompt file syntax error | Run `sah prompt validate` |
| `Template rendering failed` | Liquid template error | Check variable names and syntax |
| `MCP connection refused` | Claude Code not connecting | Check MCP configuration |
| `Git operation failed` | Git command error | Check git repository state |
| `Search index corrupted` | Database corruption | Delete and rebuild search index |
| `Permission denied` | File system permissions | Fix file/directory permissions |
| `Timeout exceeded` | Operation took too long | Increase timeout settings |

## Getting Help

If you encounter issues not covered here:

1. **Check logs**: Look at `~/.swissarmyhammer/logs/` for detailed error messages
2. **Run diagnostics**: Use `sah doctor --verbose` for comprehensive system check
3. **Search issues**: Check [GitHub Issues](https://github.com/swissarmyhammer/swissarmyhammer/issues) for similar problems
4. **Create issue**: Report new bugs with:
   - SwissArmyHammer version (`sah --version`)
   - Operating system and version
   - Complete error message
   - Steps to reproduce
   - Output of `sah doctor --verbose --format json`

### Emergency Recovery

If SwissArmyHammer is completely broken:

```bash
# Reset configuration to defaults
mv ~/.swissarmyhammer ~/.swissarmyhammer.backup
sah doctor --fix

# Reinstall from scratch
cargo uninstall swissarmyhammer-cli
cargo install swissarmyhammer-cli
sah doctor
```

This troubleshooting guide covers the most common issues and their solutions. Keep it handy for quick problem resolution.