# NEVER USE MOCKS

## Absolute Prohibition

**MOCKS ARE FORBIDDEN IN ALL SWISSARMYHAMMER CODE**

## What Constitutes a Mock

- Any test double that simulates behavior of real dependencies
- Mock objects, mock functions, mock services
- Stubbed implementations that return fake data
- Any code that pretends to be a real system for testing purposes

## What to Use Instead

### Real Dependencies
- Use real databases (in-memory or test instances)
- Use real file systems (with temporary directories)
- Use real HTTP clients (against test servers)
- Use real external services when possible

### Test Isolation Techniques
- Database transactions that rollback
- Temporary file systems
- Separate test environments
- Process isolation

### Integration Testing
- Test complete workflows end-to-end
- Use real data and real APIs
- Test against actual system boundaries
- Verify real system behavior

## Enforcement

- Code reviews must reject any mocking patterns
- CI/CD should fail on mock dependencies
- Search codebase regularly for mock patterns
- Zero tolerance for mock introductions

## Rationale

- Mocks test implementation, not behavior
- Mocks create false confidence
- Mocks become stale and misleading
- Real systems behave differently than mocks
- Integration issues are missed with mocks

## Vigilance Required

Watch for these anti-patterns:
- `mockall` crate usage
- `mock_` prefixed functions
- Test doubles masquerading as real objects
- Fake implementations for testing
- Stubbed network responses
