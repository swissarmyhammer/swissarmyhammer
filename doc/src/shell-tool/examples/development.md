# Development Workflow Examples

This guide provides practical examples of using the shell tool for common development tasks, from building and testing to version control and debugging.

## Rust Development

### Building Rust Projects

**Basic build commands**:
```bash
# Debug build
sah shell -C /project "cargo build"

# Release build with timeout
sah shell -t 900 -C /project "cargo build --release"

# Build with custom environment
sah shell -C /project -e "RUSTFLAGS='-C target-cpu=native'" "cargo build --release"

# Clean and rebuild
sah shell -C /project "cargo clean && cargo build --release"
```

**Advanced build scenarios**:
```bash
# Cross-compilation
sah shell -C /project -e "CARGO_TARGET_DIR=/tmp/target" "cargo build --target x86_64-unknown-linux-musl"

# Build with verbose output for debugging
sah shell -t 1200 -C /project -e "RUST_LOG=debug" "cargo build --verbose"

# Parallel build with job control
sah shell -t 1800 -C /project "cargo build --release --jobs $(nproc)"
```

### Testing Rust Code

**Running tests**:
```bash
# All tests
sah shell -C /project "cargo test"

# Tests with timeout and detailed output
sah shell -t 600 -C /project -e "RUST_LOG=debug" -e "RUST_BACKTRACE=1" "cargo test"

# Specific test module
sah shell -C /project "cargo test integration_tests"

# Tests with coverage
sah shell -t 900 -C /project "cargo test --coverage"
```

**Test scenarios**:
```bash
# Integration tests only
sah shell -C /project "cargo test --test integration"

# Unit tests only
sah shell -C /project "cargo test --lib"

# Benchmark tests
sah shell -t 1800 -C /project "cargo bench"

# Documentation tests
sah shell -C /project "cargo test --doc"
```

### Rust Development Workflow

**Complete development cycle**:
```bash
# 1. Update dependencies
sah shell -C /project "cargo update"

# 2. Check for issues
sah shell -C /project "cargo check"

# 3. Run clippy for lints
sah shell -C /project "cargo clippy -- -D warnings"

# 4. Format code
sah shell -C /project "cargo fmt -- --check"

# 5. Run tests
sah shell -t 600 -C /project "cargo test"

# 6. Build release
sah shell -t 900 -C /project "cargo build --release"
```

## JavaScript/Node.js Development

### Package Management

**NPM operations**:
```bash
# Install dependencies
sah shell -t 600 -C /project "npm install"

# Clean install for CI
sah shell -t 900 -C /project -e "CI=true" "npm ci"

# Update packages
sah shell -C /project "npm update"

# Check for vulnerabilities
sah shell -t 300 -C /project "npm audit"

# Fix vulnerabilities
sah shell -t 600 -C /project "npm audit fix"
```

**Yarn operations**:
```bash
# Install with Yarn
sah shell -t 600 -C /project "yarn install --frozen-lockfile"

# Add development dependency
sah shell -C /project "yarn add --dev typescript"

# Check outdated packages
sah shell -C /project "yarn outdated"
```

### Building and Testing

**Build operations**:
```bash
# Development build
sah shell -C /project -e "NODE_ENV=development" "npm run build:dev"

# Production build
sah shell -t 1200 -C /project -e "NODE_ENV=production" "npm run build"

# Build with source maps
sah shell -C /project -e "GENERATE_SOURCEMAP=true" "npm run build"

# TypeScript compilation
sah shell -C /project "npx tsc --noEmit"
```

**Testing scenarios**:
```bash
# Run all tests
sah shell -t 600 -C /project -e "NODE_ENV=test" "npm test"

# Watch mode for development
sah shell -t 1800 -C /project "npm run test:watch"

# Coverage report
sah shell -t 900 -C /project "npm run test:coverage"

# End-to-end tests
sah shell -t 1800 -C /project "npm run test:e2e"
```

### Development Workflow

**Complete Node.js workflow**:
```bash
# 1. Install dependencies
sah shell -t 600 -C /project "npm ci"

# 2. Lint code
sah shell -C /project "npm run lint"

# 3. Type check
sah shell -C /project "npx tsc --noEmit"

# 4. Run tests
sah shell -t 600 -C /project "npm test"

# 5. Build for production
sah shell -t 900 -C /project -e "NODE_ENV=production" "npm run build"

# 6. Security audit
sah shell -C /project "npm audit --audit-level=moderate"
```

