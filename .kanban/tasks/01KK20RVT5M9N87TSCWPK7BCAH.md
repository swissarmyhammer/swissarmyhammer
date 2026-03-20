---
position_column: done
position_ordinal: ffffb780
title: 'WARNING: setFocus fire-and-forget invoke may silently lose errors'
---
entity-focus-context.tsx line 45: `invoke('set_focus', { scopeChain: chain }).catch(() => {})`. The empty catch swallows all errors from the Rust side, including potential serialization errors or command-not-found errors. At minimum, log the error: `.catch((e) => console.error('set_focus failed:', e))`.