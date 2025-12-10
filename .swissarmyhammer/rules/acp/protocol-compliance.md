---
severity: error
tags:
- acp
- protocol
---

# ACP Protocol Compliance

The ACP implementation must comply with the Agent Client Protocol specification.

## Requirements

- JSON-RPC 2.0 protocol over stdio
- Field names must use camelCase (not snake_case)
- Protocol version negotiation in initialize
- Proper error codes and messages
- All required methods implemented:
  - initialize
  - authenticate (if needed)
  - new_session
  - load_session
  - set_session_mode
  - prompt
  - cancel
- Session notifications use correct format
- Tool call flow follows ACP specification

## Verification

Run protocol compliance tests to verify all requirements are met.