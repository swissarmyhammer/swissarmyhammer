// The passing fixture of the `missing-docs-swift` tool rule.
//
// Every public declaration here has documentation. The tool must report
// nothing. A tool upgrade that reports one anyway makes the doctor mark the
// rule unusable.

/// A documented public structure.
public struct DocumentedStructure {
    /// A documented public method.
    public func documentedMethod() {}
}

/// A documented public function.
public func documentedFunction() {}
