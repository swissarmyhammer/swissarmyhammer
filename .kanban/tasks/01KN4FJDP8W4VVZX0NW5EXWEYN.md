---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffa480
title: Add tests for HardenedSecurityValidator::validate_command_comprehensive
---
hardening.rs:574-639\n\nCoverage: 0% (~50 lines uncovered)\n\nUncovered lines: 565, 567-568, 574, 582-639\n\n```rust\npub fn validate_command_comprehensive(\n    &mut self, command: &str, working_dir: &Path,\n    environment: &HashMap<String, String>, context: CommandContext,\n) -> Result<SecurityAssessment, ShellSecurityError>\n```\n\nOrchestrates base validation + threat detection + audit logging. Test with:\n- Safe command → Ok with ThreatLevel::None\n- Malicious command → Ok with High threat level\n- Invalid command that fails base validation → Err\n- Threat detection disabled → always returns None threat level #coverage-gap