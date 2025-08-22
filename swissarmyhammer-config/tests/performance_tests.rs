//! Performance integration tests for SwissArmyHammer configuration system

mod common;

use common::{TestEnvironment, ConfigScope};
use serial_test::serial;
use std::time::{Duration, Instant};
use swissarmyhammer_config::{ConfigFormat, TemplateRenderer};

const PERFORMANCE_ITERATIONS: usize = 100;
const MAX_LOAD_TIME_MS: u128 = 50;
const MAX_RENDER_TIME_MS: u128 = 10;

#[test]
#[serial]
fn test_configuration_loading_performance_baseline() {
    let env = TestEnvironment::new().unwrap();

    // Create a typical-sized configuration
    let config = TestEnvironment::create_sample_toml_config();
    env.write_project_config(&config, ConfigFormat::Toml)
        .unwrap();

    // Warm up
    for _ in 0..10 {
        let _ = env.load_template_context().unwrap();
    }

    // Measure performance
    let start = Instant::now();
    for _ in 0..PERFORMANCE_ITERATIONS {
        let _ = env.load_template_context().unwrap();
    }
    let duration = start.elapsed();
    let avg_duration = duration / PERFORMANCE_ITERATIONS as u32;

    println!(
        "Baseline configuration loading: {}ms average over {} iterations",
        avg_duration.as_millis(),
        PERFORMANCE_ITERATIONS
    );

    assert!(
        avg_duration.as_millis() < MAX_LOAD_TIME_MS,
        "Configuration loading took {}ms on average, expected < {}ms",
        avg_duration.as_millis(),
        MAX_LOAD_TIME_MS
    );
}

#[test]
#[serial]
fn test_large_configuration_loading_performance() {
    let env = TestEnvironment::new().unwrap();

    // Create a large configuration with many sections and keys
    let mut large_config = String::from(
        r#"
project_name = "Performance Test"
environment = "test"
version = "1.0.0"
debug = false
"#,
    );

    // Add many sections to simulate a large enterprise configuration
    for section_idx in 0..20 {
        large_config.push_str(&format!("[section_{}]\n", section_idx));
        for key_idx in 0..25 {
            large_config.push_str(&format!(
                "key_{} = \"value_{}_{}\"\n",
                key_idx, section_idx, key_idx
            ));
        }
        large_config.push('\n');
    }

    // Add nested configurations
    for service_idx in 0..15 {
        large_config.push_str(&format!(
            r#"
[services.service_{}]
name = "service_{}"
port = {}
enabled = true
replicas = {}

[services.service_{}.database]
host = "service-{}-db.example.com"
port = 5432
name = "service_{}db"

[services.service_{}.monitoring]
metrics_port = {}
health_check = "/health"
enabled = true
"#,
            service_idx,
            service_idx,
            8000 + service_idx,
            3,
            service_idx,
            service_idx,
            service_idx,
            service_idx,
            9000 + service_idx
        ));
    }

    env.write_project_config(&large_config, ConfigFormat::Toml)
        .unwrap();

    // Measure loading performance for large configuration
    let start = Instant::now();
    for _ in 0..50 {
        // Fewer iterations for large config
        let _context = env.load_template_context().unwrap();
    }
    let duration = start.elapsed();
    let avg_duration = duration / 50;

    println!(
        "Large configuration (~{} lines) loading: {}ms average over 50 iterations",
        large_config.lines().count(),
        avg_duration.as_millis()
    );

    // Large configuration should still load reasonably fast
    assert!(
        avg_duration.as_millis() < MAX_LOAD_TIME_MS * 5, // Allow more time for large config
        "Large configuration loading took {}ms on average, expected < {}ms",
        avg_duration.as_millis(),
        MAX_LOAD_TIME_MS * 5
    );
}

#[test]
#[serial]
fn test_multi_format_loading_performance() {
    let env = TestEnvironment::new().unwrap();

    // Create configurations in all formats
    let toml_config = TestEnvironment::create_sample_toml_config();
    let yaml_config = TestEnvironment::create_sample_yaml_config();
    let json_config = TestEnvironment::create_sample_json_config();

    env.write_project_config(&toml_config, ConfigFormat::Toml)
        .unwrap();
    env.write_project_config(&yaml_config, ConfigFormat::Yaml)
        .unwrap();
    env.write_project_config(&json_config, ConfigFormat::Json)
        .unwrap();

    // Measure performance with multiple formats
    let start = Instant::now();
    for _ in 0..PERFORMANCE_ITERATIONS {
        let _context = env.load_template_context().unwrap();
    }
    let duration = start.elapsed();
    let avg_duration = duration / PERFORMANCE_ITERATIONS as u32;

    println!(
        "Multi-format configuration loading: {}ms average over {} iterations",
        avg_duration.as_millis(),
        PERFORMANCE_ITERATIONS
    );

    // Multi-format should not significantly impact performance
    assert!(
        avg_duration.as_millis() < MAX_LOAD_TIME_MS * 2, // Allow double time for multi-format
        "Multi-format configuration loading took {}ms on average, expected < {}ms",
        avg_duration.as_millis(),
        MAX_LOAD_TIME_MS * 2
    );
}

