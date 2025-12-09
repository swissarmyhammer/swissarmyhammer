# Setup comprehensive linting configuration

## Description
Configure clippy and rustfmt with strict rules to maintain code quality across the project. Add custom lints for parser-specific patterns.

## Requirements
1. Create `.rustfmt.toml` configuration
2. Create `clippy.toml` with strict settings
3. Add pre-commit hooks for formatting
4. Configure parser-specific lints
5. Document coding standards

## Rustfmt Configuration
```toml
# .rustfmt.toml
edition = "2021"
use_field_init_shorthand = true
use_try_shorthand = true
imports_granularity = "Module"
group_imports = "StdExternalCrate"
format_code_in_doc_comments = true
```

## Clippy Configuration
```toml
# clippy.toml
cognitive-complexity-threshold = 20

# In Cargo.toml
[lints.clippy]
all = "warn"
pedantic = "warn"
nursery = "warn"
cargo = "warn"

# Allows
expect_used = "allow"
unwrap_used = "allow"  # in tests only
```

## Pre-commit Hook
```bash
#!/bin/sh
# .git/hooks/pre-commit
cargo fmt --check
cargo clippy -- -D warnings
cargo test --no-run
```

## Success Criteria
- Consistent code formatting
- No clippy warnings in codebase
- Pre-commit hooks prevent bad commits
- Coding standards documented
- CI enforces linting rules