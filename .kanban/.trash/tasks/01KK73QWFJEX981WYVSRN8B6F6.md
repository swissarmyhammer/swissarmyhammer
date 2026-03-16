---
position_column: done
position_ordinal: p9
title: '[BLOCKER] CodeContextError flattens error chains into strings'
---
File: swissarmyhammer-code-context/src/error.rs\n\nAll three variants of CodeContextError wrap String instead of source errors. This violates the Rust review guideline: 'Error::source() chains must exist for wrapped errors -- don't flatten the chain.'\n\nThe Io variant should wrap std::io::Error, Database should wrap rusqlite::Error, and Election should wrap ElectionError. Currently callers lose the ability to match on or inspect the underlying error.\n\nExample fix:\n```rust\n#[error(\"io error\")]\nIo(#[from] std::io::Error),\n#[error(\"database error\")]\nDatabase(#[from] rusqlite::Error),\n#[error(\"election error\")]\nElection(#[from] ElectionError),\n``` #review-finding