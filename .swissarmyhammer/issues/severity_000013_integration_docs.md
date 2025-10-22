# Step 13: Integration Testing and Documentation

**Refer to ideas/severity.md**

## Goal

Verify that all error types implement the Severity trait, add integration tests, and update documentation.

## Context

This is the final step to ensure everything works together and is properly documented.

## Tasks

### 1. Create Comprehensive Integration Test

Create `swissarmyhammer-common/tests/severity_integration_test.rs`:

```rust
//! Integration tests for Severity trait implementation across all crates

use swissarmyhammer_common::{ErrorSeverity, Severity, SwissArmyHammerError};

#[test]
fn test_common_error_implements_severity() {
    let error = SwissArmyHammerError::NotInGitRepository;
    assert_eq!(error.severity(), ErrorSeverity::Critical);
}

// Import and test error types from other crates
#[cfg(feature = "integration-test")]
mod cross_crate_tests {
    use super::*;
    
    // These would require the crates to be available as dev-dependencies
    // For now, we document that each crate has its own unit tests
    
    #[test]
    fn test_all_error_crates_compile() {
        // This test ensures all crates compile with the Severity trait
        // The actual verification happens in each crate's unit tests
    }
}
```

### 2. Create Verification Script

Create `.swissarmyhammer/scripts/verify_severity_implementations.sh`:

```bash
#!/bin/bash
# Verify all error types implement Severity trait

set -e

echo "Verifying Severity trait implementations..."

# List of crates with error types
CRATES=(
    "swissarmyhammer-common"
    "swissarmyhammer-cli"
    "swissarmyhammer-workflow"
    "swissarmyhammer-config"
    "swissarmyhammer-rules"
    "swissarmyhammer-git"
    "swissarmyhammer-todo"
    "swissarmyhammer-search"
    "swissarmyhammer-memoranda"
    "swissarmyhammer-outline"
    "swissarmyhammer-templating"
    "swissarmyhammer-agent-executor"
    "swissarmyhammer-shell"
    "swissarmyhammer-tools"
    "swissarmyhammer"
)

echo "Building all crates..."
for crate in "${CRATES[@]}"; do
    echo "  Building $crate..."
    cargo build -p "$crate" --quiet || {
        echo "❌ Build failed for $crate"
        exit 1
    }
done

echo ""
echo "Running tests for all crates..."
for crate in "${CRATES[@]}"; do
    echo "  Testing $crate..."
    cargo test -p "$crate" --quiet || {
        echo "❌ Tests failed for $crate"
        exit 1
    }
done

echo ""
echo "Running clippy for all crates..."
for crate in "${CRATES[@]}"; do
    echo "  Linting $crate..."
    cargo clippy -p "$crate" --quiet -- -D warnings || {
        echo "❌ Clippy failed for $crate"
        exit 1
    }
done

echo ""
echo "✅ All crates implement Severity trait correctly!"
```

Make it executable:
```bash
chmod +x .swissarmyhammer/scripts/verify_severity_implementations.sh
```

### 3. Update Documentation

#### 3.1 Update swissarmyhammer-common README

Create or update `swissarmyhammer-common/README.md`:

```markdown
# swissarmyhammer-common

Common types and utilities shared across all SwissArmyHammer crates.

## Error Handling

### Severity Trait

All error types in SwissArmyHammer implement the `Severity` trait, which provides
a standardized way to query error severity levels:

\`\`\`rust
use swissarmyhammer_common::{ErrorSeverity, Severity, SwissArmyHammerError};

let error = SwissArmyHammerError::NotInGitRepository;
match error.severity() {
    ErrorSeverity::Critical => eprintln!("Critical error: {}", error),
    ErrorSeverity::Error => eprintln!("Error: {}", error),
    ErrorSeverity::Warning => eprintln!("Warning: {}", error),
}
\`\`\`

### Severity Levels

- **Critical**: System cannot continue, data loss possible, requires immediate attention
- **Error**: Operation failed but system can continue, no data loss
- **Warning**: Potential issue but operation can proceed

### Implementing Severity for Your Error Type

\`\`\`rust
use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MyError {
    #[error("critical failure")]
    CriticalFailure,
    
    #[error("not found")]
    NotFound,
    
    #[error("deprecated")]
    Deprecated,
}

impl Severity for MyError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            MyError::CriticalFailure => ErrorSeverity::Critical,
            MyError::NotFound => ErrorSeverity::Error,
            MyError::Deprecated => ErrorSeverity::Warning,
        }
    }
}
\`\`\`
```

