---
title: No Commented-Out Code
description: Detect large blocks of commented-out code
category: code-quality
severity: error
tags: ["code-quality", "cleanup"]
---

Check for large blocks (> 5 lines) of commented-out code in {{ language }}.

Commented-out code should be removed (we have source control).

Look for:
- Multiple consecutive lines of commented code
- Entire functions or classes that are commented out
- Code blocks that appear to be temporarily disabled

Do not flag:
- Regular documentation comments
- TODO/FIXME comments with explanations
- Example code in comments

If this file has no commented code blocks, respond with "PASS".
