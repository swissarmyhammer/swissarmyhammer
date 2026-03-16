---
position_column: done
position_ordinal: p8
title: '[BLOCKER] LspDaemon returns String errors instead of typed error enum'
---
File: swissarmyhammer-lsp/src/daemon.rs\n\nEvery fallible method on LspDaemon returns Result<(), String>. Per the Rust review guidelines: 'Libraries return typed error enums via thiserror. Never return String or Box dyn Error from public APIs.'\n\nThis is a library crate (not an application binary), so it should define an LspError enum (e.g., BinaryNotFound, SpawnFailed, HandshakeFailed, Timeout, ShutdownFailed) and use thiserror. The supervisor.rs module inherits this problem.\n\nThis also means callers cannot programmatically distinguish failure modes -- they would have to string-match, which is fragile. #review-finding