---
position_column: done
position_ordinal: q2
title: '[WARNING] LspSupervisorManager and LspDaemon missing Debug impls'
---
File: swissarmyhammer-lsp/src/supervisor.rs, swissarmyhammer-lsp/src/daemon.rs\n\nBoth LspSupervisorManager and LspDaemon are public types without Debug implementations. Child and watch::Sender may not be Debug, but manual impls showing the command name and current state would be valuable for diagnostics.\n\nPer Rust review guidelines: 'Debug implemented for all public types with non-empty representation.' #review-finding