---
title: No Hardcoded Secrets
description: Detects hardcoded API keys, passwords, and tokens in code
category: security
severity: error
tags: ["security", "secrets", "credentials"]
---

Check the following {{ language }} code for hardcoded secrets.

Look for:
- API keys (e.g., API_KEY = "sk_live_...", "api_key": "...", apiKey = "...")
- Passwords in plain text (e.g., password = "...", PASSWORD = "...")
- Auth tokens (e.g., token = "...", TOKEN = "...", bearer_token = "...")
- Private keys (e.g., private_key = "...", PRIVATE_KEY = "...")
- Database credentials (e.g., db_password = "...", DB_PASS = "...")
- OAuth secrets (e.g., client_secret = "...", oauth_secret = "...")

If this file type doesn't contain code (e.g., markdown, documentation files), respond with "PASS".

{% include "_partials/report-format" %}

If you find potential hardcoded secrets:
- Describe what type of secret was found
- Suggest moving to environment variables or a secrets manager

{% include "_partials/no-display-secrets" %}

{% include "_partials/pass-response" %}

{% include "_partials/code-block" %}
