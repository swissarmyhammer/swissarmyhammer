//! Profiling benchmark for repeated message generation
//!
//! This benchmark helps identify performance bottlenecks in multi-turn
//! conversation scenarios by profiling repeated message generations.
//!
//! Run with:
//! ```bash
//! cargo bench --bench generation_profiling -- --profile-time=10
//! ```
//!
//! For flamegraph profiling:
//! ```bash
//! cargo install flamegraph
//! cargo flamegraph --bench generation_profiling
//! ```

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use llama_agent::types::{
    AgentConfig, GenerationRequest, Message, MessageRole, ModelConfig, ModelSource, ParallelConfig,
    QueueConfig, RetryConfig, SessionConfig,
};
use llama_agent::{AgentAPI, AgentServer};
use std::time::SystemTime;
use tokio::runtime::Runtime;

/// Small model for profiling
const PROFILE_MODEL_REPO: &str = "unsloth/Qwen3-0.6B-GGUF";
const PROFILE_MODEL_FILE: &str = "Qwen3-0.6B-UD-Q4_K_XL.gguf";

fn create_profiling_config() -> AgentConfig {
    AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: PROFILE_MODEL_REPO.to_string(),
                filename: Some(PROFILE_MODEL_FILE.to_string()),
                folder: None,
            },
            batch_size: 64,
            use_hf_params: true,
            retry_config: RetryConfig {
                max_retries: 2,
                initial_delay_ms: 100,
                backoff_multiplier: 1.5,
                max_delay_ms: 1000,
            },
            debug: false, // Disable debug for cleaner profiling
            n_seq_max: 1,
            n_threads: 4,
            n_threads_batch: 4,
        },
        mcp_servers: Vec::new(),
        session_config: SessionConfig {
            persistence_enabled: true,
            ..Default::default()
        },
        parallel_execution_config: ParallelConfig::default(),
        queue_config: QueueConfig::default(),
    }
}

/// Benchmark: Single generation baseline
fn benchmark_single_generation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // Initialize agent once for all iterations
    let agent = rt.block_on(async {
        AgentServer::initialize(create_profiling_config())
            .await
            .expect("Failed to initialize agent")
    });

    c.bench_function("single_generation", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Create session
                let session = agent
                    .create_session()
                    .await
                    .expect("Failed to create session");

                // Add message
                agent
                    .add_message(
                        &session.id,
                        Message {
                            role: MessageRole::User,
                            content: "What is 2 + 2?".to_string(),
                            tool_call_id: None,
                            tool_name: None,
                            timestamp: SystemTime::now(),
                        },
                    )
                    .await
                    .expect("Failed to add message");

                // Generate
                let request = GenerationRequest::new(session.id).with_max_tokens(20);
                black_box(agent.generate(request).await.expect("Generation failed"));
            })
        });
    });
}

/// Benchmark: Three sequential generations (tests incremental processing)
fn benchmark_three_turn_conversation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let agent = rt.block_on(async {
        AgentServer::initialize(create_profiling_config())
            .await
            .expect("Failed to initialize agent")
    });

    c.bench_function("three_turn_conversation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let session = agent
                    .create_session()
                    .await
                    .expect("Failed to create session");
                let session_id = session.id;

                // Turn 1
                agent
                    .add_message(
                        &session_id,
                        Message {
                            role: MessageRole::User,
                            content: "What is 2 + 2?".to_string(),
                            tool_call_id: None,
                            tool_name: None,
                            timestamp: SystemTime::now(),
                        },
                    )
                    .await
                    .expect("Failed to add message");

                let request1 = GenerationRequest::new(session_id).with_max_tokens(20);
                black_box(agent.generate(request1).await.expect("Gen 1 failed"));

                // Turn 2
                agent
                    .add_message(
                        &session_id,
                        Message {
                            role: MessageRole::User,
                            content: "What is 5 + 5?".to_string(),
                            tool_call_id: None,
                            tool_name: None,
                            timestamp: SystemTime::now(),
                        },
                    )
                    .await
                    .expect("Failed to add message");

                let request2 = GenerationRequest::new(session_id).with_max_tokens(20);
                black_box(agent.generate(request2).await.expect("Gen 2 failed"));

                // Turn 3
                agent
                    .add_message(
                        &session_id,
                        Message {
                            role: MessageRole::User,
                            content: "What is 10 + 10?".to_string(),
                            tool_call_id: None,
                            tool_name: None,
                            timestamp: SystemTime::now(),
                        },
                    )
                    .await
                    .expect("Failed to add message");

                let request3 = GenerationRequest::new(session_id).with_max_tokens(20);
                black_box(agent.generate(request3).await.expect("Gen 3 failed"));
            })
        });
    });
}

/// Benchmark: Session creation overhead
fn benchmark_session_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let agent = rt.block_on(async {
        AgentServer::initialize(create_profiling_config())
            .await
            .expect("Failed to initialize agent")
    });

    c.bench_function("session_creation", |b| {
        b.iter(|| {
            rt.block_on(async {
                black_box(
                    agent
                        .create_session()
                        .await
                        .expect("Failed to create session"),
                );
            })
        });
    });
}

/// Benchmark: Message addition overhead
fn benchmark_message_addition(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let agent = rt.block_on(async {
        AgentServer::initialize(create_profiling_config())
            .await
            .expect("Failed to initialize agent")
    });

    // Create session once
    let session_id = rt
        .block_on(async {
            agent
                .create_session()
                .await
                .expect("Failed to create session")
        })
        .id;

    c.bench_function("message_addition", |b| {
        b.iter(|| {
            rt.block_on(async {
                agent
                    .add_message(
                        &session_id,
                        Message {
                            role: MessageRole::User,
                            content: "Test message".to_string(),
                            tool_call_id: None,
                            tool_name: None,
                            timestamp: SystemTime::now(),
                        },
                    )
                    .await
                    .expect("Failed to add message");
            })
        });
    });
}

/// Benchmark: Template rendering (chat template formatting)
fn benchmark_template_rendering(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let agent = rt.block_on(async {
        AgentServer::initialize(create_profiling_config())
            .await
            .expect("Failed to initialize agent")
    });

    c.bench_function("template_rendering_3_messages", |b| {
        b.iter(|| {
            rt.block_on(async {
                let session = agent
                    .create_session()
                    .await
                    .expect("Failed to create session");

                // Add 3 messages to test template rendering with conversation history
                for i in 0..3 {
                    agent
                        .add_message(
                            &session.id,
                            Message {
                                role: MessageRole::User,
                                content: format!("Message {}", i),
                                tool_call_id: None,
                                tool_name: None,
                                timestamp: SystemTime::now(),
                            },
                        )
                        .await
                        .expect("Failed to add message");
                }

                // This will trigger template rendering
                let request = GenerationRequest::new(session.id).with_max_tokens(5);
                black_box(agent.generate(request).await.expect("Generation failed"));
            })
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)  // Reduced for faster profiling
        .measurement_time(std::time::Duration::from_secs(30));
    targets =
        benchmark_single_generation,
        benchmark_session_creation,
        benchmark_message_addition,
        benchmark_three_turn_conversation,
        benchmark_template_rendering
}

criterion_main!(benches);
