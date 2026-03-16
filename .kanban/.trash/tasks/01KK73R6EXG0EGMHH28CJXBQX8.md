---
position_column: done
position_ordinal: q1
title: '[WARNING] LspServerSpec missing Debug impl'
---
File: swissarmyhammer-lsp/src/types.rs\n\nLspServerSpec is a public type but does not derive or implement Debug. The Rust review guidelines require: 'New public types must implement all applicable traits. Check for: Clone, Debug, Display, Default, PartialEq, Eq...'\n\nThe struct contains function pointers (initialization_options: Option<fn() -> Value>) which prevent a simple #[derive(Debug)], but a manual Debug impl should be provided. The function pointer also prevents Clone and PartialEq derives.\n\nAt minimum, Debug is needed for logging and diagnostics. Consider also implementing Display for human-readable output. #review-finding