#[test]
#[serial]
fn test_environment_variable_override_performance() {
    let mut env = TestEnvironment::new().unwrap();

    // Create base configuration
    env.write_project_config(&TestEnvironment::create_complex_nested_config(), ConfigFormat::Toml)
        .unwrap();

    // Set many environment variables
    let env_vars: Vec<(String, String)> = (0..50)
        .map(|i| (format!("SAH_PERF_VAR_{}", i), format!("value_{}", i)))
        .collect();

    for (key, value) in &env_vars {
        env.set_env_var(key, value).unwrap();
    }

    // Add some nested environment variables
    env.set_env_vars([
        ("SAH_SERVER__PORT", "9999"),
        ("SAH_DATABASE__POOL__MAX_CONNECTIONS", "100"),
        ("SAH_FEATURES__EXPERIMENTAL__NEW_UI", "true"),
        ("SAH_MONITORING__TRACING__SAMPLE_RATE", "0.5"),
    ])
    .unwrap();

    // Measure performance with many environment overrides
    let start = Instant::now();
    for _ in 0..PERFORMANCE_ITERATIONS {
        let _context = env.load_template_context().unwrap();
    }
    let duration = start.elapsed();
    let avg_duration = duration / PERFORMANCE_ITERATIONS as u32;

    println!(
        "Configuration loading with {} env vars: {}ms average",
        env_vars.len() + 4,
        avg_duration.as_millis()
    );

    // Environment variables should not significantly slow down loading
    assert!(
        avg_duration.as_millis() < MAX_LOAD_TIME_MS * 2,
        "Configuration with env vars took {}ms on average, expected < {}ms",
        avg_duration.as_millis(),
        MAX_LOAD_TIME_MS * 2
    );
}

#[test]
#[serial]
fn test_environment_substitution_performance() {
    let mut env = TestEnvironment::new().unwrap();

    // Set up environment variables for substitution
    env.set_env_vars([
        ("VAR_1", "value_1"),
        ("VAR_2", "value_2"),
        ("VAR_3", "value_3"),
        ("VAR_4", "value_4"),
        ("VAR_5", "value_5"),
        ("NESTED_VAR_1", "nested_1"),
        ("NESTED_VAR_2", "nested_2"),
        ("DB_HOST", "performance-db"),
        ("DB_PORT", "5432"),
        ("API_KEY", "perf_test_key"),
    ])
    .unwrap();

    // Create configuration with many environment substitutions
    let mut config = String::from(
        r#"
project_name = "${VAR_1}_project"
environment = "${VAR_2}_env"
version = "1.0.0"

[database]
host = "${DB_HOST}"
port = "${DB_PORT}"
connection_string = "postgresql://${DB_USER:-user}:${DB_PASS:-pass}@${DB_HOST}:${DB_PORT}/db"

[api]
key = "${API_KEY}"
endpoint = "https://${VAR_3}.example.com/${VAR_4}/api"
"#,
    );

    // Add more substitutions
    for i in 1..=20 {
        config.push_str(&format!(
            "substitution_{} = \"${{VAR_{}}}_suffix\"\n",
            i,
            (i % 5) + 1
        ));
    }

    env.write_project_config(&config, ConfigFormat::Toml)
        .unwrap();

    // Measure environment substitution performance
    let start = Instant::now();
    for _ in 0..PERFORMANCE_ITERATIONS {
        let _context = env.load_template_context().unwrap();
    }
    let duration = start.elapsed();
    let avg_duration = duration / PERFORMANCE_ITERATIONS as u32;

    println!(
        "Configuration with env substitution: {}ms average over {} iterations",
        avg_duration.as_millis(),
        PERFORMANCE_ITERATIONS
    );

    // Environment substitution should not significantly impact performance
    assert!(
        avg_duration.as_millis() < MAX_LOAD_TIME_MS * 3,
        "Configuration with substitution took {}ms on average, expected < {}ms",
        avg_duration.as_millis(),
        MAX_LOAD_TIME_MS * 3
    );
}

