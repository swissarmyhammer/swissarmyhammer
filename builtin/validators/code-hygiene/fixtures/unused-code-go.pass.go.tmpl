// Package fixtures holds the Go fixtures of the `unused-code-go` tool rule.
//
// Every unexported item in this file is reached from the exported root below —
// one of every kind the failing fixture leaves dead. The tool must report
// nothing. A tool upgrade that reports one anyway makes the doctor mark the
// rule unusable.
package fixtures

// liveType is an unexported type the root names.
type liveType struct {
	liveCount int
}

// liveMethod is an unexported method the root calls.
func (t liveType) liveMethod() int { return t.liveCount }

// liveConst is an unexported constant the root reads.
const liveConst = 1

// liveVar is an unexported variable the root reads.
var liveVar = 2

// liveFunc is an unexported function the root calls.
func liveFunc() int { return liveConst + liveVar }

// DeadCodePassRoot is the exported root that keeps every unexported item above
// in use. An exported item is the package's surface, so the unused check counts
// it as used and never reports it.
func DeadCodePassRoot() int {
	return liveType{liveCount: 1}.liveMethod() + liveFunc()
}
