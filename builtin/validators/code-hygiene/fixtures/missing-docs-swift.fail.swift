// The failing fixture of the `missing-docs-swift` tool rule.
//
// It holds one undocumented public declaration of every kind the passing
// fixture documents. The tool must report each one. A tool upgrade that stops
// reporting a kind makes the doctor mark the rule unusable.

/// A documented public function, so only the declarations below are reported.
public func documentedNeighbor() {}

public struct UndocumentedStructure {
    public func undocumentedMethod() {}
}

public func undocumentedFunction() {}
