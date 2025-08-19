# CLI Exclusion System Testing Makefile
.PHONY: help test test-unit test-integration test-property test-e2e test-performance test-coverage test-report clean-test

# Default target
help: ## Show this help message
	@echo "CLI Exclusion System Testing Commands:"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "Examples:"
	@echo "  make test           # Run all tests"
	@echo "  make test-coverage  # Run with coverage analysis" 
	@echo "  make test-report    # Generate comprehensive test report"

# Test execution targets
test: ## Run all CLI exclusion tests
	@echo "🧪 Running comprehensive CLI exclusion tests..."
	cargo test --test cli_exclusion_comprehensive_tests --no-capture

test-unit: ## Run unit tests only  
	@echo "🧪 Running CLI exclusion unit tests..."
	cargo test --test cli_exclusion_comprehensive_tests -- unit --no-capture

test-integration: ## Run integration tests only
	@echo "🧪 Running CLI exclusion integration tests..."
	cargo test --test cli_exclusion_comprehensive_tests -- integration --no-capture

test-property: ## Run property-based tests only
	@echo "🧪 Running CLI exclusion property tests..."
	PROPTEST_CASES=1000 cargo test --test cli_exclusion_comprehensive_tests -- property --no-capture

test-e2e: ## Run end-to-end tests only
	@echo "🧪 Running CLI exclusion end-to-end tests..."
	cargo test --test cli_exclusion_comprehensive_tests -- e2e --no-capture

test-performance: ## Run performance tests only
	@echo "🧪 Running CLI exclusion performance tests..."
	cargo test --test cli_exclusion_comprehensive_tests -- performance --no-capture --ignored

# Coverage and reporting targets
test-coverage: ## Run comprehensive test coverage analysis
	@echo "📊 Running comprehensive test coverage..."
	./scripts/test_coverage.sh

test-report: ## Generate comprehensive test report
	@echo "📋 Generating comprehensive test report..."
	./scripts/generate_test_report.sh

test-coverage-cli: ## Run CLI exclusion specific coverage only
	@echo "📊 Running CLI exclusion coverage analysis..."
	cargo install cargo-tarpaulin --force
	cargo tarpaulin --config cli_exclusion_coverage --skip-clean

# Development and maintenance targets
test-quick: ## Run quick smoke tests for development
	@echo "⚡ Running quick CLI exclusion smoke tests..."
	cargo test --test cli_exclusion_comprehensive_tests -- --test-threads=4 unit integration

test-watch: ## Watch for changes and re-run tests
	@echo "👀 Watching for changes..."
	cargo watch -x "test --test cli_exclusion_comprehensive_tests -- unit integration"

test-debug: ## Run tests with debug output
	@echo "🐛 Running tests with debug output..."
	RUST_LOG=debug cargo test --test cli_exclusion_comprehensive_tests -- --nocapture

# Cleanup targets  
clean-test: ## Clean test artifacts and reports
	@echo "🧹 Cleaning test artifacts..."
	rm -rf target/tarpaulin/
	rm -rf target/test_reports/
	rm -rf target/debug/deps/*cli_exclusion*
	rm -rf target/release/deps/*cli_exclusion*

# Validation targets
validate: test test-coverage ## Full validation pipeline (tests + coverage)
	@echo "✅ CLI exclusion system validation completed successfully"

validate-ci: ## CI-specific validation with stricter settings
	@echo "🏗️ Running CI validation pipeline..."
	PROPTEST_CASES=2000 cargo test --test cli_exclusion_comprehensive_tests --no-capture
	./scripts/test_coverage.sh
	./scripts/generate_test_report.sh

# Installation and setup targets
setup-tools: ## Install required testing tools
	@echo "🛠️ Installing testing tools..."
	cargo install cargo-tarpaulin --force
	cargo install cargo-watch --force
	cargo install cargo-nextest --force
	@echo "✅ Testing tools installed successfully"

check-setup: ## Verify testing environment setup
	@echo "🔍 Checking testing environment..."
	@cargo --version || (echo "❌ Cargo not found" && exit 1)
	@cargo-tarpaulin --version || (echo "❌ cargo-tarpaulin not found. Run 'make setup-tools'" && exit 1) 
	@python3 --version || (echo "⚠️ Python3 not found - needed for coverage parsing")
	@bc --version || (echo "⚠️ bc not found - needed for coverage validation")
	@echo "✅ Testing environment setup verified"

# Documentation targets
docs-tests: ## Generate test documentation
	@echo "📚 Generating test documentation..."
	cargo doc --document-private-items --no-deps --open