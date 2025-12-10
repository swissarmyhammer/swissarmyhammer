---
severity: error
tags:
- acp
- security
---

# ACP Client Capability Enforcement

All ACP operations must check client capabilities before execution.

## Requirements

- Check `client.fs.read_text_file` capability before file reads
- Check `client.fs.write_text_file` capability before file writes
- Check `client.terminal` capability before terminal operations
- Store client capabilities from initialize request
- Return appropriate errors when capabilities are missing
- Never assume capabilities are available

## Verification

Test that operations fail gracefully when client capabilities are not advertised.