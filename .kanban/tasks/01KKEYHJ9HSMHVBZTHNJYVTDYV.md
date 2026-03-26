---
assignees:
- assistant
depends_on:
- 01KKEYHDFZYK1Z4STY2EPKFGMQ
position_column: done
position_ordinal: b880
title: Wire up all LSP clients in server.rs instead of just rust-analyzer
---
Replace hardcoded get_daemon('rust-analyzer') with iteration over all running daemons, collecting SharedLspClient for each. Spawn one LSP indexing worker per client."