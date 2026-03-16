---
position_column: done
position_ordinal: q8
title: '[NIT] context_dir() returns owned PathBuf instead of borrowing'
---
File: swissarmyhammer-code-context/src/workspace.rs, line 136\n\ncontext_dir() allocates a new PathBuf on every call. Per Rust API guidelines: 'as_ (free, borrow to borrow)'. Since this joins a constant suffix to an owned path, the allocation is cheap but unnecessary for repeated calls.\n\nConsider caching the context_dir in the struct or accepting the allocation. This is minor. #review-finding