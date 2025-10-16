# Environment Variables

SwissArmyHammer Tools respects a number of environment variables for configuration and runtime behavior. This document provides a complete reference for all supported environment variables.

## Storage Location Variables

### SWISSARMYHAMMER_MEMOS_DIR

Controls the directory where memos are stored.

- **Type**: Path (absolute or relative)
- **Default**: `.swissarmyhammer/memos` (in git root if detected)
- **Example**:
  ```bash
  export SWISSARMYHAMMER_MEMOS_DIR=/custom/path/to/memos
  ```

**Usage**:
```bash
# Use custom memo directory
export SWISSARMYHAMMER_MEMOS_DIR=~/my-notes
sah serve
```

**Priority**: Overrides configuration file `memos.directory` setting.

### SWISSARMYHAMMER_ISSUES_DIR

Controls the directory where issues are stored.

- **Type**: Path (absolute or relative)
- **Default**: `.swissarmyhammer/issues` (in git root if detected)
- **Example**:
  ```bash
  export SWISSARMYHAMMER_ISSUES_DIR=/custom/path/to/issues
  ```

**Usage**:
```bash
# Use custom issue directory
export SWISSARMYHAMMER_ISSUES_DIR=~/my-issues
sah serve
```

**Priority**: Overrides configuration file `issues.directory` setting.

## Logging Variables

### RUST_LOG

Controls the logging level and filtering for Rust components.

- **Type**: String (log level or filter expression)
- **Default**: `info`
- **Valid levels**: `error`, `warn`, `info`, `debug`, `trace`
- **Example**:
  ```bash
  export RUST_LOG=debug
  export RUST_LOG=swissarmyhammer_tools=debug
  export RUST_LOG=swissarmyhammer_tools::mcp=trace
  ```

**Usage**:

```bash
# Enable debug logging for everything
RUST_LOG=debug sah serve

# Enable trace logging for specific module
RUST_LOG=swissarmyhammer_tools::mcp::tools=trace sah serve

# Multiple filters
RUST_LOG=warn,swissarmyhammer_tools=debug sah serve
```

**Filter Syntax**:
- `level` - Set global level
- `crate=level` - Set level for specific crate
- `crate::module=level` - Set level for specific module
- `level,filter1,filter2` - Multiple filters (comma-separated)

**Priority**: Overrides configuration file `logging.level` setting.

### RUST_LOG_STYLE

Controls the styling of log output.

- **Type**: String
- **Default**: `auto`
- **Valid values**: `auto`, `always`, `never`
- **Example**:
  ```bash
  export RUST_LOG_STYLE=always
  ```

**Usage**:
```bash
# Force colored output even when piped
RUST_LOG_STYLE=always sah serve | less -R

# Disable colors for log files
RUST_LOG_STYLE=never sah serve > server.log
```

### RUST_BACKTRACE

Controls whether backtraces are displayed on panics.

- **Type**: Boolean-like string
- **Default**: `0` (disabled)
- **Valid values**: `0`, `1`, `full`
- **Example**:
  ```bash
  export RUST_BACKTRACE=1
  export RUST_BACKTRACE=full
  ```

**Usage**:
```bash
# Enable backtraces for debugging
RUST_BACKTRACE=1 sah serve

# Full backtraces with all frames
RUST_BACKTRACE=full sah serve
```

## Mode Variables

### SAH_CLI_MODE

Enables CLI mode features and behaviors.

- **Type**: Boolean (`0` or `1`)
- **Default**: `0` (disabled)
- **Example**:
  ```bash
  export SAH_CLI_MODE=1
  ```

**Usage**:
```bash
# Enable CLI mode
SAH_CLI_MODE=1 sah serve
```

**Effects when enabled**:
- Changes default storage paths
- Enables CLI-specific output formatting
- Adjusts certain behavior for CLI usage

## API and Authentication Variables

### ANTHROPIC_API_KEY

API key for Anthropic's Claude models (required for agent execution).

- **Type**: String (API key)
- **Default**: None (must be provided)
- **Required**: For workflow execution and agent features
- **Example**:
  ```bash
  export ANTHROPIC_API_KEY=sk-ant-...
  ```

