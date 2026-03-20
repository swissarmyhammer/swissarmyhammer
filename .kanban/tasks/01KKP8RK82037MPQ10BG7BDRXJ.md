---
position_column: done
position_ordinal: d680
title: '[Low] Search mode tests lack entity-interaction coverage'
---
The search mode tests in command-palette.test.tsx (lines 242-347) verify rendering states (hint text, listbox role, backdrop click) but do not actually test the filtering behavior — no test types into the CM6 editor and verifies that matching entities appear in the list. The `it('calls inspect and onClose when a search result is clicked')` test (line 329) sets up entities but never actually produces filtered results (it just advances timers and unmounts).\n\nThe debounce makes this harder to test but the existing command-mode tests show the pattern works. Consider adding at least one test that programmatically sets the filter state and verifies entity results render.\n\nSeverity: Low (test coverage gap)"