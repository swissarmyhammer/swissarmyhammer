---
position_column: done
position_ordinal: fffffd80
title: Fix avatar to show data:image and use round shape
---
Avatar component should:
1. Actually render the data:image avatar field when present (currently showing initials fallback even when avatar exists)
2. Use fully round shape (rounded-full) instead of rounded rectangle

- [ ] Read avatar.tsx and fix image rendering logic
- [ ] Ensure all avatar sizes use rounded-full
- [ ] Verify the OS actor's avatar data URI is being persisted correctly in state.rs
- [ ] Test passes