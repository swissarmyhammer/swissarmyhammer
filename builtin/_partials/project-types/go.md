---
title: Go Project Guidelines
description: Best practices and tooling for Go projects
partial: true
---

### Go Project Guidelines

**Common Commands:**
- Build: `go build` or `go build ./...` (all packages)
- **Run ALL tests:** `go test ./...` (tests all packages recursively)
- **Run tests with coverage:** `go test -cover ./...`
- **Run specific package tests:** `go test ./path/to/package`
- **Run specific test:** `go test -run TestName ./path/to/package`
- Run: `go run .` or `go run main.go`
- Format: `go fmt ./...` (auto-format code)
- Lint: `golangci-lint run` (if installed)
- Tidy dependencies: `go mod tidy`
- Download dependencies: `go mod download`

**IMPORTANT:** Do NOT glob for test files. Use `go test ./...` to run all tests - Go discovers `*_test.go` files automatically.

**Best Practices:**
- Always run `go fmt` before committing (Go standard)
- Use `go mod tidy` to clean up dependencies
- Run `go vet` to catch common mistakes
- Test coverage: `go test -cover ./...`
- Consider using `golangci-lint` for comprehensive linting

**Module Management:**
- Dependencies defined in `go.mod`
- Use `go get` to add dependencies
- Use `go mod tidy` to remove unused dependencies
- Vendor dependencies: `go mod vendor` (optional)

**File Locations:**
- Main package: Root or `cmd/` directory
- Source code: Root or organized by package
- Tests: Same directory as source (e.g., `file_test.go`)
- Build output: Binary with module name (git-ignored)
- Vendor: `vendor/` (git-ignored unless vendoring)
