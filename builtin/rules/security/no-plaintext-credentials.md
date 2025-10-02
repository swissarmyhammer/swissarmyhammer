---
title: No Plaintext Credentials in Configuration
description: Detects plaintext credentials and sensitive data in configuration files
category: security
severity: error
tags: ["security", "credentials", "configuration"]
---

Check for plaintext credentials and sensitive data in {{ language }} configuration files.

Look for in configuration/environment files:
- Database passwords (e.g., password=, db_password=, DB_PASS=)
- API keys and secrets (e.g., api_key=, secret_key=, client_secret=)
- Authentication tokens (e.g., token=, auth_token=, access_token=)
- Private keys or certificates inline
- SMTP passwords
- Cloud provider credentials (AWS access keys, Azure keys, GCP credentials)

Safe patterns (these are OK):
- Environment variable references (e.g., ${DATABASE_PASSWORD}, $ENV_VAR)
- Placeholder values (e.g., "your-api-key-here", "CHANGE_ME", "TODO")
- Example or template files clearly marked as non-production
- Empty or null values

File types to check:
- .env files (but .env.example or .env.template with placeholders is OK)
- .yaml, .yml configuration files
- .toml configuration files
- .json configuration files
- .ini files
- .properties files

If this file is clearly a template, example, or documentation file (contains "example", "template", "sample" in name or has placeholder values), respond with "PASS".

If you find actual credentials:
- Report the line number
- Identify the type of credential
- DO NOT display the actual credential value
- Suggest using environment variables or a secrets manager

If no plaintext credentials are found, respond with "PASS".

Code to analyze:
```
{{ target_content }}
```
