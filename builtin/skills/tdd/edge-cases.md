# Edge Cases and Coverage Checklist

**Load this reference when**: planning what to test for a specific behavior, or when reviewing test coverage before considering a feature complete.

## Edge Case Categories

For each behavior you test, consider these input categories. Not every category applies to every function — use judgment, but err toward testing more rather than fewer.

### Input Boundaries

- **Empty**: empty string, empty array/vec, empty map, zero-length slice
- **Null/None/missing**: null pointers, Option::None, undefined fields, missing keys
- **Single element**: one-item collection, single-character string
- **Boundary values**: 0, -1, 1, i32::MAX, i32::MIN, f64::INFINITY, f64::NAN
- **Off-by-one**: first element, last element, length-1, length+1

### Invalid Input

- **Wrong type**: string where number expected, object where array expected
- **Malformed**: invalid UTF-8, partial JSON, truncated input
- **Injection**: SQL injection strings, XSS payloads, path traversal (../)
- **Special characters**: Unicode, emoji, null bytes, newlines in single-line fields
- **Overflow**: values exceeding expected ranges, very long strings (10k+ chars)

### Error Conditions

- **Network failures**: timeout, connection refused, DNS failure, partial response
- **Resource exhaustion**: disk full, memory pressure, file descriptor limits
- **Permission errors**: read-only filesystem, unauthorized access, expired tokens
- **Concurrency**: race conditions, deadlocks, out-of-order events
- **Dependency failures**: database down, external API returning 500, malformed response

### State

- **Initial state**: first call, empty database, no prior configuration
- **Repeated operations**: calling same function twice, duplicate inserts, idempotency
- **State transitions**: valid→invalid, active→deleted, pending→complete
- **Stale state**: expired cache, changed-since-read, concurrent modification

## Coverage Targets

| Code Category | Minimum Coverage |
|--------------|-----------------|
| Financial calculations | 100% |
| Authentication/authorization | 100% |
| Security-critical paths | 100% |
| Core business logic | 100% |
| Public API surface | 90%+ |
| Internal utilities | 80%+ |
| Generated code | Exclude |
| Configuration | Exclude |

## Test Structure

Every test follows Arrange-Act-Assert:

1. **Arrange**: Set up the preconditions and inputs
2. **Act**: Execute the single behavior under test
3. **Assert**: Verify the result

One logical assertion per test. If a test name needs "and" in it, split it into two tests.

**Naming convention**: `should_[expected behavior]_when_[condition]` or your language's idiomatic equivalent (`test_[behavior]_[condition]` in Rust, `it('[behavior] when [condition]')` in JS).

## Test Quality Signals

**Good test**: Breaks when behavior changes. Survives internal refactoring. Reads like a specification. Uses real code paths.

**Bad test**: Breaks when implementation changes but behavior doesn't. Mocks internal collaborators. Tests private methods. Asserts on call counts or ordering of internal operations. Verifies state through back doors (querying DB directly instead of using the API).

**The refactor test**: Rename an internal function. If tests break but behavior hasn't changed, those tests are testing implementation, not behavior. Fix them.
