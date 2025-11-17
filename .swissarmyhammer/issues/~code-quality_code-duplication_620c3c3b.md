# Rule Violation: code-quality/code-duplication

**File**: builtin/prompts/tool_use.md.liquid
**Severity**: ERROR

## Violation Message

VIOLATION
Rule: code-quality/code-duplication
File: builtin/prompts/tool_use.md.liquid
Line: 5-6
Severity: warning
Message: Duplicate emphasis on .swissarmyhammer/issues and .swissarmyhammer/memos importance. The same message about these directories not being .gitignored and being important metadata is repeated twice in consecutive lines with slightly different wording.
Suggestion: Consolidate into a single statement: "**IMPORTANT** .swissarmyhammer/issues and .swissarmyhammer/memos contain important metadata and should never be .gitignored - always commit them to git with your code changes"

---

VIOLATION
Rule: code-quality/code-duplication
File: builtin/prompts/tool_use.md.liquid
Line: 47-72
Severity: info
Message: The "Task Management" section (lines 47-72) contains nearly identical content to the example shown in lines 54-72, which appears to duplicate task management instructions that likely exist elsewhere in the codebase. The entire section including the example pattern is repeated verbatim.
Suggestion: This section appears to be duplicated from another prompt file. Consider extracting this task management guidance into a shared partial template (e.g., `_task_management.md.liquid`) and including it with `{% include 'task_management' %}` to maintain consistency across prompts and enable single-source updates.

---
*This issue was automatically created by `sah rule check --create-todos`*
