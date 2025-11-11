---
title: Data Protection
description: Check for hardcoded secrets, sensitive data exposure, and insecure cryptography
category: security
severity: error
tags: ["security", "secrets", "encryption", "data-protection"]
---

Check {{ language }} code for data protection issues.

Look for:
- Hardcoded credentials, API keys, or secrets
- Sensitive data in logs or error messages
- Weak or broken cryptography usage
- Missing encryption for sensitive data
- Insecure random number generation
- Passwords or tokens in version control

Common issues:
- API keys hardcoded in source: `api_key = "sk_live_12345"`
- Passwords in configuration files
- Sensitive data logged: `log.info("User password: {}", password)`
- Using deprecated crypto: MD5, SHA1 for passwords
- Weak encryption keys or algorithms
- Secrets in environment variables without protection

Do not flag:
- Public configuration values
- Properly managed secrets using secret managers
- Strong cryptography (AES-256, modern TLS, bcrypt)
- Encrypted data at rest and in transit
- Secure key management practices

{% include "_partials/report-format" %}

Report issues with:
- Type of data protection issue
- Location and what is exposed
- Suggested secure alternative

{% include "_partials/pass-response" %}
