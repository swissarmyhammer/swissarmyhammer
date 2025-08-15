# Add markdowndown Dependency

## Overview
Add the `markdowndown` crate dependency to the workspace for HTML fetching and markdown conversion functionality. Refer to /Users/wballard/github/sah-fetch/ideas/fetch.md.

## Tasks
- Add `markdowndown` crate to workspace dependencies in root `Cargo.toml`
- Research the markdowndown API and available configuration options
- Verify the dependency integrates correctly by running `cargo check`

## Implementation Details
- Add to `[workspace.dependencies]` section with appropriate version
- Use default features initially, can be refined later
- Ensure compatibility with existing reqwest and url dependencies

## Success Criteria
- `markdowndown` crate is available across workspace
- No dependency conflicts
- Basic API understanding documented in commit message

## Dependencies
- None - this is the first step

## Estimated Impact
- Small change to Cargo.toml
- Foundation for web fetch implementation
## Proposed Solution

Based on the existing workspace structure and the ideas/fetch.md specification, I will:

1. **Add markdowndown dependency to workspace**: Add `markdowndown = "0.1"` to the `[workspace.dependencies]` section in the root `Cargo.toml`
2. **Research markdowndown crate**: Examine the available version and API to understand integration points
3. **Verify dependency resolution**: Run `cargo check` to ensure no conflicts with existing dependencies like `reqwest` and `url`
4. **Document findings**: Add any important notes about the API structure for future implementation steps

This is the foundational step for the web fetch tool that will enable HTML fetching and markdown conversion functionality as outlined in the specification.

## Implementation Results

✅ **Successfully added markdowndown dependency to workspace**

### Changes Made
- Added `markdowndown = { git = "https://github.com/swissarmyhammer/markdowndown", version = "0.1.0" }` to `[workspace.dependencies]` in root `Cargo.toml`
- Used git dependency since the crate is not yet published to crates.io

### Dependency Research Findings
- **Repository**: https://github.com/swissarmyhammer/markdowndown
- **Version**: 0.1.0 (current main branch)
- **Status**: Active development, 87 commits, not yet published to crates.io
- **License**: MIT
- **Purpose**: Rust library for converting URLs to markdown with intelligent handling

### Key Dependencies (from markdowndown)
- `reqwest` - HTTP client (compatible with existing workspace dependency)
- `html2text` - HTML to text conversion
- `tokio` - Async runtime (compatible with existing workspace dependency)
- `serde` - Serialization (compatible with existing workspace dependency)

### Verification
- ✅ `cargo check` passed with no dependency conflicts
- ✅ All workspace members compile successfully
- ✅ No version conflicts with existing reqwest, tokio, serde dependencies

### API Overview (for future implementation)
The markdowndown crate appears to provide:
- URL fetching with smart document type detection
- HTML to markdown conversion
- Support for various document formats (HTML, Google Docs, Office 365, GitHub issues)
- Async API compatible with tokio
- Configurable conversion options
- Robust error handling

### Next Steps
The markdowndown dependency is now available across the workspace for the upcoming MCP tool implementation. The foundation is ready for building the web_fetch tool as specified in ideas/fetch.md.