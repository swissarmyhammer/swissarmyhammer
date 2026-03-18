---
position_column: done
position_ordinal: ffffffbf80
title: Verify no .swissarmyhammer directory references remain and run tests
---
Run cargo nextest run -p swissarmyhammer-common and -p swissarmyhammer-config, then verify grep shows no remaining references