## Python Development

### Virtual Environment Management

**Setup and activation**:
```bash
# Create virtual environment
sah shell -C /project "python -m venv .venv"

# Install dependencies with virtual environment
sah shell -C /project -e "PATH=/project/.venv/bin:$PATH" "pip install -r requirements.txt"

# Install development dependencies
sah shell -C /project -e "PATH=/project/.venv/bin:$PATH" "pip install -r requirements-dev.txt"
```

### Testing Python Code

**Running tests with pytest**:
```bash
# Basic test run
sah shell -C /project -e "PYTHONPATH=/project" "python -m pytest"

# Tests with coverage
sah shell -t 600 -C /project -e "PYTHONPATH=/project" "python -m pytest --cov=src --cov-report=html"

# Specific test file
sah shell -C /project -e "PYTHONPATH=/project" "python -m pytest tests/test_module.py"

# Verbose output with debugging
sah shell -C /project -e "PYTHONPATH=/project" -e "PYTEST_CURRENT_TEST=1" "python -m pytest -v -s"
```

**Quality checks**:
```bash
# Code formatting with black
sah shell -C /project "black --check ."

# Linting with flake8
sah shell -C /project "flake8 src/ tests/"

# Type checking with mypy
sah shell -C /project "mypy src/"

# Security analysis
sah shell -C /project "bandit -r src/"
```

## Git Operations

### Common Git Commands

**Repository status and information**:
```bash
# Check repository status
sah shell -C /project "git status --porcelain"

# View recent commits
sah shell -C /project "git log --oneline -n 10"

# Check for uncommitted changes
sah shell --quiet -C /project "git diff --exit-code"

# Check current branch
sah shell -C /project "git branch --show-current"
```

**Branch operations**:
```bash
# Create and switch to new branch
sah shell -C /project "git checkout -b feature/new-feature"

# Switch branches
sah shell -C /project "git checkout main"

# Merge branch
sah shell -C /project "git merge feature/completed-feature"

# Delete merged branch
sah shell -C /project "git branch -d feature/completed-feature"
```

### Git Workflow Integration

**Pre-commit checks**:
```bash
# Check if there are uncommitted changes
sah shell -C /project "git status --porcelain | wc -l"

# Stage all changes
sah shell -C /project "git add ."

# Commit with message
sah shell -C /project "git commit -m 'feat: add new functionality'"

# Push to remote
sah shell -C /project "git push origin $(git branch --show-current)"
```

**Advanced Git operations**:
```bash
# Interactive rebase (be careful with timeouts)
sah shell -t 1800 -C /project "git rebase -i HEAD~3"

# Squash last 3 commits
sah shell -C /project "git reset --soft HEAD~3 && git commit -m 'Combined commit'"

# Cherry-pick specific commit
sah shell -C /project "git cherry-pick abc123def"
```

## Build System Integration

### Make-based Projects

**Traditional Makefiles**:
```bash
# Clean build
sah shell -C /project "make clean"

# Parallel build
sah shell -t 1800 -C /project "make -j$(nproc)"

# Debug build
sah shell -C /project -e "DEBUG=1" "make"

# Install to system
sah shell -C /project "make install PREFIX=/usr/local"
```

### CMake Projects

**CMake build workflow**:
```bash
# Create build directory
sah shell -C /project "mkdir -p build"

# Configure project
sah shell -C /project/build "cmake .."

# Build with CMake
sah shell -t 1200 -C /project/build "cmake --build . --parallel $(nproc)"

# Run tests
sah shell -C /project/build "ctest --verbose"

# Install
sah shell -C /project/build "cmake --install . --prefix /usr/local"
```

## Database Operations

### Database Migrations

**Running database migrations**:
```bash
# Django migrations
sah shell -C /project -e "DATABASE_URL=postgresql://user:pass@localhost/db" "python manage.py migrate"

# Rails migrations
sah shell -C /project -e "RAILS_ENV=development" "bundle exec rails db:migrate"

# Node.js with Sequelize
sah shell -C /project -e "NODE_ENV=development" "npx sequelize-cli db:migrate"
```

### Database Utilities

