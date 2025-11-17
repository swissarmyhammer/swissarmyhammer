# Rule Violation: code-quality/code-duplication

**File**: swissarmyhammer-common/src/directory.rs
**Severity**: ERROR

## Violation Message

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-common/src/directory.rs
Line: 100
Severity: warning
Message: The three constructor methods `from_git_root()`, `from_user_home()`, and `from_custom_root()` contain near-identical code blocks for directory creation. Each method has the same pattern: compute root path, check if exists, create if not, return struct.
Suggestion: Extract the common directory creation and struct initialization logic into a private helper method. Create a method like `fn new(root: PathBuf, root_type: DirectoryRootType) -> Result<Self>` that handles the existence check, directory creation, and struct construction. Then each constructor would only need to compute the root path and call this helper.

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-common/src/directory.rs
Line: 103-105
Severity: warning
Message: The code block for checking directory existence and creating it if needed is duplicated across three methods (`from_git_root()`, `from_user_home()`, `from_custom_root()`). This exact 3-line pattern appears identically in all three constructors.
Suggestion: Extract this into a shared helper function or incorporate it into the proposed `new()` constructor method mentioned above. This would reduce the duplication and ensure consistent directory creation behavior across all constructor variants.

---
*This issue was automatically created by `sah rule check --create-todos`*
