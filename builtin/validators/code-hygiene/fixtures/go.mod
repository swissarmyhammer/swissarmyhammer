// The Go fixtures of the `unused-code-go` tool rule, as a module.
//
// `staticcheck -checks U1000 ./...` needs a module to load, because the unused
// check reads a whole package: an unexported item is dead only when no file of
// the package names it. The Go fixtures of every code-hygiene tool rule are
// files of the one `fixtures` package this module holds, so the tool sees the
// fixtures and nothing else.
//
// The `go` directive names an old release on purpose. It is the lowest version
// the fixtures need, so the toolchain already installed always satisfies it and
// never downloads another one.
module code-hygiene-fixtures

go 1.21
