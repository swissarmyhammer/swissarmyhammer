# Installation

SwissArmyHammer can be installed in several ways depending on your needs and platform.

## Pre-built Binaries

Currently, SwissArmyHammer does not provide pre-built binaries for download. This is a planned feature for future releases. For now, please use the Cargo installation method below.

## Quick Install (Recommended)

To install SwissArmyHammer:

```bash
# Install from git repository
cargo install --git https://github.com/wballard/swissarmyhammer.git swissarmyhammer-cli
```

To update SwissArmyHammer to the latest version:

```bash
# Update from git repository
cargo install --git https://github.com/wballard/swissarmyhammer.git swissarmyhammer-cli --force
```

The `--force` flag will overwrite the existing installation.

## Clone and Build

If you are installing from source:

- **Rust 1.70 or later** - Install from [rustup.rs](https://rustup.rs/)
- **Git** - For cloning the repository

If you want to build from source or contribute to development:

```bash
# Clone the repository
git clone https://github.com/wballard/swissarmyhammer.git
cd swissarmyhammer

# Build the CLI (debug mode for development)
cargo build

# Build optimized release version
cargo build --release

# Install from the local source
cargo install --path swissarmyhammer-cli

# Or run directly without installing
cargo run -- --help
```

## Verification

After installation, verify that SwissArmyHammer is working correctly:

```bash
# Check version
swissarmyhammer --version

# Run diagnostics
swissarmyhammer doctor

# Show help
swissarmyhammer --help

```

The `doctor` command will check your installation and provide helpful diagnostics if anything needs attention.

## Shell Completions

Generate and install shell completions for better CLI experience:

### Bash

```bash
# Linux/macOS (user-specific)
swissarmyhammer completion bash > ~/.local/share/bash-completion/completions/swissarmyhammer

# macOS with Homebrew bash-completion
swissarmyhammer completion bash > $(brew --prefix)/etc/bash_completion.d/swissarmyhammer

# Alternative location (ensure directory exists)
mkdir -p ~/.bash_completion.d
swissarmyhammer completion bash > ~/.bash_completion.d/swissarmyhammer
```

### Zsh

```bash
# User-specific (ensure ~/.zfunc is in your fpath)
mkdir -p ~/.zfunc
swissarmyhammer completion zsh > ~/.zfunc/_swissarmyhammer

# Add to ~/.zshrc if not already present:
# fpath=(~/.zfunc $fpath)
# autoload -U compinit && compinit

# System-wide (with appropriate permissions)
swissarmyhammer completion zsh > /usr/local/share/zsh/site-functions/_swissarmyhammer
```

### Fish

```bash
# User-specific
swissarmyhammer completion fish > ~/.config/fish/completions/swissarmyhammer.fish

# Ensure the directory exists
mkdir -p ~/.config/fish/completions
swissarmyhammer completion fish > ~/.config/fish/completions/swissarmyhammer.fish
```

## Next Steps

Once installed, you can start using SwissArmyHammer as an MCP server with Claude Code.

## Troubleshooting

### Common Issues

**Command not found**: Make sure `~/.cargo/bin` is in your PATH.

**Build failures**: Ensure you have Rust 1.70+ installed and try updating Rust:

```bash
rustup update
```

**Permission errors**: Don't use `sudo` with cargo install - it installs to your user directory.

For more help, run:

```bash
swissarmyhammer doctor
```