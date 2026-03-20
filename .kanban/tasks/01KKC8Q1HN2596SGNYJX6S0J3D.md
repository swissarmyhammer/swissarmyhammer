---
assignees:
- assistant
depends_on:
- 01KKC8PX1S23FSYN708FREMC6E
position_column: done
position_ordinal: ff8c80
title: Add LspJsonRpcClient to LspDaemon after handshake
---
Add swissarmyhammer-code-context dep to swissarmyhammer-lsp. Store client in daemon, expose via client() method. Convert tokio ChildStdin/Stdout to std types after handshake.