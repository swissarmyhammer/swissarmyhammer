#!/bin/bash
set -e

echo "=== Template Cache Validation ==="
echo

echo "1. Building project..."
cargo build --release
echo "✓ Build successful"
echo

echo "2. Running unit tests..."
cargo nextest run --failure-output immediate --hide-progress-bar --status-level fail --final-status-level fail template_cache
echo "✓ Unit tests passed"
echo

echo "3. Running integration tests..."
cargo nextest run --failure-output immediate --hide-progress-bar --status-level fail --final-status-level fail template_cache_integration
cargo nextest run --failure-output immediate --hide-progress-bar --status-level fail --final-status-level fail template_cache_e2e
echo "✓ Integration tests passed"
echo

echo "4. Running benchmarks..."
cargo bench --bench template_cache_bench -- --quick
echo "✓ Benchmarks completed"
echo

echo "5. Checking for clippy warnings..."
cargo clippy --all-targets -- -D warnings
echo "✓ No clippy warnings"
echo

echo "6. Running example..."
cargo run --example template_caching --release
echo "✓ Example ran successfully"
echo

echo "=== All Validations Passed ==="
