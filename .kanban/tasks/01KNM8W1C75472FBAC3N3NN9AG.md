---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffc880
title: '[warning] VirtualTagDisplay hardcodes tag metadata instead of reading from schema'
---
File: kanban-app/ui/src/components/fields/displays/virtual-tag-display.tsx\n\nThe VIRTUAL_TAG_META constant hardcodes color and description for READY, BLOCKED, and BLOCKING tags. The comment says these \"mirror the Rust DEFAULT_REGISTRY in virtual_tags.rs\" but this creates a maintenance burden -- if the Rust side adds a new virtual tag or changes a color, the frontend must be manually updated.\n\nThe architecture doc states: \"UI interprets Field metadata. Never hardcode field-specific rendering logic in React.\" and \"No hardcoded field/entity logic in components.\"\n\nSuggestion: The virtual tag colors and descriptions should be provided by the backend (via the computed field value or a companion metadata endpoint) rather than duplicated in the frontend. At minimum, add a TODO comment acknowledging this duplication and referencing the Rust source. #review-finding