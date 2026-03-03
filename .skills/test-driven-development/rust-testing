# Rust Testing Patterns

**Load this reference when**: working in a Rust project and applying TDD. Covers test structure, tooling, and Rust-specific patterns.

## Test Runner

Use `cargo nextest` instead of `cargo test`. It runs tests in parallel per-test (not per-crate), is significantly faster, and gives better output on failures.

```bash
# Install
cargo install cargo-nextest

# Run all tests
cargo nextest run

# Run specific test
cargo nextest run test_name

# Run tests in a specific module
cargo nextest run --package my-crate -E 'test(module::name)'

# Run with nocapture for println debugging
cargo nextest run --nocapture

# Run only previously failed tests
cargo nextest run --run-ignored=ignored-only
```

Fall back to `cargo test` only for doc tests (`cargo nextest` does not run doc tests) and `--doc` flag targets.

## Test Structure

```rust
// In src/lib.rs or src/module.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_add_when_given_two_positive_numbers() {
        // Arrange
        let a = 2;
        let b = 3;

        // Act
        let result = add(a, b);

        // Assert
        assert_eq!(result, 5);
    }

    #[test]
    #[should_panic(expected = "division by zero")]
    fn should_panic_when_dividing_by_zero() {
        divide(10, 0);
    }

    #[test]
    fn should_return_err_when_input_invalid() {
        let result = parse_config("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }
}
```

### Test Naming

Use `should_[behavior]_when_[condition]`:

```rust
fn should_return_none_when_key_missing()
fn should_trim_whitespace_when_input_padded()
fn should_reject_when_token_expired()
```

## Integration Tests

Place in `tests/` directory at crate root. Each file is compiled as a separate crate — only accesses the public API.

```rust
// tests/integration_test.rs
use my_crate::MyService;

#[test]
fn should_process_end_to_end() {
    let service = MyService::new();
    let result = service.process("input");
    assert_eq!(result.status(), Status::Complete);
}
```

Use a `tests/common/mod.rs` for shared test utilities (not `tests/common.rs` which cargo treats as a test file).

## Test Utilities

### Builder Pattern for Test Data

```rust
#[cfg(test)]
struct UserBuilder {
    name: String,
    email: String,
    active: bool,
}

#[cfg(test)]
impl UserBuilder {
    fn new() -> Self {
        Self {
            name: "Test User".into(),
            email: "test@example.com".into(),
            active: true,
        }
    }

    fn name(mut self, name: &str) -> Self {
        self.name = name.into();
        self
    }

    fn inactive(mut self) -> Self {
        self.active = false;
        self
    }

    fn build(self) -> User {
        User { name: self.name, email: self.email, active: self.active }
    }
}
```

### Temporary Files

```rust
use tempfile::TempDir;

#[test]
fn should_write_output_to_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("output.txt");

    write_output(&path, "content").unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "content");
    // dir is cleaned up on drop
}
```

## Async Tests

```rust
#[tokio::test]
async fn should_fetch_when_url_valid() {
    let result = fetch_data("https://example.com").await;
    assert!(result.is_ok());
}

#[tokio::test]
#[should_panic]
async fn should_timeout_when_server_unresponsive() {
    let result = fetch_with_timeout(Duration::from_millis(1)).await.unwrap();
}
```

## Property-Based Testing

Use `proptest` for property-based testing. Define properties that must hold for all inputs rather than testing specific examples.

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn should_roundtrip_serialize_deserialize(input in ".*") {
        let encoded = encode(&input);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn should_never_exceed_capacity(size in 0..1000usize) {
        let buf = Buffer::with_capacity(size);
        assert!(buf.len() <= buf.capacity());
    }
}
```

## Mocking

Prefer **trait-based dependency injection** over mock frameworks. Define a trait for the boundary, implement it for production, implement it for test:

```rust
trait TimeProvider {
    fn now(&self) -> DateTime<Utc>;
}

struct RealTime;
impl TimeProvider for RealTime {
    fn now(&self) -> DateTime<Utc> { Utc::now() }
}

#[cfg(test)]
struct FakeTime(DateTime<Utc>);
#[cfg(test)]
impl TimeProvider for FakeTime {
    fn now(&self) -> DateTime<Utc> { self.0 }
}
```

Only mock at **system boundaries**: clocks, network, filesystem, external services. If you need `mockall` or similar for internal types, your design likely needs refactoring.

## Coverage

```bash
# Using cargo-llvm-cov
cargo install cargo-llvm-cov

cargo llvm-cov nextest           # Coverage with nextest runner
cargo llvm-cov --html            # Generate HTML report
cargo llvm-cov --fail-under 80   # Fail if below threshold
```

## Workspace Tips

In a cargo workspace, run tests for a specific crate to keep feedback loops tight:

```bash
cargo nextest run -p my-crate                             # One crate
cargo nextest run -p my-crate -E 'test(specific_test)'    # One test
cargo nextest run --workspace                             # All crates
```

## Snapshot Testing

Use `insta` for snapshot testing. Useful for serialized output, error messages, and presentation layers:

```rust
use insta::assert_snapshot;

#[test]
fn should_format_error_message() {
    let err = validate_input("").unwrap_err();
    assert_snapshot!(err.to_string());
}

// Run `cargo insta review` to accept/reject snapshots
```
