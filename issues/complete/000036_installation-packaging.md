# Installation and Package Distribution

## Problem
The doctor command shows that swissarmyhammer is not in PATH and suggests manual path configuration. For a tool meant to be widely adopted, installation should be seamless through standard package managers.

## Current State
- Binary must be manually added to PATH
- No standard package manager distributions
- Users must provide full path to Claude Code configuration

## Requirements from Specification
- "Zero configuration required to start"
- Wide distribution for adoption
- Following patterns of successful Rust CLIs

## Missing Package Manager Support
- [ ] **Homebrew** (macOS/Linux) - Most important for developer tools
- [ ] **Cargo** (`cargo install swissarmyhammer`) - Should work already but needs publishing
- [ ] **apt** (Debian/Ubuntu) 
- [ ] **pacman** (Arch Linux)
- [ ] **scoop** (Windows)
- [ ] **winget** (Windows)
- [ ] **snap** (Universal Linux)

## Installation Methods Needed
- [ ] Pre-built binaries for GitHub releases
- [ ] Installation script (curl | sh pattern)
- [ ] Container/Docker images
- [ ] Package manager repositories

## Implementation Tasks
- [ ] Set up automated binary releases in GitHub Actions
- [ ] Create Homebrew formula
- [ ] Publish to crates.io
- [ ] Create installation documentation
- [ ] Add installation verification to doctor command

## Success Criteria
- [ ] `brew install swissarmyhammer` works on macOS/Linux
- [ ] `cargo install swissarmyhammer` works from crates.io
- [ ] Binary is automatically in PATH after installation
- [ ] Doctor command shows green check for installation
- [ ] Installation takes < 30 seconds on typical systems