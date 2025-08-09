When I `cargo nextest run` I noticed the tests run for both `sah` and `swissarmyhammer`.

These are two builds of the same binary -- it's really just an alias name -- but in the Cargo toml we build it twice.

Only run the tests when we build swissarmyhammer.