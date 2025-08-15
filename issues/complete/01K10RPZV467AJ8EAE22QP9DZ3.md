```
 cargo run search index "**.*.rs"
   Compiling ring v0.17.14
   Compiling rustls v0.23.29
   Compiling rustls-webpki v0.103.4
   Compiling ureq v2.12.1
   Compiling hf-hub v0.3.2
   Compiling swissarmyhammer v0.1.0 (/Users/wballard/github/swissarmyhammer/swissarmyhammer)
   Compiling swissarmyhammer-cli v0.1.0 (/Users/wballard/github/swissarmyhammer/swissarmyhammer-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.80s
     Running `target/debug/swissarmyhammer search index '**.*.rs'`
2025-07-25T12:24:48.387844Z  INFO swissarmyhammer: Running search command
🔍 Starting semantic search indexing...
Indexing files matching: **.*.rs
2025-07-25T12:24:48.388766Z  INFO swissarmyhammer::semantic::storage: Initializing vector storage at: .swissarmyhammer/semantic.db (using in-memory fallback)
❌ Indexing failed: Invalid configuration: NOMIC_API_KEY environment variable is required
```

Nope -- I want the embedding done in process with fast embed like I said https://github.com/Anush008/fastembed-rs.

Get rid of NOMIC_API_KEY.

Auto download the model.

## Proposed Solution

I will replace the external NOMIC API dependency with local fastembed-rs processing by:

1. **Adding fastembed-rs dependency** - Update Cargo.toml to include fastembed-rs crate
2. **Refactor embedding layer** - Replace NOMIC API calls with local fastembed-rs embedding generation
3. **Remove environment variable requirement** - Eliminate NOMIC_API_KEY dependency completely
4. **Implement auto-download** - Configure fastembed-rs to automatically download required embedding models on first use
5. **Update configuration** - Modify semantic search initialization to use local embeddings
6. **Test integration** - Verify semantic search indexing works without external API dependencies

This approach provides better privacy, eliminates external API dependencies, and removes the configuration burden of managing API keys.
