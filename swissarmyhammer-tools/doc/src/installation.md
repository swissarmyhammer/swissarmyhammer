# Installation

This guide provides detailed installation instructions for SwissArmyHammer Tools.

## Prerequisites

### System Requirements

- **Operating System**: macOS, Linux, or Windows
- **Rust**: Version 1.70 or later
- **Memory**: 2GB RAM minimum, 4GB recommended
- **Disk Space**: 500MB for installation and dependencies

### Installing Rust

If you don't have Rust installed:

**macOS and Linux:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Windows:**
Download and run [rustup-init.exe](https://rustup.rs/)

Verify installation:
```bash
rustc --version
cargo --version
```

## Installation Methods

### Method 1: Cargo Install (Recommended)

Install the latest released version:

```bash
cargo install swissarmyhammer
```

This installs the `sah` command-line tool globally.

Verify installation:
```bash
sah --version
```

### Method 2: From Source

Clone and build from source:

```bash
git clone https://github.com/swissarmyhammer/swissarmyhammer-tools.git
cd swissarmyhammer-tools
cargo build --release
```

The binary will be at `target/release/sah`.

Add to PATH or copy to system location:
```bash
# macOS/Linux
sudo cp target/release/sah /usr/local/bin/

# Or add to PATH
export PATH="$PATH:$(pwd)/target/release"
```

### Method 3: Pre-built Binaries

Download pre-built binaries from the [releases page](https://github.com/swissarmyhammer/swissarmyhammer-tools/releases).

**macOS:**
```bash
# Download and extract
tar xzf swissarmyhammer-macos.tar.gz
sudo mv sah /usr/local/bin/
```

**Linux:**
```bash
# Download and extract
tar xzf swissarmyhammer-linux.tar.gz
sudo mv sah /usr/local/bin/
```

**Windows:**
```powershell
# Extract zip and add to PATH
# Or move sah.exe to a directory in your PATH
```

## Platform-Specific Instructions

### macOS

#### Using Homebrew

```bash
brew tap swissarmyhammer/tap
brew install swissarmyhammer
```

#### Apple Silicon (M1/M2/M3)

Ensure you have Xcode command line tools:
```bash
xcode-select --install
```

Install using cargo (works on both Intel and Apple Silicon):
```bash
cargo install swissarmyhammer
```

### Linux

#### Ubuntu/Debian

Install build dependencies:
```bash
sudo apt-get update
sudo apt-get install build-essential pkg-config libssl-dev
```

Then install:
```bash
cargo install swissarmyhammer
```

#### Fedora/RHEL/CentOS

Install build dependencies:
```bash
sudo dnf install gcc openssl-devel
```

Then install:
```bash
cargo install swissarmyhammer
```

#### Arch Linux

Install from AUR:
```bash
yay -S swissarmyhammer
```

Or use cargo:
```bash
cargo install swissarmyhammer
```

### Windows

#### Prerequisites

Install Visual Studio Build Tools or Visual Studio with C++ workload.

Download from: https://visualstudio.microsoft.com/downloads/

#### Installation

Install via cargo:
```bash
cargo install swissarmyhammer
```

Or download pre-built binary from releases page.

## Updating

### Update via Cargo

```bash
cargo install swissarmyhammer --force
```

### Update from Source

```bash
cd swissarmyhammer-tools
git pull
cargo build --release
```

## Uninstalling

### Remove Cargo Installation

```bash
cargo uninstall swissarmyhammer
```

### Remove Source Build

Delete the binary:
```bash
# macOS/Linux
sudo rm /usr/local/bin/sah

# Or remove from custom location
rm ~/bin/sah
```

### Clean Project Data

Remove project-specific data:
```bash
rm -rf .swissarmyhammer/
```

**Warning:** This deletes issues, memos, and search index.

## Verifying Installation

### Check Version

```bash
sah --version
```

Expected output:
```
swissarmyhammer-tools 0.1.0
```

### List Commands

```bash
sah --help
```

### Test Server

```bash
sah serve --help
```

## Optional Dependencies

### mdBook (for documentation)

If you want to build documentation locally:

```bash
cargo install mdbook
```

Then build docs:
```bash
cd doc
mdbook build
mdbook serve
```

### Tree-sitter CLI (for development)

For working with tree-sitter grammars:

```bash
npm install -g tree-sitter-cli
```

## Troubleshooting Installation

### Cargo Install Hangs

**Issue:** Installation appears to hang during compilation.

**Solution:** This is normal for first install. Rust is compiling all dependencies. Wait 5-10 minutes.

### Linker Errors

**Issue:** Compilation fails with linker errors.

**Solution:**
- macOS: Install Xcode command line tools
- Linux: Install gcc and build-essential
- Windows: Install Visual Studio Build Tools

### SSL Errors

**Issue:** OpenSSL-related compilation errors.

**Solution:**
- macOS: `brew install openssl`
- Linux: Install libssl-dev or openssl-devel
- Windows: OpenSSL should be included with Rust

### Permission Denied

**Issue:** Can't write to installation directory.

**Solution:**
```bash
# Install to user directory instead
cargo install swissarmyhammer --root ~/.cargo
```

Or use `sudo` for system-wide install (not recommended).

## Post-Installation

### Shell Completion

Generate shell completion scripts:

**Bash:**
```bash
sah completion bash > ~/.local/share/bash-completion/completions/sah
```

**Zsh:**
```bash
sah completion zsh > ~/.zfunc/_sah
```

**Fish:**
```bash
sah completion fish > ~/.config/fish/completions/sah.fish
```

### Environment Variables

Optional environment variables:

```bash
# Enable debug logging
export RUST_LOG=debug

# Custom working directory
export SAH_CWD=/path/to/project

# Custom config location
export SAH_CONFIG=/path/to/config.json
```

## Next Steps

- [Quick Start](./quick-start.md): Get started with your first tasks
- [Configuration](./configuration.md): Configure SwissArmyHammer for your workflow
- [Features](./features.md): Explore available tools and capabilities
