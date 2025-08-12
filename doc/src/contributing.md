# Contributing

Welcome to SwissArmyHammer! We appreciate your interest in contributing to this project. This guide will help you get started.

## Code of Conduct

SwissArmyHammer follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Please be respectful and inclusive in all interactions.

## Getting Started

### Development Environment

1. **Install Rust**: Ensure you have Rust 1.70 or later installed
2. **Clone the repository**:
   ```bash
   git clone https://github.com/swissarmyhammer/swissarmyhammer.git
   cd swissarmyhammer
   ```

3. **Install dependencies**:
   ```bash
   # Install development dependencies
   cargo install cargo-watch cargo-tarpaulin cargo-audit
   
   # Install pre-commit hooks (optional but recommended)
   pip install pre-commit
   pre-commit install
   ```

4. **Run tests** to verify setup:
   ```bash
   cargo test
   cargo clippy
   cargo fmt --check
   ```

### Project Structure

```
swissarmyhammer/
├── swissarmyhammer/          # Core library
├── swissarmyhammer-cli/      # Command-line interface
├── swissarmyhammer-tools/    # MCP tools and server
├── builtin/                  # Built-in prompts and workflows
├── doc/                      # Documentation (mdBook)
├── tests/                    # Integration tests
└── benches/                  # Benchmarks
```

## How to Contribute

### Reporting Issues

Before creating an issue, please:
1. Search existing issues to avoid duplicates
2. Use the issue templates when available
3. Provide detailed information including:
   - SwissArmyHammer version (`sah --version`)
   - Operating system and version
   - Steps to reproduce
   - Expected vs actual behavior
   - Relevant configuration files

### Proposing Features

For new features:
1. Open an issue with the "feature request" label
2. Describe the problem you're solving
3. Provide examples of how it would work
4. Consider implementation complexity
5. Wait for maintainer feedback before starting work

### Code Contributions

#### Pull Request Process

1. **Fork the repository** and create a feature branch
2. **Make your changes** following our coding standards
3. **Add tests** for new functionality
4. **Update documentation** if needed
5. **Run the full test suite**:
   ```bash
   # Run all tests
   cargo test --workspace
   
   # Run integration tests
   cargo test --test '*'
   
   # Check formatting and lints
   cargo fmt --check
   cargo clippy -- -D warnings
   
   # Run benchmarks (if performance-related)
   cargo bench
   ```

6. **Create a pull request** with:
   - Clear title and description
   - Link to related issues
   - Screenshots/examples if applicable
   - Checklist of completed items

#### Coding Standards

**Rust Code Style**:
- Use `cargo fmt` for formatting
- Pass `cargo clippy` with no warnings
- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Write comprehensive doc comments with examples
- Use meaningful variable and function names

**Error Handling**:
- Use the `anyhow` crate for error handling
- Provide contextual error messages
- Use the `Result` type consistently
- Don't panic in library code

**Testing**:
- Write unit tests for all public functions
- Add integration tests for complex workflows
- Use property-based testing where appropriate
- Maintain test coverage above 80%

**Documentation**:
- Write rustdoc comments for all public items
- Include usage examples in documentation
- Update the user guide for new features
- Keep CHANGELOG.md updated

#### Code Review Guidelines

**For Authors**:
- Keep PRs focused and reasonably sized
- Respond to feedback promptly
- Be open to suggestions and changes
- Test edge cases and error conditions

**For Reviewers**:
- Be constructive and specific in feedback
- Test the changes locally when possible
- Check for security implications
- Verify documentation is updated

### Documentation Contributions

Documentation improvements are always welcome:

- **User Guide**: Located in `doc/src/`
- **API Documentation**: Rust doc comments in source code
- **Examples**: Located in `doc/src/examples/`
- **README**: Project overview and quick start

When updating documentation:
1. Use clear, concise language
2. Provide practical examples
3. Test all code examples
4. Check for broken links
5. Follow the existing style and structure

