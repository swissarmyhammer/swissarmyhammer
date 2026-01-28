---
name: no-commented-code
description: Detect large blocks of commented-out code
severity: error
trigger: Stop
tags:
  - code-quality
  - cleanup
timeout: 30
---

Check for large blocks (> 5 lines) of commented-out code.

Commented-out code should be removed (we have source control).

Look for:
- Multiple consecutive lines of commented code
- Entire functions or classes that are commented out
- Code blocks that appear to be temporarily disabled

Do not flag:
- Regular documentation comments
- TODO/FIXME comments with explanations
- Example code in comments
