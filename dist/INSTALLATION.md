# Installation Guide

SwissArmyHammer is available through multiple installation methods. Choose the one that works best for your system.

## Quick Install (Recommended)

### Cargo Install from Git

```bash
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer.git swissarmyhammer-cli
```

This will:
- Install the latest version directly from the Git repository
- Place the `sah` binary in `~/.cargo/bin` (make sure it's in your PATH)
- Work on all platforms supported by Rust

## Install from Source

### Prerequisites

- Rust 1.70 or later
- Git

### Build and Install

```bash
# Clone the repository
git clone https://github.com/wballard/swissarmyhammer.git
cd swissarmyhammer

# Build the release binary
cargo build --release

# Install to ~/.cargo/bin (make sure it's in your PATH)
cargo install --path .

# Or copy the binary manually
cp target/release/swissarmyhammer /usr/local/bin/
```

### Using Cargo

```bash
# Install from the git repository (recommended)
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer.git swissarmyhammer-cli
```

## Verification

After installation, verify that SwissArmyHammer is working correctly:

```bash
# Check version
swissarmyhammer --version

# Run diagnostics
swissarmyhammer doctor

# Test basic functionality
swissarmyhammer --help
```

## Configuration

### Claude Code Integration

Add SwissArmyHammer to your Claude Code MCP configuration:

```json
{
  "mcpServers": {
    "swissarmyhammer": {
      "command": "swissarmyhammer",
      "args": ["serve"]
    }
  }
}
```

### Shell Completions

Generate and install shell completions for better CLI experience:

```bash
# Bash
swissarmyhammer completion bash > ~/.local/share/bash-completion/completions/swissarmyhammer

# Zsh (add to fpath)
swissarmyhammer completion zsh > ~/.zfunc/_swissarmyhammer

# Fish
swissarmyhammer completion fish > ~/.config/fish/completions/swissarmyhammer.fish

# PowerShell
swissarmyhammer completion powershell >> $PROFILE
```

## Updating

### Manual Update

Re-run the installation method you used initially. For the install script:

```bash
curl -fsSL https://raw.githubusercontent.com/wballard/swissarmyhammer/main/dist/install.sh | bash
```

### Homebrew

```bash
brew update && brew upgrade swissarmyhammer
```

### Cargo

```bash
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer.git swissarmyhammer-cli --force
```

## Troubleshooting

### Common Issues

1. **Command not found**
   - Make sure the binary is in your PATH
   - Try the full path: `~/.local/bin/swissarmyhammer`

2. **Permission denied**
   - Make sure the binary is executable: `chmod +x swissarmyhammer`
   - Check file permissions and ownership

3. **Binary won't run on older systems**
   - Try the musl variant for Linux: `swissarmyhammer-x86_64-unknown-linux-musl`
   - Check your system's minimum requirements

4. **Installation script fails**
   - Make sure you have `curl` or `wget` installed
   - Check your internet connection
   - Try downloading manually

### Getting Help

If you encounter issues:

1. Run `swissarmyhammer doctor` for diagnostics
2. Check the [GitHub Issues](https://github.com/wballard/swissarmyhammer/issues)
3. Create a new issue with:
   - Your operating system and version
   - Installation method used
   - Error messages
   - Output of `swissarmyhammer doctor`

## Uninstalling

### Remove Binary

```bash
# If installed to /usr/local/bin
sudo rm /usr/local/bin/swissarmyhammer

# If installed to ~/.local/bin
rm ~/.local/bin/swissarmyhammer

# If installed via Homebrew
brew uninstall swissarmyhammer

# If installed via Cargo
cargo uninstall swissarmyhammer
```

### Remove Configuration

```bash
# Remove user configuration and prompts
rm -rf ~/.swissarmyhammer

# Remove shell completions
rm ~/.local/share/bash-completion/completions/swissarmyhammer  # Bash
rm ~/.zfunc/_swissarmyhammer  # Zsh
rm ~/.config/fish/completions/swissarmyhammer.fish  # Fish
```

### Remove from Claude Code

Remove the `swissarmyhammer` entry from your Claude Code MCP configuration.