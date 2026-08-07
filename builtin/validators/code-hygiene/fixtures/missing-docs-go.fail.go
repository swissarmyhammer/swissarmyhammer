// Package fixtures holds the Go fixtures of the `missing-docs-go` tool rule.
//
// This file holds one undocumented exported item. The tool must report it. A
// tool upgrade that stops reporting it makes the doctor mark the rule
// unusable.
package fixtures

// DocumentedNeighbor is a documented exported function, so only the item below
// is reported.
func DocumentedNeighbor() {}

func UndocumentedFunction() {}
