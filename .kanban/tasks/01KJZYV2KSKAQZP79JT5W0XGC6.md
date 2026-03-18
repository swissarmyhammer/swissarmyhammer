---
position_column: done
position_ordinal: fffc80
title: 'WARNING: Public types missing Debug derive'
---
views/src/context.rs, views/src/changelog.rs\n\nViewsContext, ViewsContextBuilder, ViewsChangelog are pub but don't derive Debug.\n\nFix: add Debug derive to all three.