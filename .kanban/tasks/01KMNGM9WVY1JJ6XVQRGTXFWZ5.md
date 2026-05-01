---
assignees:
- claude-code
depends_on:
- 01KMNGKRB4EBYE1RCD43P0JSE5
- 01KMNGKF1REG46ASTK8MX504GX
position_column: done
position_ordinal: ffffffffffe580
title: 'shelltool-cli: doctor command'
---
Implement `shelltool doctor` using DoctorRunner from swissarmyhammer-doctor.\n\n## Checks\n1. ShellExecuteTool's Doctorable health checks (config, patterns, Bash denied, skill deployed)\n2. shelltool binary in PATH\n3. MCP server registered in agent configs\n4. Git repository (warning if not found)\n\n## Files\n- shelltool-cli/src/doctor.rs\n- shelltool-cli/src/main.rs (dispatch)\n\n## Acceptance\n- `shelltool doctor` prints formatted table with all checks\n- `shelltool doctor --verbose` shows fix suggestions\n- Exit code 0/1/2 based on check status