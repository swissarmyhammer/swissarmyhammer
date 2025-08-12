# Installation

This guide covers installing SwissArmyHammer on various platforms and configuring it for use with Claude Code.

## Prerequisites

- **Rust 1.70+** - Required for building from source
- **Claude Code** - For MCP integration (recommended)
- **Git** - For issue management features

## Installation Methods

### Option 1: Pre-built Binaries (Recommended)

Download the latest release for your platform from [GitHub Releases](https://github.com/swissarmyhammer/swissarmyhammer/releases).

#### Linux/macOS
```bash
# Download and install
curl -L https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/sah-linux-x64.tar.gz | tar xz
sudo mv sah /usr/local/bin/
```

#### Windows
```powershell
# Download sah-windows-x64.zip from releases page
# Extract to a directory in your PATH
```

### Option 2: Cargo Install
```bash
cargo install swissarmyhammer-cli
```

### Option 3: Build from Source
```bash
git clone https://github.com/swissarmyhammer/swissarmyhammer.git
cd swissarmyhammer
cargo build --release
sudo cp target/release/sah /usr/local/bin/
```

## Verification

Verify the installation:
```bash
sah --version
sah doctor
```

## Claude Code Integration

### Automatic Configuration
```bash
# Add SwissArmyHammer as an MCP server
claude mcp add sah sah serve

# Verify the connection
claude mcp list
```

### Manual Configuration

Add to your Claude Code MCP configuration (`~/.config/claude-code/mcp.json`):

```json
{
  "servers": {
    "sah": {
      "command": "sah",
      "args": ["serve"]
    }
  }
}
```

Restart Claude Code to load the MCP server.

## Directory Setup

SwissArmyHammer uses a three-tier directory structure:

### 1. Built-in (Automatic)
Pre-installed prompts and workflows are embedded in the binary.

### 2. User Directory
```bash
# Create your personal prompt directory
mkdir -p ~/.swissarmyhammer/prompts
mkdir -p ~/.swissarmyhammer/workflows
mkdir -p ~/.swissarmyhammer/memoranda
mkdir -p ~/.swissarmyhammer/issues
```

### 3. Local Directory (Per Project)
```bash
# In your project directory
mkdir -p .swissarmyhammer/prompts
mkdir -p .swissarmyhammer/workflows
mkdir -p .swissarmyhammer/memoranda
mkdir -p .swissarmyhammer/issues
```

## Shell Completions

Generate shell completions for your shell:

### Bash
```bash
sah completions bash > ~/.bash_completion.d/sah
source ~/.bash_completion.d/sah
```

### Zsh
```bash
sah completions zsh > ~/.zfunc/_sah
# Add ~/.zfunc to your fpath in ~/.zshrc
```

### Fish
```bash
sah completions fish > ~/.config/fish/completions/sah.fish
```

### PowerShell
```powershell
sah completions powershell | Out-File -Encoding utf8 $PROFILE
```

## Configuration File

Create an optional configuration file at `~/.swissarmyhammer/sah.toml`:

```toml
[general]
default_template_engine = "liquid"
auto_reload = true

[logging]
level = "info"
format = "compact"

[mcp]
enable_tools = ["issues", "memoranda", "search", "abort"]
timeout_ms = 30000

[search]
embedding_model = "nomic-embed-code"
index_path = "~/.swissarmyhammer/search.db"

[workflow]
max_parallel_actions = 4
default_timeout_ms = 300000
```

## Environment Variables

Set optional environment variables:

```bash
export SAH_HOME="$HOME/.swissarmyhammer"
export SAH_LOG_LEVEL="info"
export SAH_MCP_TIMEOUT="30000"
```

## Verification Steps

Run the doctor command to verify everything is configured correctly:

```bash
sah doctor
```

The doctor will check:
- ✅ Installation and binary location
- ✅ Directory structure
- ✅ Claude Code MCP integration
- ✅ File permissions
- ✅ Configuration validity
- ✅ Built-in prompts availability

## Troubleshooting

### Installation Issues

#### Problem: `sah: command not found`

**Cause**: SwissArmyHammer is not in your system PATH.

**Solutions**:
```bash
# Check if sah is installed
which sah
ls -la /usr/local/bin/sah

# If installed but not in PATH, add it
echo 'export PATH="/usr/local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# For custom installation location
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Verify PATH is updated
echo $PATH
```

#### Problem: Permission denied when running sah

**Cause**: Binary doesn't have execute permissions.

**Solutions**:
```bash
# Make binary executable
chmod +x $(which sah)

# If in custom location
chmod +x ~/.local/bin/sah

# Verify permissions
ls -la $(which sah)
```

#### Problem: Cargo install fails with compilation errors

**Cause**: Missing dependencies or outdated Rust version.

**Solutions**:
```bash
# Update Rust toolchain
rustup update stable
rustup default stable

# Install required dependencies (Ubuntu/Debian)
sudo apt-get update
sudo apt-get install build-essential pkg-config libssl-dev

# Install required dependencies (macOS)
xcode-select --install

# Clear cargo cache and retry
cargo clean
rm -rf ~/.cargo/registry/cache
cargo install swissarmyhammer-cli
```

#### Problem: Binary download fails or is corrupted

**Cause**: Network issues or incomplete download.

**Solutions**:
```bash
# Verify download integrity
sha256sum sah-linux-x64.tar.gz

# Re-download with resume support
wget -c https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/sah-linux-x64.tar.gz

# For macOS, use curl with resume
curl -C - -L -O https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/sah-macos-x64.tar.gz
```

### Directory and Permission Issues

#### Problem: Cannot create directories in ~/.swissarmyhammer

**Cause**: Insufficient permissions or disk space.

**Solutions**:
```bash
# Check disk space
df -h ~

# Check and fix permissions
ls -la ~ | grep swissarmyhammer
chmod 755 ~/.swissarmyhammer

# Create directories manually if needed
mkdir -p ~/.swissarmyhammer/{prompts,workflows,memoranda,issues}

# Fix ownership if needed
sudo chown -R $(whoami):$(id -gn) ~/.swissarmyhammer
```

#### Problem: Permission errors when running sah doctor

**Cause**: Incorrect file permissions or ownership.

**Solutions**:
```bash
# Fix SwissArmyHammer directory permissions
chmod -R 755 ~/.swissarmyhammer

# Fix configuration file permissions
chmod 600 ~/.swissarmyhammer/sah.toml

# Check current permissions
ls -la ~/.swissarmyhammer/
```

### Claude Code Integration Issues

#### Problem: MCP server not connecting

**Symptoms**:
- Claude Code doesn't show SwissArmyHammer tools
- Connection timeout errors
- MCP server status shows as offline

**Solutions**:
```bash
# Test MCP connection directly
sah serve --stdio
# Press Ctrl+C to exit after testing

# Check Claude Code MCP configuration
claude mcp list
claude mcp status sah

# Restart Claude Code MCP service
claude mcp restart sah

# Verify binary path in MCP config
which sah  # Should match path in mcp.json
```

#### Problem: MCP tools not available in Claude Code

**Cause**: MCP server not properly registered or tools not enabled.

**Solutions**:
1. **Check MCP Configuration**:
```bash
# Verify MCP config file exists
ls -la ~/.config/claude-code/mcp.json

# Check configuration syntax
cat ~/.config/claude-code/mcp.json | jq .
```

2. **Correct Configuration Example**:
```json
{
  "servers": {
    "sah": {
      "command": "sah",
      "args": ["serve"],
      "env": {
        "SAH_HOME": "/Users/yourname/.swissarmyhammer"
      }
    }
  }
}
```

3. **Restart Claude Code**:
```bash
# Kill all Claude Code processes
pkill -f "claude"

# Restart Claude Code
claude
```

#### Problem: MCP timeout errors

**Cause**: Operations taking too long or server overloaded.

**Solutions**:
```bash
# Increase timeout in configuration
cat > ~/.swissarmyhammer/sah.toml << EOF
[mcp]
timeout_ms = 60000  # Increase from default 30000
max_concurrent_requests = 2  # Reduce from default 4
EOF

# Check system resources
top -p $(pgrep sah)

# Restart MCP server
claude mcp restart sah
```

### Configuration Issues

#### Problem: Configuration file not loaded

**Symptoms**:
- Settings don't take effect
- Default values used instead of configured values

**Solutions**:
```bash
# Check configuration file location
ls -la ~/.swissarmyhammer/sah.toml

# Validate configuration syntax
sah config validate

# Test configuration loading
sah --debug doctor 2>&1 | grep "config"

# Use explicit config file
sah --config ~/.swissarmyhammer/sah.toml doctor
```

#### Problem: Invalid configuration values

**Cause**: Syntax errors or unsupported options.

**Solutions**:
```bash
# Validate configuration file
sah config validate ~/.swissarmyhammer/sah.toml

# Check for common issues
cat ~/.swissarmyhammer/sah.toml | grep -E "(timeout|path|level)"

# Reset to default configuration
mv ~/.swissarmyhammer/sah.toml ~/.swissarmyhammer/sah.toml.backup
sah doctor  # Will create default config
```

### Search and Index Issues

#### Problem: Semantic search not working

**Symptoms**:
- "Index not found" errors
- Search returns no results
- Model download fails

**Solutions**:
```bash
# Check index location and size
ls -la ~/.swissarmyhammer/search.db

# Rebuild search index
sah search index "**/*.rs" --force

# Check available disk space for models
df -h ~/.cache/  # or wherever models are stored

# Test basic search functionality
echo "fn main() { println!(\"hello\"); }" > test.rs
sah search index "test.rs"
sah search query "hello world"
rm test.rs
```

#### Problem: Model download fails

**Cause**: Network issues or insufficient disk space.

**Solutions**:
```bash
# Check internet connectivity
curl -I https://huggingface.co/

# Check available disk space
df -h ~/.cache/

# Clear model cache if corrupted
rm -rf ~/.cache/huggingface/
rm -rf ~/.swissarmyhammer/search.db

# Re-download models
sah search index "**/*.md" --force
```

### Performance Issues

#### Problem: Slow startup or operation

**Cause**: Large prompt collections or resource constraints.

**Solutions**:
```bash
# Profile startup time
time sah --help

# Check prompt directory sizes
du -sh ~/.swissarmyhammer/prompts/*
du -sh .swissarmyhammer/prompts/*

# Enable lazy loading in config
cat >> ~/.swissarmyhammer/sah.toml << EOF
[general]
lazy_loading = true
cache_prompts = true
max_cached_prompts = 100
EOF

# Monitor resource usage
top -p $(pgrep sah)
```

#### Problem: High memory usage

**Cause**: Large search indices or memory leaks.

**Solutions**:
```bash
# Check memory usage
ps aux | grep sah

# Reduce search index size
# Remove large unnecessary files from indexing
echo "*.log" >> .swissarmyhammerignore
echo "node_modules/" >> .swissarmyhammerignore

# Configure memory limits
cat >> ~/.swissarmyhammer/sah.toml << EOF
[search]
max_index_size_mb = 100
max_model_memory_mb = 500
EOF
```

### Platform-Specific Issues

#### macOS Issues

**Problem**: "sah cannot be opened because the developer cannot be verified"

**Solutions**:
```bash
# Allow unsigned binary (macOS Catalina and later)
xattr -d com.apple.quarantine $(which sah)

# Alternative: Allow in System Preferences
# System Preferences > Security & Privacy > General > "Allow anyway"
```

**Problem**: Homebrew installation issues

**Solutions**:
```bash
# Update Homebrew
brew update

# Install dependencies
brew install rust git

# Install via Homebrew (if tap available)
brew tap swissarmyhammer/tap
brew install swissarmyhammer
```

#### Windows Issues

**Problem**: PowerShell execution policy errors

**Solutions**:
```powershell
# Set execution policy for current user
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser

# Verify installation
Get-Command sah
sah --version
```

**Problem**: Path not updated in Windows

**Solutions**:
```powershell
# Add to PATH manually
$env:PATH += ";C:\path\to\sah"

# Add permanently to user PATH
[Environment]::SetEnvironmentVariable("PATH", $env:PATH + ";C:\path\to\sah", "User")

# Restart terminal and verify
Get-Command sah
```

#### Linux Distribution Specific

**Problem**: Missing dependencies on minimal distributions

**Solutions**:
```bash
# Alpine Linux
apk add --no-cache musl-dev gcc

# CentOS/RHEL
yum groupinstall "Development Tools"
yum install openssl-devel

# Arch Linux
pacman -S base-devel openssl
```

### Diagnostic Commands

When troubleshooting, use these diagnostic commands:

```bash
# Comprehensive system check
sah doctor --verbose

# Show all configuration sources
sah config show --all-sources

# Test MCP functionality
sah serve --test

# Check file permissions
sah doctor --check-permissions

# Validate all prompt files
sah validate --recursive ~/.swissarmyhammer/prompts

# Show version and build info
sah --version --verbose

# Enable debug logging for troubleshooting
SAH_LOG_LEVEL=debug sah doctor

# Test network connectivity for model downloads
sah search test-connection
```

### Getting Help

If you're still experiencing issues:

1. **Check the logs**:
```bash
# View recent logs
sah logs --tail 50

# Enable debug logging
SAH_LOG_LEVEL=debug sah doctor 2>&1 | tee debug.log
```

2. **Create a minimal reproduction case**:
```bash
# Create test environment
mkdir /tmp/sah-test
cd /tmp/sah-test
sah init --minimal
sah doctor
```

3. **Report the issue** with:
   - Output of `sah --version`
   - Output of `sah doctor --verbose`
   - Operating system and version
   - Steps to reproduce the problem
   - Any error messages or logs

### Recovery Procedures

#### Complete Reset

If everything is broken, reset to clean state:

```bash
# Backup current configuration
mv ~/.swissarmyhammer ~/.swissarmyhammer.backup.$(date +%Y%m%d)

# Remove binary and reinstall
sudo rm $(which sah)
# Follow installation instructions again

# Restore just the data you need
mkdir -p ~/.swissarmyhammer
cp -r ~/.swissarmyhammer.backup.*/prompts ~/.swissarmyhammer/
cp -r ~/.swissarmyhammer.backup.*/workflows ~/.swissarmyhammer/

# Initialize fresh configuration
sah doctor
```

#### Partial Reset

Reset just configuration while keeping data:

```bash
# Backup configuration
cp ~/.swissarmyhammer/sah.toml ~/.swissarmyhammer/sah.toml.backup

# Reset configuration
rm ~/.swissarmyhammer/sah.toml
sah doctor  # Creates fresh config

# Compare and merge settings if needed
diff ~/.swissarmyhammer/sah.toml.backup ~/.swissarmyhammer/sah.toml
```

## Next Steps

- [Quick Start](quick-start.md) - Create your first prompt
- [Configuration](configuration.md) - Customize your setup
- [CLI Reference](cli-reference.md) - Learn all available commands