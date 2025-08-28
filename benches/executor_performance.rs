//! Performance benchmarks for different executor types
//!
//! These benchmarks measure the performance characteristics of both Claude Code
//! and LlamaAgent executors to ensure they provide acceptable performance for
//! workflow execution.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::collections::HashMap;
use std::time::Duration;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer::workflow::actions::{AgentExecutionContext, AgentExecutorFactory};
use swissarmyhammer::workflow::template_context::WorkflowTemplateContext;
use swissarmyhammer_config::agent::{AgentConfig, LlamaAgentConfig};
use tokio::runtime::Runtime;

/// Benchmark executor initialization time
fn bench_executor_initialization(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("executor_initialization");
    // Allow longer time for initialization since model loading can take time
    group.measurement_time(Duration::from_secs(60));
    group.sample_size(10); // Smaller sample size for expensive operations

    // Claude executor initialization
    group.bench_function("claude_executor_init", |b| {
        b.iter(|| {
            let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
            let context = WorkflowTemplateContext::with_vars(HashMap::new())
                .expect("Failed to create context");
            let mut context_with_config = context;
            context_with_config.set_agent_config(AgentConfig::claude_code());
            let execution_context = AgentExecutionContext::new(&context_with_config);

            // Benchmark the factory creation and initialization
            rt.block_on(async {
                match AgentExecutorFactory::create_executor(&execution_context).await {
                    Ok(executor) => {
                        black_box(executor);
                    }
                    Err(_) => {
                        // Expected in environments without Claude CLI
                        // Don't fail the benchmark, just record the attempt
                    }
                }
            })
        });
    });

    // LlamaAgent executor initialization
    group.bench_function("llama_executor_init", |b| {
        b.iter(|| {
            let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
            let context = WorkflowTemplateContext::with_vars(HashMap::new())
                .expect("Failed to create context");
            let mut context_with_config = context;
            let llama_config = LlamaAgentConfig::for_testing();
            context_with_config.set_agent_config(AgentConfig::llama_agent(llama_config));
            let execution_context = AgentExecutionContext::new(&context_with_config);

            rt.block_on(async {
                match AgentExecutorFactory::create_executor(&execution_context).await {
                    Ok(executor) => {
                        black_box(executor);
                    }
                    Err(_) => {
                        // Expected in environments without model files
                        // Don't fail the benchmark, just record the attempt
                    }
                }
            })
        });
    });

    group.finish();
}

/// Benchmark executor factory performance
fn bench_executor_factory(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("executor_factory");
    group.measurement_time(Duration::from_secs(30));

    // Claude executor factory
    group.bench_function("factory_create_claude", |b| {
        b.iter(|| {
            let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
            let context = WorkflowTemplateContext::with_vars(HashMap::new())
                .expect("Failed to create context");
            let mut context_with_config = context;
            context_with_config.set_agent_config(AgentConfig::claude_code());
            let execution_context = AgentExecutionContext::new(&context_with_config);

            rt.block_on(async {
                match AgentExecutorFactory::create_executor(&execution_context).await {
                    Ok(executor) => {
                        black_box(executor);
                    }
                    Err(_) => {
                        // Expected failure - just continue
                    }
                }
            })
        });
    });

    // LlamaAgent executor factory
    group.bench_function("factory_create_llama", |b| {
        b.iter(|| {
            let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
            let context = WorkflowTemplateContext::with_vars(HashMap::new())
                .expect("Failed to create context");
            let mut context_with_config = context;
            let llama_config = LlamaAgentConfig::for_testing();
            context_with_config.set_agent_config(AgentConfig::llama_agent(llama_config));

            let execution_context = AgentExecutionContext::new(&context_with_config);

            rt.block_on(async {
                match AgentExecutorFactory::create_executor(&execution_context).await {
                    Ok(executor) => {
                        black_box(executor);
                    }
                    Err(_) => {
                        // Expected failure - just continue
                    }
                }
            })
        });
    });

    group.finish();
}

/// Benchmark context creation performance
fn bench_context_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_creation");

    group.bench_function("empty_context", |b| {
        b.iter(|| {
            let context = WorkflowTemplateContext::with_vars(HashMap::new())
                .expect("Failed to create context");
            black_box(context);
        });
    });

    group.bench_function("context_with_vars", |b| {
        let vars = HashMap::from([
            (
                "test_var".to_string(),
                serde_json::Value::String("test_value".to_string()),
            ),
            (
                "number_var".to_string(),
                serde_json::Value::Number(42.into()),
            ),
        ]);

        b.iter(|| {
            let context =
                WorkflowTemplateContext::with_vars(vars.clone()).expect("Failed to create context");
            black_box(context);
        });
    });

    group.bench_function("context_with_config", |b| {
        b.iter(|| {
            let context = WorkflowTemplateContext::with_vars(HashMap::new())
                .expect("Failed to create context");
            let mut context_with_config = context;
            context_with_config.set_agent_config(AgentConfig::claude_code());
            let execution_context = AgentExecutionContext::new(&context_with_config);
            black_box(execution_context);
        });
    });

    group.finish();
}

/// Benchmark configuration operations
fn bench_configuration(c: &mut Criterion) {
    let mut group = c.benchmark_group("configuration");

    group.bench_function("claude_config_creation", |b| {
        b.iter(|| {
            let config = AgentConfig::claude_code();
            black_box(config);
        });
    });

    group.bench_function("llama_config_creation", |b| {
        b.iter(|| {
            let llama_config = LlamaAgentConfig::for_testing();
            let config = AgentConfig::llama_agent(llama_config);
            black_box(config);
        });
    });

    group.bench_function("config_serialization", |b| {
        let config = AgentConfig::claude_code();
        b.iter(|| {
            let serialized = serde_json::to_string(&config).expect("Failed to serialize");
            black_box(serialized);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_executor_initialization,
    bench_executor_factory,
    bench_context_creation,
    bench_configuration
);
criterion_main!(benches);
