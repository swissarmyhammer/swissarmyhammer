---
position_column: done
position_ordinal: d1
title: 'Card 2: get_board_data command returning entity bags with counts'
---
New command returns board structure with columns/swimlanes/tags as raw entity bags (Entity::to_json()) with computed counts injected as extra fields. Keep existing get_board unchanged.