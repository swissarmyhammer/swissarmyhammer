// The failing fixture of the `missing-docs-dart` tool rule.
//
// It holds one undocumented public member. The tool must report it. A tool
// upgrade that stops reporting it makes the doctor mark the rule unusable.

/// A documented public function, so only the item below is reported.
void documentedNeighbor() {}

void undocumentedFunction() {}
