# Install all CLI binaries
install:
    cargo install --path swissarmyhammer-cli
    cargo install --path avp-cli
    cargo install --path mirdan-cli
    just mirdan-dev
    just kanban-dev

# Build the Mirdan tray app (debug) and install to /Applications for deep link testing
mirdan-dev:
    cd mirdan-app && cargo tauri build --debug
    rm -rf /Applications/Mirdan.app
    hdiutil attach target/debug/bundle/dmg/Mirdan_*.dmg -nobrowse -quiet
    cp -r /Volumes/Mirdan/Mirdan.app /Applications/
    hdiutil detach /Volumes/Mirdan -quiet
    @echo "Installed Mirdan.app to /Applications"
    @echo "Test deep links: open mirdan://install/no-secrets"

# Build the Kanban app (debug) and install to /Applications
kanban-dev:
    cd kanban-app && cargo tauri build --debug
    rm -rf /Applications/Kanban.app
    hdiutil attach target/debug/bundle/dmg/Kanban_*.dmg -nobrowse -quiet
    cp -r /Volumes/Kanban/Kanban.app /Applications/
    hdiutil detach /Volumes/Kanban -quiet
    @echo "Installed Kanban.app to /Applications"

# Run the installed Mirdan tray app
mirdan-run:
    open /Applications/Mirdan.app

# Tail Mirdan logs in Console (os_log)
mirdan-logs:
    log stream --predicate 'subsystem == "ai.mirdan.app"' --level debug

# Tail Kanban logs in Console (os_log)
kanban-logs:
    log stream --predicate 'subsystem == "com.swissarmyhammer.kanban"' --level debug

# Tail all app logs (both Mirdan and Kanban)
logs:
    log stream --predicate 'subsystem == "ai.mirdan.app" OR subsystem == "com.swissarmyhammer.kanban"' --level debug