#[test]
#[serial]
fn test_template_rendering_performance() {
    let mut env = TestEnvironment::new().unwrap();

    // Set up configuration for template rendering
    env.write_project_config(&TestEnvironment::create_sample_toml_config(), ConfigFormat::Toml)
        .unwrap();

    env.set_env_vars([
        ("SAH_RENDER_VAR", "performance_test"),
        ("SAH_BUILD_ID", "12345"),
    ])
    .unwrap();

    let _provider = env.create_provider();
    let renderer = TemplateRenderer::new().unwrap();

    // Test different template complexities
    let simple_template = "Hello {{ project_name }} v{{ version }}!";
    let complex_template = r#"
Project: {{ project_name }}
Environment: {{ environment }}
Database: {{ database.host }}:{{ database.port }}/{{ database.name }}
Debug: {% if debug %}enabled{% else %}disabled{% endif %}
Services: {% for service in services %}{{ service.name }}:{{ service.port }} {% endfor %}
"#;

    // Measure simple template rendering performance
    let start = Instant::now();
    for _ in 0..PERFORMANCE_ITERATIONS * 2 {
        let _result = renderer.render_with_config(simple_template, None).unwrap();
    }
    let simple_duration = start.elapsed();
    let simple_avg = simple_duration / (PERFORMANCE_ITERATIONS * 2) as u32;

    // Measure complex template rendering performance
    let start = Instant::now();
    for _ in 0..PERFORMANCE_ITERATIONS {
        let _result = renderer.render_with_config(complex_template.trim(), None).unwrap();
    }
    let complex_duration = start.elapsed();
    let complex_avg = complex_duration / PERFORMANCE_ITERATIONS as u32;

    println!(
        "Simple template rendering: {}ms average over {} iterations",
        simple_avg.as_millis(),
        PERFORMANCE_ITERATIONS * 2
    );
    println!(
        "Complex template rendering: {}ms average over {} iterations",
        complex_avg.as_millis(),
        PERFORMANCE_ITERATIONS
    );

    assert!(
        simple_avg.as_millis() < MAX_RENDER_TIME_MS,
        "Simple template rendering took {}ms on average, expected < {}ms",
        simple_avg.as_millis(),
        MAX_RENDER_TIME_MS
    );

    assert!(
        complex_avg.as_millis() < MAX_RENDER_TIME_MS * 5,
        "Complex template rendering took {}ms on average, expected < {}ms",
        complex_avg.as_millis(),
        MAX_RENDER_TIME_MS * 5
    );
}

#[test]
#[serial]
fn test_concurrent_configuration_loading_performance() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let env = Arc::new(TestEnvironment::new().unwrap());

    // Create a moderately complex configuration
    env.write_project_config(&TestEnvironment::create_complex_nested_config(), ConfigFormat::Toml)
        .unwrap();

    const THREAD_COUNT: usize = 4;
    const ITERATIONS_PER_THREAD: usize = 25;

    let barrier = Arc::new(Barrier::new(THREAD_COUNT));
    let mut handles = Vec::new();

    let start = Instant::now();

    // Spawn multiple threads to simulate concurrent loading
    for thread_id in 0..THREAD_COUNT {
        let env_clone = Arc::clone(&env);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            // Wait for all threads to be ready
            barrier_clone.wait();

            // Each thread loads configuration multiple times
            for _ in 0..ITERATIONS_PER_THREAD {
                let _context = env_clone.load_template_context().unwrap();
            }

            thread_id
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    let total_duration = start.elapsed();
    let total_iterations = THREAD_COUNT * ITERATIONS_PER_THREAD;
    let avg_duration = total_duration / total_iterations as u32;

    println!(
        "Concurrent configuration loading ({} threads, {} iterations each): {}ms total, {}ms average per load",
        THREAD_COUNT,
        ITERATIONS_PER_THREAD,
        total_duration.as_millis(),
        avg_duration.as_millis()
    );

    // Concurrent access should not significantly degrade performance
    assert!(
        avg_duration.as_millis() < MAX_LOAD_TIME_MS * 2,
        "Concurrent configuration loading took {}ms on average, expected < {}ms",
        avg_duration.as_millis(),
        MAX_LOAD_TIME_MS * 2
    );
}