**Usage**:
```bash
# Set API key for agent execution
export ANTHROPIC_API_KEY=sk-ant-api03-...
sah serve
```

**Security**:
- Never commit API keys to version control
- Use environment variable or secure secret management
- Restrict shell history: `export HISTCONTROL=ignorespace` and prefix with space

## Development Variables

### SWISSARMYHAMMER_DEV

Enables development mode features.

- **Type**: Boolean (`0` or `1`)
- **Default**: `0` (disabled)
- **Example**:
  ```bash
  export SWISSARMYHAMMER_DEV=1
  ```

**Usage**:
```bash
# Enable development mode
SWISSARMYHAMMER_DEV=1 sah serve
```

**Effects when enabled**:
- More verbose error messages
- Additional debug logging
- Development-specific behaviors

### CARGO_MANIFEST_DIR

Automatically set by Cargo when running from source.

- **Type**: Path
- **Set by**: Cargo
- **Description**: Path to the manifest directory (for locating bundled resources)

**Usage**: Typically not set manually, used internally.

## Runtime Variables

### NO_COLOR

Disables colored output (follows the [NO_COLOR standard](https://no-color.org/)).

- **Type**: Any value (presence indicates true)
- **Default**: Not set
- **Example**:
  ```bash
  export NO_COLOR=1
  ```

**Usage**:
```bash
# Disable all color output
NO_COLOR=1 sah serve
```

### TERM

Terminal type, affects output formatting.

- **Type**: String
- **Default**: Set by shell/terminal
- **Common values**: `xterm-256color`, `screen`, `dumb`

**Usage**: Automatically set by your terminal, but can be overridden:
```bash
# Force dumb terminal (no colors or special formatting)
TERM=dumb sah serve
```

## System Variables

### PATH

System PATH for locating executables.

- **Type**: Colon-separated paths (Unix) or semicolon-separated (Windows)
- **Description**: SwissArmyHammer needs to find git, cargo, and other tools

**Usage**:
```bash
# Ensure cargo bin is in PATH
export PATH="$HOME/.cargo/bin:$PATH"
sah serve
```

### HOME

User's home directory.

- **Type**: Path
- **Description**: Used for locating global configuration and storage

**Usage**: Automatically set by the system, used to locate:
- `~/.config/swissarmyhammer/sah.yaml`
- `~/.swissarmyhammer/prompts/`
- `~/.swissarmyhammer/workflows/`

## Configuration Priority

Environment variables override configuration file settings:

1. Default values (hardcoded)
2. Global config file (`~/.config/swissarmyhammer/sah.yaml`)
3. Project-local config file (`./sah.yaml`)
4. **Environment variables** ← Takes precedence
5. Command-line arguments (highest priority)

## Setting Environment Variables

### In Current Shell Session

```bash
# Bash/Zsh
export VAR_NAME=value

# Fish
set -x VAR_NAME value

# Windows PowerShell
$env:VAR_NAME = "value"

# Windows CMD
set VAR_NAME=value
```

### Persistent Configuration

#### Unix/Linux/macOS

Add to shell profile (`~/.bashrc`, `~/.zshrc`, etc.):

```bash
# ~/.bashrc or ~/.zshrc
export SWISSARMYHAMMER_MEMOS_DIR=~/my-notes
export RUST_LOG=info
export ANTHROPIC_API_KEY=sk-ant-...
```

Then reload:
```bash
source ~/.bashrc  # or ~/.zshrc
```

#### Windows

Use System Properties → Environment Variables, or:

```powershell
# PowerShell (persistent for current user)
[System.Environment]::SetEnvironmentVariable(
    'SWISSARMYHAMMER_MEMOS_DIR',
    'C:\Users\Name\notes',
    'User'
)
```

### Per-Command

Set for single command execution:

```bash
# Unix/Linux/macOS
RUST_LOG=debug sah serve

# Windows PowerShell
$env:RUST_LOG="debug"; sah serve

# Windows CMD
set RUST_LOG=debug && sah serve
```

### Using .env Files

Create `.env` file in project root:

```bash
# .env
SWISSARMYHAMMER_MEMOS_DIR=./.swissarmyhammer/memos
RUST_LOG=info
```

Load with shell tools:

```bash
# Using export
export $(cat .env | xargs)

# Using a tool like direnv
# (automatically loads .env when entering directory)
```

**Security**: Add `.env` to `.gitignore` if it contains secrets!

## Environment Variable Patterns

### Development Setup

```bash
#!/bin/bash
# dev-env.sh

export RUST_LOG=debug
export RUST_BACKTRACE=1
export SWISSARMYHAMMER_DEV=1
export SWISSARMYHAMMER_MEMOS_DIR=/tmp/dev-memos
export SWISSARMYHAMMER_ISSUES_DIR=/tmp/dev-issues

# Source this file: source dev-env.sh
```

### Production Setup

```bash
#!/bin/bash
# prod-env.sh

export RUST_LOG=warn
export SWISSARMYHAMMER_MEMOS_DIR=/var/lib/swissarmyhammer/memos
export SWISSARMYHAMMER_ISSUES_DIR=/var/lib/swissarmyhammer/issues
export NO_COLOR=1

# For secrets, use a secrets manager, not environment files!
```

### Testing Setup

```bash
#!/bin/bash
# test-env.sh

export RUST_LOG=debug
export RUST_BACKTRACE=full
export SWISSARMYHAMMER_MEMOS_DIR=/tmp/test-memos-$$
export SWISSARMYHAMMER_ISSUES_DIR=/tmp/test-issues-$$

# Cleanup on exit
trap "rm -rf /tmp/test-*-$$" EXIT
```

## Checking Environment Variables

### View Current Value

```bash
# Unix/Linux/macOS
echo $SWISSARMYHAMMER_MEMOS_DIR
printenv SWISSARMYHAMMER_MEMOS_DIR

# Windows PowerShell
echo $env:SWISSARMYHAMMER_MEMOS_DIR

# Windows CMD
echo %SWISSARMYHAMMER_MEMOS_DIR%
```

### List All SwissArmyHammer Variables

```bash
# Unix/Linux/macOS
printenv | grep SWISSARMYHAMMER
env | grep RUST_LOG

# Windows PowerShell
Get-ChildItem Env: | Where-Object Name -like "*SWISSARMYHAMMER*"
```

## Troubleshooting

### Environment Variables Not Taking Effect

1. **Check spelling**: Variable names are case-sensitive on Unix/Linux
2. **Verify export**: Ensure using `export` in bash/zsh
3. **Check scope**: Variable set in current shell session?
4. **Restart application**: Changes require application restart
5. **Check priority**: Command-line args override environment variables

### Secrets in Environment Variables

**Problem**: Accidentally exposed secrets in shell history or logs.

**Solutions**:

1. **Use secret management tools**:
   ```bash
   # Use a password manager or secrets vault
   export ANTHROPIC_API_KEY=$(pass show anthropic/api-key)
   ```

2. **Prevent history recording**:
   ```bash
   # Bash/Zsh - prefix with space (requires HISTCONTROL=ignorespace)
   export HISTCONTROL=ignorespace
    export ANTHROPIC_API_KEY=sk-ant-...  # Note leading space
   ```

3. **Clear from history**:
   ```bash
   # Remove from current session history
   history -d <line-number>

   # Clear entire history
   history -c
   ```

4. **Use config files with restricted permissions**:
   ```bash
   # Store in file with restricted permissions
   echo "ANTHROPIC_API_KEY=sk-ant-..." > ~/.anthropic
   chmod 600 ~/.anthropic
   source ~/.anthropic
   ```

## Best Practices

1. **Use environment variables for**:
   - Secrets and API keys
   - Environment-specific paths
   - Temporary overrides
   - Development settings

2. **Don't use environment variables for**:
   - Permanent project configuration (use `sah.yaml`)
   - Shared team settings (use project-local `sah.yaml`)
   - Complex nested structures (use YAML)

3. **Security**:
   - Never commit secrets to version control
   - Use restricted file permissions for secret files
   - Consider using a secrets manager
   - Audit environment variables regularly

4. **Documentation**:
   - Document required environment variables in README
   - Provide example `.env.example` file
   - Include setup scripts for common scenarios

## Related Documentation

- [Configuration Reference](./configuration.md)
- [Getting Started](../getting-started.md)
- [Troubleshooting](../troubleshooting.md)
