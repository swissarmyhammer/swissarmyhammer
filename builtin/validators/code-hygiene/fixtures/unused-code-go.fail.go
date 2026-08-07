// Package fixtures holds the Go fixtures of the `unused-code-go` tool rule.
//
// This file holds one unused unexported item of every kind the passing fixture
// keeps in use — the type, the struct field, the method, the constant, the
// variable, and the function. The tool must report each one. A tool upgrade
// that stops reporting a kind makes the doctor mark the rule unusable.
package fixtures

// deadType is an unexported type no file of this package names.
type deadType struct{}

// deadHolder is named by the exported root below, so the tool reports its
// unused field alone and not the type around it.
type deadHolder struct {
	liveField int
	deadField int
}

// deadMethod is an unexported method no file of this package calls.
func (h deadHolder) deadMethod() int { return h.liveField }

// deadConst is an unexported constant no file of this package reads.
const deadConst = 1

// deadVar is an unexported variable no file of this package reads.
var deadVar = 2

// deadFunc is an unexported function no file of this package calls.
func deadFunc() int { return 3 }

// DeadCodeFailRoot keeps deadHolder and its live field in use. Without it the
// tool would report the type and both fields, and the fixture would no longer
// pin the struct-field kind on its own.
func DeadCodeFailRoot() int {
	return deadHolder{liveField: 1}.liveField
}
