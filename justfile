# Install all CLI binaries
# --locked: build against the workspace Cargo.lock (what CI/tests verified).
# A bare `cargo install --path` re-resolves dependencies fresh, so a new
# upstream release (e.g. time 0.3.48 breaking cookie 0.18.1) fails the
# install even though the workspace builds clean.
install:
    cargo install --locked --path apps/swissarmyhammer-cli
    cargo install --locked --path apps/mirdan-cli
    cargo install --locked --path apps/kanban-cli
    cargo install --locked --path apps/code-context-cli

sah:
    cargo install --locked --path apps/swissarmyhammer-cli

kanban:
    cargo install --path apps/kanban-cli

mirdan:
    cargo install --locked --path apps/mirdan-cli

outdated:
    cargo install cargo-edit
    cargo upgrade --dry-run
