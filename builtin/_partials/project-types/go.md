---
title: Go Project Guidelines
description: Best practices and tooling for Go projects
partial: true
---

### Go Project Guidelines

**Testing — do NOT glob; Go discovers `*_test.go` automatically:**
- All: `go test ./...`
- With coverage: `go test -cover ./...`
- Package: `go test ./path/to/pkg`
- Single: `go test -run TestName ./path/to/pkg`

**Common commands:**
- Build: `go build ./...`
- Run: `go run .`
- Format: `go fmt ./...` (or `goimports -w .` to also fix imports) — Go enforces this; always run before committing
- Vet: `go vet ./...`
- Lint: `golangci-lint run` (if installed)
- Deps: `go mod tidy`, `go mod download`, `go get <pkg>`

**File locations:**
- Main: root or `cmd/`
- Tests: alongside source as `*_test.go`
- Vendor: `vendor/` (git-ignored unless vendoring)
- Module config: `go.mod`
