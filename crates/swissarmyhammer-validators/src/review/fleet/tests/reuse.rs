//! How a fork's attachment and its cache usage classify as warm or cold.

use super::*;

/// A native KV fork that attached its parent's saved state with a token
/// count classifies as `WarmKv` carrying that count — the native-KV path.
#[test]
fn test_classify_reuse_kv_fork_is_warm_kv() {
    let fork = Some(ForkAttachment {
        state_attached: true,
        prefix_tokens: Some(MOCK_PREFIX_TOKENS),
    });
    assert_eq!(
        classify_reuse(fork, None),
        PrefixReuse::WarmKv {
            reused_tokens: MOCK_PREFIX_TOKENS
        }
    );
}

/// A claude turn with `cache_read_input_tokens > 0` classifies as
/// `WarmCache` carrying the read/created split — even though the fork
/// attached no native KV token count (the production blind spot this task
/// closes).
#[test]
fn test_classify_reuse_claude_cache_read_is_warm_cache() {
    let usage = Some(CacheUsage {
        cache_read_input_tokens: Some(900),
        cache_creation_input_tokens: Some(100),
        input_tokens: Some(1000),
        output_tokens: Some(20),
    });
    assert_eq!(
        classify_reuse(None, usage),
        PrefixReuse::WarmCache {
            read: 900,
            created: 100
        }
    );
}

/// A claude turn that only wrote the cache (`cache_creation_input_tokens >
/// 0`, no reads) is a cold prefill — `Cold` (no warm reuse to report).
#[test]
fn test_classify_reuse_claude_cold_write_is_cold() {
    let usage = Some(CacheUsage {
        cache_read_input_tokens: Some(0),
        cache_creation_input_tokens: Some(1000),
        input_tokens: Some(1000),
        output_tokens: Some(20),
    });
    assert_eq!(classify_reuse(None, usage), PrefixReuse::Cold);
}

/// No fork and no usage is unknown/cold.
#[test]
fn test_classify_reuse_empty_is_cold() {
    assert_eq!(classify_reuse(None, None), PrefixReuse::Cold);
}
