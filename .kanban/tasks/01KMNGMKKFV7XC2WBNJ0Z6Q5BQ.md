---
assignees:
- claude-code
depends_on:
- 01KMNGKRB4EBYE1RCD43P0JSE5
position_column: done
position_ordinal: ffffffffffe280
title: 'shelltool-cli: soldier turtle banner'
---
Create ASCII art banner from the soldier turtle image (~/Downloads/image-DuqIuOeDnvpo47cByj5yqlvtIvRLFv.png) + \"SHELLTOOL\" in ANSI Shadow block letters.\n\n## Pattern\nFollow avp-cli/src/banner.rs exactly:\n- ANSI 256-color gradient (green tones for turtle theme)\n- Respects NO_COLOR env var and non-TTY detection\n- render_banner(out, use_color) for testability\n- print_banner() public function\n- Show on interactive --help or no-args-with-terminal\n\n## Files\n- shelltool-cli/src/banner.rs\n\n## Acceptance\n- `shelltool` (interactive, no args) shows turtle + SHELLTOOL banner\n- `shelltool --help` shows banner before help\n- Piped input does NOT show banner\n- NO_COLOR=1 produces plain text