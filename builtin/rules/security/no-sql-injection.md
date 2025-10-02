---
title: No SQL Injection Vulnerabilities
description: Detects potential SQL injection vulnerabilities from string concatenation in queries
category: security
severity: error
tags: ["security", "sql-injection", "database"]
---

Check for potential SQL injection vulnerabilities in {{ language }} code.

Look for:
- String concatenation or formatting to build SQL queries (e.g., "SELECT * FROM users WHERE id = " + userId)
- f-strings or template literals used to inject values into SQL (e.g., f"SELECT * FROM {table}")
- Raw SQL queries constructed from user input without parameterization
- Dynamic table or column names from untrusted sources

If this file type doesn't interact with databases (e.g., markdown, CSS, HTML), respond with "PASS".

Safe patterns (these are OK):
- Parameterized queries (e.g., cursor.execute("SELECT * FROM users WHERE id = ?", [userId]))
- ORM query builders (e.g., User.where(id: user_id))
- Query parameter placeholders (e.g., $1, $2 in PostgreSQL; ? in SQLite)

If you find potential SQL injection vulnerabilities:
- Report the line number
- Explain the vulnerability
- Suggest using parameterized queries or an ORM instead

If no vulnerabilities are found, respond with "PASS".

Code to analyze:
```
{{ target_content }}
```
