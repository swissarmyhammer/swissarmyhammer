---
name: review-security
title: Security Code Review
description: Perform a comprehensive security review of code to identify vulnerabilities
parameters:
  - name: context
    description: Context about the code (e.g., "handles user authentication")
    required: false
    default: "general purpose code"
  - name: severity_threshold
    description: Minimum severity to report (critical, high, medium, low)
    required: false
    default: "low"
---

## Code Under Review

Please review the all code in this project with a focus on: {{context}}

## Security Analysis

{% render "principals" %}
{% render "coding_standards" %}

### 1. Vulnerability Scan

Analyzing for common security vulnerabilities:

#### Input Validation

- SQL Injection risks
- Command Injection vulnerabilities
- Path Traversal attacks
- Cross-Site Scripting (XSS)
- XML/XXE injection

#### Authentication & Authorization

- Weak authentication mechanisms
- Missing authorization checks
- Session management issues
- Insecure password handling

#### Data Protection

- Sensitive data exposure
- Insecure cryptography usage
- Missing encryption for data in transit
- Hardcoded secrets or credentials

#### Security Misconfigurations

- Debug mode in production
- Verbose error messages
- Insecure defaults
- Missing security headers

### 2. Code-Specific Vulnerabilities

Based on the language and context:

- Race conditions
- Memory safety issues
- Resource leaks
- Denial of Service vectors

### 3. Severity Classification

Rate findings by severity ({{severity_threshold}} and above):

- **Critical**: Immediate exploitation risk
- **High**: Significant security impact
- **Medium**: Moderate risk requiring attention
- **Low**: Best practice violations

### 4. Recommendations

#### Immediate Actions

- Critical fixes that must be addressed
- Security patches to apply
- Configuration changes needed

#### Best Practices

- Secure coding patterns to adopt
- Libraries or frameworks that help
- Security testing to implement

#### Long-term Improvements

- Architectural changes for better security
- Security training recommendations
- Process improvements

### 5. Secure Code Example

Provide refactored code that addresses the security issues identified.

## Process

- list all source files in the project and create a markdown scratchpad file, this is your todo list
- create a CODE_REVIEW.md markdown file, this is your code review output
- for each file in the todo list
  - perform the Security Analysis
  - summarize your findings
  - write your findings to the code review output

{% render "review_format" %}
