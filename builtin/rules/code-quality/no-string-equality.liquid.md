---
title: No String Equality
description: Detect the mis use of stringify for equality checks
category: code-quality
severity: error
tags: ["code-quality", "cleanup"]
denied_tools:
  - ".*"
---

DO NOT turn data into strings just to compare for equality
DO implement equality methods in a language appropriate pattern to compare for equality
