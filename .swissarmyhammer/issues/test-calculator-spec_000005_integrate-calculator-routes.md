# Step 5: Integrate Calculator Routes into HTTP Server

Refer to .swissarmyhammer/tmp/test-calculator-spec.md

## Goal
Add calculator routes to the existing axum HTTP server configuration.

## Requirements
- Locate existing axum server setup in swissarmyhammer-tools
- Add `/add` route to router
- Ensure routes don't conflict with MCP endpoints
- Maintain existing server functionality

## Integration Points

Based on codebase exploration:
- HTTP server is in `swissarmyhammer-tools` 
- Look for existing axum Router configuration
- MCP server uses HTTP mode with `McpServerMode::Http`

## Router Configuration

```rust
// Add to existing router
let calculator_routes = Router::new()
    .route("/add", get(add_handler));

// Nest under /calculator prefix to avoid conflicts
app = app.nest("/calculator", calculator_routes);
```

## Mermaid Diagram

```mermaid
graph TD
    A[HTTP Server] --> B[MCP Routes]
    A --> C[Calculator Routes]
    C --> D[/calculator/add]
    D --> E[add_handler]
    E --> F[Calculator Service]
```

## Acceptance Criteria
- [ ] Calculator routes integrated into router
- [ ] Server compiles and runs
- [ ] MCP functionality still works
- [ ] Manual curl test succeeds: `curl "http://localhost:PORT/calculator/add?a=5&b=3"`
- [ ] No conflicts with existing routes
- [ ] Code formatted and no clippy warnings

## Dependencies
Requires: Step 4 (HTTP handlers implemented)

## Next Step
Add comprehensive integration tests for HTTP endpoints