#[test]
#[serial]
fn test_memory_usage_performance() {
    let env = TestEnvironment::new().unwrap();

    // Create a configuration with many repetitive sections
    let mut memory_test_config = String::new();
    
    for i in 0..100 {
        memory_test_config.push_str(&format!(
            r#"
[memory_test_{}]
id = {}
name = "test_item_{}"
description = "This is a test item for memory usage testing with index {}"
enabled = {}
priority = {}
tags = ["tag1", "tag2", "tag3", "tag4", "tag5"]
metadata = {{ created_at = "2024-01-01T00:00:00Z", version = "1.0.0" }}
"#,
            i, i, i, i, i % 2 == 0, i % 10
        ));
    }

    env.write_project_config(&memory_test_config, ConfigFormat::Toml)
        .unwrap();

    // Load configuration multiple times and measure consistency
    let mut load_times = Vec::new();
    
    for _ in 0..20 {
        let start = Instant::now();
        let _context = env.load_template_context().unwrap();
        load_times.push(start.elapsed());
    }

    // Calculate statistics
    let avg_time = load_times.iter().sum::<Duration>() / load_times.len() as u32;
    let min_time = load_times.iter().min().unwrap();
    let max_time = load_times.iter().max().unwrap();

    println!(
        "Memory test configuration loading: min={}ms, avg={}ms, max={}ms over {} iterations",
        min_time.as_millis(),
        avg_time.as_millis(),
        max_time.as_millis(),
        load_times.len()
    );

    // Performance should be consistent (max should not be much larger than min)
    let variation_factor = max_time.as_millis() as f64 / min_time.as_millis() as f64;
    assert!(
        variation_factor < 3.0,
        "Performance variation too high: {}x difference between min and max",
        variation_factor
    );

    // Average should still be within acceptable range
    assert!(
        avg_time.as_millis() < MAX_LOAD_TIME_MS * 4,
        "Memory test configuration took {}ms on average, expected < {}ms",
        avg_time.as_millis(),
        MAX_LOAD_TIME_MS * 4
    );
}

#[test]
#[serial]
fn test_configuration_file_discovery_performance() {
    let env = TestEnvironment::new().unwrap();

    // Create multiple configuration files that need to be discovered
    let configs = [
        ("sah.toml", ConfigFormat::Toml),
        ("sah.yaml", ConfigFormat::Yaml),
        ("sah.json", ConfigFormat::Json),
        ("swissarmyhammer.toml", ConfigFormat::Toml),
        ("swissarmyhammer.yaml", ConfigFormat::Yaml),
        ("swissarmyhammer.json", ConfigFormat::Json),
    ];

    for (name_part, format) in &configs {
        let config_content = match format {
            ConfigFormat::Toml => format!("test_key_{} = \"toml_value\"", name_part.replace('.', "_")),
            ConfigFormat::Yaml => format!("test_key_{}: yaml_value", name_part.replace('.', "_")),
            ConfigFormat::Json => format!("{{\"test_key_{}\": \"json_value\"}}", name_part.replace('.', "_")),
        };

        env.write_config(&config_content, *format, ConfigScope::Project, &name_part.split('.').next().unwrap())
            .unwrap();
    }

    // Also create global configs
    for (name_part, format) in &configs {
        let config_content = match format {
            ConfigFormat::Toml => format!("global_key_{} = \"toml_global\"", name_part.replace('.', "_")),
            ConfigFormat::Yaml => format!("global_key_{}: yaml_global", name_part.replace('.', "_")),
            ConfigFormat::Json => format!("{{\"global_key_{}\": \"json_global\"}}", name_part.replace('.', "_")),
        };

        env.write_config(&config_content, *format, ConfigScope::Global, &name_part.split('.').next().unwrap())
            .unwrap();
    }

    // Measure file discovery performance
    let start = Instant::now();
    for _ in 0..PERFORMANCE_ITERATIONS {
        let _context = env.load_template_context().unwrap();
    }
    let duration = start.elapsed();
    let avg_duration = duration / PERFORMANCE_ITERATIONS as u32;

    println!(
        "Configuration file discovery ({} files): {}ms average over {} iterations",
        configs.len() * 2, // project + global
        avg_duration.as_millis(),
        PERFORMANCE_ITERATIONS
    );

    // File discovery should not significantly impact performance
    assert!(
        avg_duration.as_millis() < MAX_LOAD_TIME_MS * 3,
        "Configuration discovery took {}ms on average, expected < {}ms",
        avg_duration.as_millis(),
        MAX_LOAD_TIME_MS * 3
    );
}