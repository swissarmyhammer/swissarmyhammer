---
position_column: done
position_ordinal: ffffb380
title: 'nit: mirdan/src/doctor.rs check_credentials still refers to ~/.mirdan/credentials in fix text'
---
`mirdan/src/doctor.rs:228`\n\nThe fix suggestion for a missing agent config says:\n```\n\"Check ~/.mirdan/agents.yaml for syntax errors\"\n```\nand the credentials check says:\n```\n\"~/.mirdan/credentials\"\n```\n\nNeither path has been updated to its XDG equivalent. If users follow these instructions they will look in the wrong place.\n\nSuggestion: Update these strings to reference the XDG paths, e.g.:\n- `\"Check $XDG_CONFIG_HOME/mirdan/agents.yaml (defaults to ~/.config/mirdan/agents.yaml) for syntax errors\"`\n- `\"$XDG_CONFIG_HOME/mirdan/credentials\"` #review-finding