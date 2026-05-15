---
position_column: done
position_ordinal: ffff8a80
title: '[WARNING] ViewsContext missing Debug derive on public types'
---
Public types ViewsContext, ViewsContextBuilder, and ViewsChangelog in swissarmyhammer-views do not derive Debug. While consistent with the existing FieldsContext pattern, the Rust API guidelines recommend public types implement Debug. Consider adding `#[derive(Debug)]` or a manual impl that redacts the internal state. Low priority since it matches existing project convention. #warning