# Create Builtin Security Rules

Refer to ideas/rules.md

## Goal

Create example builtin security rules to ship with the system.

## Context

Security rules demonstrate the rules system and provide immediate value to users.

## Implementation

1. Create in `builtin/rules/security/`:

2. **no-hardcoded-secrets.md**:
```markdown
---
title: No Hardcoded Secrets
description: Detects hardcoded API keys, passwords, and tokens in code
category: security
severity: error
tags: ["security", "secrets", "credentials"]
---

Check the following {{ language }} code for hardcoded secrets.

Look for:
- API keys (e.g., API_KEY = "sk_live_...")
- Passwords in plain text
- Auth tokens
- Private keys

If this file type doesn't contain code (e.g., markdown, config files), respond with "PASS".

Report any findings with line numbers and suggestions.
```

3. **no-eval.md**:
```markdown
---
title: No Dangerous eval() Usage
description: Detects dangerous eval() or exec() usage
category: security
severity: error
tags: ["security", "code-injection"]
---

Check for dangerous eval() or exec() usage in {{ language }} code.

Flag any use of:
- eval()
- exec()
- Function() constructor with dynamic code

If this file doesn't support these patterns, respond with "PASS".
```

4. Create 2-3 more security rules

## Testing

- Test each rule manually with test command
- Verify they catch actual issues
- Verify they pass clean code

## Success Criteria

- [ ] 4-5 security rules created
- [ ] All rules tested
- [ ] Rules catch real issues
- [ ] Documentation clear
