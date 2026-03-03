---
title: Scaffold Tauri crate as workspace member
position:
  column: done
  ordinal: a9
---
Create the swissarmyhammer-kanban-app/ directory as a new Cargo workspace member with Tauri v2 binary crate scaffold.

Files to create:
- swissarmyhammer-kanban-app/Cargo.toml — binary crate, deps: swissarmyhammer-kanban (path), tauri v2, tauri-build v2, clap, serde, serde_json, tokio, dirs, chrono
- swissarmyhammer-kanban-app/build.rs — tauri_build::build()
- swissarmyhammer-kanban-app/tauri.conf.json — productName, identifier, frontendDist ../ui/dist, devUrl localhost:5173, beforeDevCommand/beforeBuildCommand npm --prefix ui
- swissarmyhammer-kanban-app/capabilities/default.json — core:default only (kanban crate handles file I/O, no Tauri fs plugin needed)
- swissarmyhammer-kanban-app/src/main.rs — minimal placeholder

Also edit workspace root Cargo.toml to add "swissarmyhammer-kanban-app" to members list.

Verify: cargo check -p swissarmyhammer-kanban-app compiles.