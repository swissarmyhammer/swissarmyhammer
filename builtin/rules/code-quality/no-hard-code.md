---
title: No hard coding
description: Detect the mis use of hard coding to make tests appear to pass
category: code-quality
severity: error
tags: ["code-quality", "cleanup"]
denied_tools:
  - ".*"
---

DO implement a solution that works correctly for all valid inputs, not just the test cases.
DO NOT hard-code values or create solutions that only work for specific test inputs.
