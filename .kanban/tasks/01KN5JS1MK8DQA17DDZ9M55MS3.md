---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff8680
title: 'Fix: remove tag operation not working with StoreHandle write path'
---
## What

User reported that "remove tag" doesn't work after the StoreHandle migration. The untag operation modifies the body to remove #tag patterns, but the tag is not actually removed.

This is likely a computed field interaction issue — `tags` is a computed field derived from `#tag` patterns in the body. The untag operation reads the entity (with computed tags), modifies the body, writes back. With the new StoreHandle path, something in this pipeline is broken.

Needs investigation and a targeted test to reproduce before fixing.

## Acceptance Criteria
- [ ] Untag operation works correctly with StoreHandle path
- [ ] Test proves tag is removed from body after untag
- [ ] Computed tags field reflects the body change on re-read