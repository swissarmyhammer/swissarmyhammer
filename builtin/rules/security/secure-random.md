---
title: Require Cryptographically Secure Random
description: Requires cryptographically secure random number generators for security-sensitive operations
category: security
severity: warning
tags: ["security", "cryptography", "random"]
---

Check for use of insecure random number generators in {{ language }} code for security-sensitive operations.

Insecure patterns (flag these):
- Math.random() in JavaScript for tokens, passwords, or cryptographic keys
- random.random() in Python for security purposes
- rand() in C/C++ for cryptographic operations
- Random class in Java for security-sensitive data
- mt_rand() or rand() in PHP for security tokens

Secure alternatives (these are OK):
- crypto.randomBytes() or crypto.getRandomValues() in JavaScript
- secrets module in Python (secrets.token_hex(), secrets.token_urlsafe())
- arc4random() or /dev/urandom in C/C++
- SecureRandom in Java
- random_bytes() in PHP

Context matters:
- Using Math.random() for UI animations or non-security purposes is OK
- Only flag usage that appears to be for security (tokens, keys, passwords, sessions, nonces)

If this file doesn't generate random numbers or isn't security-related, respond with "PASS".

If you find insecure random usage in security contexts:
- Report the line number
- Explain why the current random generator is insecure
- Suggest the appropriate secure alternative for {{ language }}

If no insecure usage is found, respond with "PASS".

Code to analyze:
```
{{ target_content }}
```
