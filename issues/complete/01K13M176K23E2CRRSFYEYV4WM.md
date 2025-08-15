```
 cargo run search query "duckdb" 
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.19s
     Running `target/debug/swissarmyhammer search query duckdb`
2025-07-26T15:00:49.069727Z  INFO swissarmyhammer: Running search command
🔍 Starting semantic search query...
Searching for: duckdb
Result limit: 10
2025-07-26T15:00:49.096837Z  INFO swissarmyhammer::semantic::storage: Initializing DuckDB vector storage at: /Users/wballard/.swissarmyhammer/semantic.db
2025-07-26T15:00:49.100901Z  INFO swissarmyhammer::semantic::storage: Database schema initialized successfully
2025-07-26T15:00:49.100970Z  INFO swissarmyhammer::semantic::embedding: Initializing fastembed embedding engine with model: all-MiniLM-L6-v2
```

when running search or index at the root of a repository, this case swissarmyhammer itself, the semantic db was created in my home directory.

i already gave you an issue about this, I really mean it -- the semantic db should NEVER be in the user home directory, it ONLY should be in a .swissarmyhammer directory in a local repository


get to it, don't fuck this up yet again.