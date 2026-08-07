// Package fixtures holds the Go fixtures of the `missing-docs-go` tool rule.
//
// This file holds one undocumented exported item of every kind the passing
// fixture documents. The tool must report each one. A tool upgrade that stops
// reporting a kind makes the doctor mark the rule unusable.
package fixtures

// DocumentedNeighbor is a documented exported function, so only the items
// below are reported.
func DocumentedNeighbor() {}

type UndocumentedType struct{}

func (t UndocumentedType) UndocumentedMethod() {}

func UndocumentedFunction() {}
