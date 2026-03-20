---
position_column: done
position_ordinal: ffffffa580
title: Verify no remaining home_dir patterns and run tests
---
Run cargo nextest for agents, mirdan, prompts. Verify no home_dir().join(".agents") or home_dir().join(".avp") patterns remain.