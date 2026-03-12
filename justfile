# Install all CLI binaries
install:
    cargo install --path swissarmyhammer-cli
    cargo install --path avp-cli
    cargo install --path mirdan-cli

# Build the Mirdan tray app (debug) and install to /Applications for deep link testing
mirdan-dev:
    cd mirdan-app && cargo tauri build --debug
    rm -rf /Applications/Mirdan.app
    hdiutil attach target/debug/bundle/dmg/Mirdan_*.dmg -nobrowse -quiet
    cp -r /Volumes/Mirdan/Mirdan.app /Applications/
    hdiutil detach /Volumes/Mirdan -quiet
    @echo "Installed Mirdan.app to /Applications"
    @echo "Test deep links: open mirdan://install/no-secrets"

# Run the installed Mirdan tray app
mirdan-run:
    open /Applications/Mirdan.app

# Tail Mirdan logs in Console (os_log)
mirdan-logs:
    log stream --predicate 'subsystem == "ai.mirdan.app"' --level debug
