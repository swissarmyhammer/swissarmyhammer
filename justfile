# Install all CLI binaries
install:
    cargo install --path swissarmyhammer-cli
    cargo install --path avp-cli
    cargo install --path mirdan-cli
    cargo install --path kanban-cli
    cargo install --path shelltool-cli
    cargo install --path code-context-cli
    just mirdan-build
    just kanban-build

# Build the Mirdan tray app (debug) and install to /Applications for deep link testing
mirdan-build:
    cd mirdan-app && cargo tauri build --debug
    rm -rf /Applications/Mirdan.app
    hdiutil attach target/debug/bundle/dmg/Mirdan_*.dmg -nobrowse -quiet
    cp -r /Volumes/Mirdan/Mirdan.app /Applications/
    hdiutil detach /Volumes/Mirdan -quiet
    @echo "Installed Mirdan.app to /Applications"
    @echo "Test deep links: open mirdan://install/no-secrets"

# Build the Kanban app (debug) and install to /Applications
kanban-build:
    cd kanban-app && cargo tauri build --debug
    rm -rf /Applications/Kanban.app
    hdiutil attach target/debug/bundle/dmg/Kanban_*.dmg -nobrowse -quiet
    cp -r /Volumes/Kanban/Kanban.app /Applications/
    hdiutil detach /Volumes/Kanban -quiet
    @echo "Installed Kanban.app to /Applications"
    @echo "Test deep links: kanban open ."

# Run the installed Mirdan tray app
mirdan-run: mirdan-build
    open /Applications/Mirdan.app

# Run the installed Kanban app
kanban-run: kanban-build
    open /Applications/Kanban.app

# Run Mirdan in development mode (with Cargo)
mirdan-dev:
    cd mirdan-app && cargo tauri dev

# Run Kanban in development mode (with Cargo)
kanban-dev:
    cd kanban-app && cargo tauri dev

# Tail Mirdan logs in Console (os_log)
mirdan-logs:
    log stream --predicate 'subsystem == "ai.mirdan.app"' --level debug

# Tail Kanban logs in Console (os_log)
kanban-logs:
    log stream --predicate 'subsystem == "com.swissarmyhammer.kanban"' --level debug

# Tail all app logs (both Mirdan and Kanban)
logs:
    log stream --style compact --predicate 'subsystem == "ai.mirdan.app" OR subsystem == "com.swissarmyhammer.kanban"' --level debug
