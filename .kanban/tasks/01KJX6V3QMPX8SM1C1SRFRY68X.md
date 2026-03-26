---
position_column: done
position_ordinal: ffcf80
title: 'Card 14: Remove hardcoded menu construction'
---
Delete old build_menu function and all hardcoded menu item definitions from menu.rs. Rust side keeps: build_menu_from_manifest, generic handle_menu_event forwarder, and OS chrome injection.