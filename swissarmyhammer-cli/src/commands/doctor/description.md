Diagnose and troubleshoot your SwissArmyHammer setup in seconds.

Save hours of debugging time with comprehensive automated checks that identify
configuration issues, permission problems, and integration errors before they
impact your workflow.

WHAT IT CHECKS

The doctor command runs a complete health assessment of your environment:
• PATH Configuration - Verifies swissarmyhammer is accessible from your shell
• Claude Code Integration - Validates MCP server configuration and connectivity
• Prompt System - Checks directories, file permissions, and YAML syntax
• File Watching - Tests file system event monitoring capabilities
• System Resources - Validates required dependencies and system capabilities

WHY USE DOCTOR

• Quick Diagnosis - Complete system check in seconds, not hours
• Clear Reporting - Easy-to-understand pass/fail results with actionable guidance
• Early Detection - Catch configuration problems before they cause failures
• Setup Validation - Verify your installation is working correctly
• Integration Testing - Ensure Claude Code and MCP are properly connected

UNDERSTANDING RESULTS

Exit codes indicate the severity of findings:
  0 - All checks passed - System is healthy and ready
  1 - Warnings found - System works but has recommendations
  2 - Errors found - Critical issues preventing proper operation

COMMON WORKFLOWS

First-time setup verification:
  swissarmyhammer doctor

Detailed diagnostic output:
  swissarmyhammer doctor --verbose

After configuration changes:
  swissarmyhammer doctor

CI/CD health checks:
  swissarmyhammer doctor && echo "System ready"

EXAMPLES

Basic health check:
  swissarmyhammer doctor

Detailed diagnostics with fix suggestions:
  swissarmyhammer doctor --verbose

Quiet mode for scripting:
  swissarmyhammer doctor --quiet

The doctor command gives you confidence that your development environment
is properly configured and ready for AI-powered workflows.