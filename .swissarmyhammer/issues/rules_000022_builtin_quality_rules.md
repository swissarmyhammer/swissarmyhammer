# Create Builtin Code Quality Rules

Refer to ideas/rules.md

## Goal

Create example builtin code quality rules.

## Context

Code quality rules help maintain clean, maintainable code and demonstrate the rules system's capabilities.

## Implementation

1. Create in `builtin/rules/code-quality/`:

2. **function-length.md**:
```markdown
---
title: Function Length Limit
description: Functions should be less than 50 lines
category: code-quality
severity: warning
tags: ["code-quality", "maintainability"]
---

Check {{ language }} code for functions longer than 50 lines.

Count actual code lines (excluding comments and blank lines).

Report any functions over 50 lines with:
- Function name
- Current line count
- Suggestion to break into smaller functions

If this file doesn't define functions, respond with "PASS".
```

3. **no-commented-code.md**:
```markdown
---
title: No Commented-Out Code
description: Detect large blocks of commented-out code
category: code-quality
severity: info
tags: ["code-quality", "cleanup"]
---

Check for large blocks (> 5 lines) of commented-out code in {{ language }}.

Commented-out code should be removed (we have source control).

If this file has no commented code blocks, respond with "PASS".
```

4. **cognitive-complexity.md**:
```markdown
---
title: Cognitive Complexity
description: Limit cognitive complexity of functions
category: code-quality
severity: warning
tags: ["code-quality", "complexity"]
---

Analyze {{ language }} code for high cognitive complexity (nested ifs, loops, etc).

Flag functions with:
- Deeply nested conditions (> 3 levels)
- Many branches
- Complex boolean logic

Suggest refactoring strategies.

If this file doesn't define functions, respond with "PASS".
```

5. Create 1-2 more quality rules

## Testing

- Test each rule manually
- Verify they catch issues
- Verify they pass clean code

## Success Criteria

- [ ] 4-5 code quality rules created
- [ ] All rules tested
- [ ] Rules work correctly
- [ ] Documentation clear