**Database maintenance**:
```bash
# Create database backup
sah shell -t 1800 "pg_dump -h localhost -U user dbname > backup_$(date +%Y%m%d).sql"

# Restore database
sah shell -t 1800 "psql -h localhost -U user dbname < backup_20240815.sql"

# Check database size
sah shell "psql -h localhost -U user -d dbname -c 'SELECT pg_size_pretty(pg_database_size(current_database()));'"
```

## Development Environment Setup

### Project Initialization

**Setting up new projects**:
```bash
# Initialize Rust project
sah shell -C /workspace "cargo new my-project --bin"

# Initialize Node.js project
sah shell -C /workspace "npm init -y && npm install express"

# Initialize Python project with structure
sah shell -C /workspace "mkdir my-project && cd my-project && python -m venv .venv"
```

### Development Server Management

**Starting development servers**:
```bash
# Node.js development server
sah shell -C /project -e "NODE_ENV=development" -e "PORT=3000" "npm run dev"

# Python Flask development server
sah shell -C /project -e "FLASK_ENV=development" -e "FLASK_APP=app.py" "flask run"

# Ruby on Rails server
sah shell -C /project -e "RAILS_ENV=development" "bundle exec rails server"

# Hugo static site generator
sah shell -C /project "hugo server --buildDrafts"
```

## Code Quality and Analysis

### Linting and Formatting

**Multi-language formatting**:
```bash
# Format Rust code
sah shell -C /project "cargo fmt"

# Format JavaScript/TypeScript
sah shell -C /project "npx prettier --write ."

# Format Python code
sah shell -C /project "black . && isort ."

# Format Go code
sah shell -C /project "gofmt -w ."
```

### Static Analysis

**Code analysis tools**:
```bash
# Rust Clippy
sah shell -C /project "cargo clippy -- -D warnings"

# ESLint for JavaScript
sah shell -C /project "npx eslint src/ --fix"

# Python static analysis
sah shell -C /project "mypy src/ && pylint src/"

# Security scanning
sah shell -t 600 -C /project "cargo audit"
sah shell -C /project "npm audit"
```

## Performance Testing

### Benchmarking

**Performance measurements**:
```bash
# Rust benchmarks
sah shell -t 1800 -C /project "cargo bench"

# Node.js performance testing
sah shell -t 900 -C /project "npm run benchmark"

# Load testing with wrk
sah shell -t 300 "wrk -t12 -c400 -d30s http://localhost:3000/"

# Memory usage testing
sah shell -C /project "valgrind --tool=memcheck ./target/release/my-app"
```

## Best Practices for Development

### Command Organization

**Organize development tasks**:
```bash
# Create development script
cat > dev-workflow.sh << 'EOF'
#!/bin/bash
set -e

echo "Starting development workflow..."

# Format code
sah shell -C /project "cargo fmt"

# Run lints
sah shell -C /project "cargo clippy -- -D warnings"

# Run tests
sah shell -t 600 -C /project "cargo test"

# Build release
sah shell -t 900 -C /project "cargo build --release"

echo "Development workflow completed successfully!"
EOF

chmod +x dev-workflow.sh
./dev-workflow.sh
```

### Error Handling

**Robust development workflows**:
```bash
# Check if build succeeds before running tests
if sah shell --quiet -C /project "cargo check"; then
    echo "Build check passed, running tests..."
    sah shell -t 600 -C /project "cargo test"
else
    echo "Build check failed, skipping tests"
    exit 1
fi

# Conditional operations
sah shell -C /project "test -f Cargo.toml && cargo build || npm install"
```

### Development Automation

**Automated development workflows**:
```bash
#!/bin/bash
# complete-dev-cycle.sh

PROJECT_DIR="/path/to/project"

# Function to run shell commands with error checking
run_command() {
    echo "Running: $1"
    if ! sah shell -C "$PROJECT_DIR" "$1"; then
        echo "Command failed: $1"
        exit 1
    fi
}

# Development cycle
run_command "git pull origin main"
run_command "cargo update"
run_command "cargo fmt"
run_command "cargo clippy -- -D warnings"
run_command "cargo test"
run_command "cargo build --release"
run_command "git add ."
run_command "git commit -m 'chore: automated development cycle'"
run_command "git push origin main"

echo "Development cycle completed successfully!"
```

This development workflow documentation provides comprehensive examples for common development scenarios. Adapt the commands and timeouts based on your specific project requirements and system performance.