// The failing fixture of the `missing-docs-swift` tool rule.
//
// It holds one undocumented public declaration. The tool must report it. A
// tool upgrade that stops reporting it makes the doctor mark the rule
// unusable.

/// A documented public function, so only the item below is reported.
public func documentedNeighbor() {}

public func undocumentedFunction() {}