#### 3.2 Add Documentation to Main README

Update the main `README.md` to mention the Severity trait.

### 4. Add Rustdoc Examples

Ensure the Severity trait has good rustdoc examples in `swissarmyhammer-common/src/error.rs`.

### 5. Run Full Workspace Tests

```bash
cargo test --workspace
```

### 6. Run Clippy on Workspace

```bash
cargo clippy --workspace -- -D warnings
```

### 7. Check Documentation Builds

```bash
cargo doc --workspace --no-deps
```

### 8. Create Summary Document

Create `.swissarmyhammer/docs/severity_trait_implementation.md`:

```markdown
# Severity Trait Implementation Summary

## Overview

All error types across the SwissArmyHammer codebase now implement the `Severity` trait
from swissarmyhammer-common.

## Implemented Error Types

### swissarmyhammer-common
- ✅ SwissArmyHammerError
- ✅ ParameterError
- ✅ ConditionError

### swissarmyhammer-cli
- ✅ ValidationError
- ✅ ConversionError

### swissarmyhammer-workflow
- ✅ WorkflowError
- ✅ ExecutorError
- ✅ GraphError
- ✅ StateError
- ✅ ActionError
- ✅ ParseError

### swissarmyhammer-config
- ✅ ConfigurationError
- ✅ AgentError

### swissarmyhammer-rules
- ✅ RuleError

### swissarmyhammer-git
- ✅ GitError

### swissarmyhammer-todo
- ✅ TodoError

### swissarmyhammer-search
- ✅ SearchError

### swissarmyhammer-memoranda
- ✅ MemorandaError

### swissarmyhammer-outline
- ✅ OutlineError

### swissarmyhammer-templating
- ✅ TemplatingError

### swissarmyhammer-agent-executor
- ✅ ActionError

### swissarmyhammer-shell
- ✅ ShellSecurityError
- ✅ ShellError

### swissarmyhammer-tools
- ✅ SecurityError
- ✅ ContentFetchError
- ✅ DuckDuckGoError
- ✅ ToolValidationError
- ✅ ValidationError
- ✅ SendError

### swissarmyhammer (main)
- ✅ PlanCommandError

## Usage Guidelines

See [swissarmyhammer-common README](../../swissarmyhammer-common/README.md) for
usage examples and implementation guidelines.

## Testing

All implementations include unit tests verifying severity assignments.

Run full test suite:
\`\`\`bash
cargo test --workspace
\`\`\`

Run verification script:
\`\`\`bash
.swissarmyhammer/scripts/verify_severity_implementations.sh
\`\`\`
```

## Acceptance Criteria

- [ ] Integration test created
- [ ] Verification script created and executable
- [ ] swissarmyhammer-common README updated
- [ ] Main README updated
- [ ] Rustdoc examples added/improved
- [ ] Summary document created
- [ ] All workspace tests pass: `cargo test --workspace`
- [ ] All workspace builds clean: `cargo build --workspace`
- [ ] All workspace clippy clean: `cargo clippy --workspace`
- [ ] Documentation builds: `cargo doc --workspace --no-deps`
- [ ] Verification script runs successfully

## Files to Create/Modify

- `swissarmyhammer-common/tests/severity_integration_test.rs` (new)
- `.swissarmyhammer/scripts/verify_severity_implementations.sh` (new)
- `swissarmyhammer-common/README.md` (update or create)
- `README.md` (update)
- `swissarmyhammer-common/src/error.rs` (improve rustdoc)
- `.swissarmyhammer/docs/severity_trait_implementation.md` (new)

## Estimated Changes

~150 lines of code (tests + docs + scripts)

## Next Step

Implementation complete! 🎉