## Development Workflows

### Running Tests

```bash
# Unit tests only
cargo test --lib

# Integration tests only  
cargo test --test '*'

# All tests with verbose output
cargo test --workspace --verbose

# Test with coverage
cargo tarpaulin --out html

# Test specific module
cargo test --package swissarmyhammer search::tests
```

### Development Server

For MCP development:
```bash
# Run MCP server in development mode
cargo run --bin swissarmyhammer-cli serve --stdio

# Or with debug logging
SAH_LOG_LEVEL=debug cargo run --bin swissarmyhammer-cli serve --stdio
```

### Benchmarking

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench search

# Generate benchmark reports
cargo bench -- --save-baseline main
```

### Debugging

```bash
# Run with debug logging
SAH_LOG_LEVEL=debug cargo run --bin swissarmyhammer-cli prompt list

# Use debugger
RUST_LOG=debug cargo run --bin swissarmyhammer-cli -- --help

# Memory debugging with valgrind
valgrind --tool=memcheck cargo run --bin swissarmyhammer-cli
```

## Contribution Areas

### High-Impact Areas

1. **Performance Optimizations**
   - Search indexing speed
   - Template rendering performance  
   - Memory usage reduction
   - Startup time optimization

2. **New Language Support**
   - Add TreeSitter parsers
   - Language-specific prompt templates
   - Build tool integrations

3. **MCP Tool Enhancements**
   - New tool implementations
   - Better error reporting
   - Request/response validation

4. **Documentation**
   - More examples and tutorials
   - Video guides
   - Translation to other languages

5. **Testing**
   - Edge case coverage
   - Performance regression tests
   - Cross-platform testing

### Good First Issues

Look for issues labeled `good-first-issue`:
- Documentation improvements
- Small bug fixes
- Adding new built-in prompts
- Test coverage improvements
- Error message enhancements

## Release Process

### Versioning

SwissArmyHammer uses [Semantic Versioning](https://semver.org/):
- **MAJOR**: Incompatible API changes
- **MINOR**: New functionality (backwards compatible)
- **PATCH**: Bug fixes (backwards compatible)

### Release Checklist

1. Update version numbers in `Cargo.toml` files
2. Update CHANGELOG.md with release notes
3. Run full test suite on multiple platforms
4. Update documentation if needed
5. Create release PR for review
6. Tag release after merge
7. Build and publish binaries
8. Update package registries (crates.io)
9. Announce release

## Community Guidelines

### Communication

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: General questions and ideas
- **Discord**: Real-time chat (if available)
- **Matrix**: Alternative chat platform (if available)

### Getting Help

If you need help:
1. Check the documentation first
2. Search existing issues
3. Ask in GitHub Discussions
4. Tag maintainers if urgent

### Recognition

Contributors are recognized:
- In CONTRIBUTORS.md file
- In release notes for significant contributions
- Through GitHub's contribution tracking
- In project documentation when appropriate

## Legal

### License

By contributing to SwissArmyHammer, you agree that your contributions will be licensed under the same license as the project (MIT or Apache-2.0).

### Copyright

- You retain copyright of your contributions
- You grant the project permission to use your contributions
- You confirm you have the right to make the contribution
- You agree your contribution does not violate any third-party rights

### Contributor License Agreement

Currently, no formal CLA is required, but this may change as the project grows. Contributors will be notified if a CLA becomes necessary.

## Resources

### Useful Links

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [mdBook Guide](https://rust-lang.github.io/mdBook/)
- [Model Context Protocol](https://github.com/anthropics/model-context-protocol)

### Tools and Services

- **CI/CD**: GitHub Actions
- **Code Coverage**: Codecov
- **Documentation**: GitHub Pages with mdBook
- **Package Registry**: crates.io
- **Binary Releases**: GitHub Releases

Thank you for contributing to SwissArmyHammer! Your efforts help make AI-powered development tools more accessible and powerful for everyone.