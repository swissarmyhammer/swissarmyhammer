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