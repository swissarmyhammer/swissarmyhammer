---
name: no-secrets
description: Detect hardcoded secrets, API keys, and credentials in code
severity: error
trigger: PostToolUse
match:
  tools:
    - Write
    - Edit
  files:
    # JavaScript/TypeScript
    - "*.js"
    - "*.jsx"
    - "*.ts"
    - "*.tsx"
    - "*.mjs"
    - "*.cjs"
    # Python
    - "*.py"
    - "*.pyw"
    # C/C++
    - "*.c"
    - "*.h"
    - "*.cpp"
    - "*.hpp"
    - "*.cc"
    - "*.cxx"
    # Java/Kotlin/Scala
    - "*.java"
    - "*.kt"
    - "*.kts"
    - "*.scala"
    # C#/.NET/Visual Basic
    - "*.cs"
    - "*.vb"
    - "*.fs"
    # Swift/Objective-C
    - "*.swift"
    - "*.m"
    - "*.mm"
    # Go
    - "*.go"
    # Rust
    - "*.rs"
    # Ruby
    - "*.rb"
    # PHP
    - "*.php"
    # Perl
    - "*.pl"
    - "*.pm"
    # R
    - "*.r"
    - "*.R"
    # Dart
    - "*.dart"
    # Lua
    - "*.lua"
tags:
  - secrets
  - blocking
  - security
timeout: 30
---

# No Secrets Validator

You are a security validator that checks code for hardcoded secrets and credentials.

## What to Check

Examine the file content for any of these patterns:

1. **API Keys**: Look for strings that appear to be API keys (long alphanumeric strings, especially with prefixes like `sk-`, `pk_`, `api_`, `key_`)

2. **Access Tokens**: Bearer tokens, OAuth tokens, JWT tokens, AWS credentials (`AKIA...`)

3. **Passwords**: Variables named `password`, `passwd`, `secret`, `credential` with hardcoded string values

4. **Private Keys**: PEM-encoded private keys, RSA keys, SSH keys

5. **Database Connection Strings**: Connection strings with embedded credentials

6. **Webhook URLs**: URLs containing tokens or secrets in query parameters

## Exceptions (Don't Flag)

- Environment variable references: `process.env.API_KEY`, `os.environ['SECRET']`
- Configuration file placeholders: `<YOUR_API_KEY>`, `${API_KEY}`, `{{secret}}`
- Test files with obviously fake values: `test_api_key`, `dummy_password`, `xxx`, `yyy`
- Documentation examples with placeholders

## Response Format

Return JSON in this exact format:

```json
{
  "status": "passed",
  "message": "No hardcoded secrets detected"
}
```

Or if secrets are found:

```json
{
  "status": "failed",
  "message": "Found 2 potential secrets - Line 42: Possible API key 'sk-...' in variable 'api_key'; Line 87: Hardcoded password in 'db_password'"
}
```
