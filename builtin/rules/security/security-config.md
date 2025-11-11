---
title: Security Configuration
description: Check for insecure configurations, debug mode in production, and missing security headers
category: security
severity: warning
tags: ["security", "configuration", "hardening"]
---

Check {{ language }} code for security configuration issues.

Look for:
- Debug mode enabled in production code
- Verbose error messages that leak information
- Insecure default configurations
- Missing security headers
- CORS configured too permissively
- Disabled security features

Common issues:
- Debug flags: `debug = true`, `NODE_ENV=development`
- Stack traces exposed to users
- Default credentials still in use
- Missing security headers: CSP, HSTS, X-Frame-Options
- CORS allowing all origins: `Access-Control-Allow-Origin: *`
- TLS/SSL disabled or weakened

Do not flag:
- Debug mode in test or development configuration
- Appropriate error messages without sensitive data
- Properly configured security headers
- Restricted CORS policies
- Strong TLS configuration

{% include "_partials/report-format" %}

Report issues with:
- Configuration issue type
- Location and current setting
- Recommended secure configuration

{% include "_partials/pass-response" %}
