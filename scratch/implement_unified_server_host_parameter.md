# Support configurable host in unified server

## Location
`swissarmyhammer-cli/src/commands/serve/mod.rs:72`

## Current State
```rust
// Note: unified server currently only supports 127.0.0.1, host parameter ignored for now
```

## Description
The unified server currently only listens on 127.0.0.1 (localhost) and ignores the host parameter. This should be fixed to allow binding to other addresses.

## Requirements
- Implement configurable host binding in unified server
- Support IPv4 and IPv6 addresses
- Add validation for host parameter
- Update tests to verify host binding
- Consider security implications (binding to 0.0.0.0)
- Update documentation

## Use Cases
- Running server on specific network interfaces
- Container deployments
- Remote access scenarios