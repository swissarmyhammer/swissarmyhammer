---
title: Authentication and Authorization
description: Check for weak authentication mechanisms and missing authorization checks
category: security
severity: error
tags: ["security", "authentication", "authorization"]
---

Check {{ language }} code for authentication and authorization issues.

Look for:
- Endpoints or functions without authentication checks
- Missing authorization checks for sensitive operations
- Weak password requirements or storage
- Session management issues
- Missing token validation
- Insufficient permission checks

Common issues:
- Public endpoints that should require authentication
- Authorization checks that can be bypassed
- Passwords stored in plaintext or weak hashes
- Sessions without proper timeout or invalidation
- JWT tokens not validated properly
- Role checks that don't verify permissions

Do not flag:
- Public endpoints that are intentionally unrestricted
- Properly implemented authentication middleware
- Secure password hashing (bcrypt, argon2, scrypt)
- Properly validated and signed tokens

{% include "_partials/report-format" %}

Report issues with:
- Issue type and severity
- Location and affected endpoint/function
- Suggested security improvement

{% include "_partials/pass-response" %}
