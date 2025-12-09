use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use mermaid_parser::parse_diagram;
use std::fs;

fn load_sample_files() -> Vec<(String, String)> {
    let mut samples = Vec::new();

    // Load flowchart samples (576 total)
    if let Ok(entries) = fs::read_dir("test/flowchart") {
        for entry in entries.take(10).flatten() {
            // Take first 10 for individual benchmarks
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("mermaid") {
                if let Ok(content) = fs::read_to_string(&path) {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    samples.push((format!("flowchart_{}", name), content));
                }
            }
        }
    }

    // Load other diagram type samples
    for diagram_type in &["sankey", "architecture", "block", "c4", "class"] {
        let dir_path = format!("test/{}", diagram_type);
        if let Ok(entries) = fs::read_dir(&dir_path) {
            for entry in entries.take(2).flatten() {
                // Take 2 samples per type
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("mermaid") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        let name = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        samples.push((format!("{}_{}", diagram_type, name), content));
                    }
                }
            }
        }
    }

    samples
}

fn load_all_flowchart_samples() -> Vec<String> {
    let mut samples = Vec::new();

    if let Ok(entries) = fs::read_dir("test/flowchart") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("mermaid") {
                if let Ok(content) = fs::read_to_string(&path) {
                    samples.push(content);
                }
            }
        }
    }

    samples
}

fn benchmark_individual_parsers(c: &mut Criterion) {
    let samples = load_sample_files();

    // Simple diagram samples for quick benchmarks
    let simple_diagrams = vec![
        ("flowchart_simple", "flowchart TD\n    A --> B"),
        ("sankey_simple", "sankey-beta\n    A,B,10"),
        (
            "timeline_simple",
            "timeline\n    title My Timeline\n    2021 : Event 1",
        ),
        ("sequence_simple", "sequenceDiagram\n    Alice->Bob: Hello"),
        ("class_simple", "classDiagram\n    class Animal"),
        ("state_simple", "stateDiagram\n    [*] --> Still"),
    ];

    let mut group = c.benchmark_group("individual_parsers");

    // Benchmark simple diagrams
    for (name, content) in simple_diagrams {
        group.bench_with_input(BenchmarkId::new("simple", name), &content, |b, content| {
            b.iter(|| parse_diagram(black_box(content)))
        });
    }

    // Benchmark loaded file samples
    for (name, content) in &samples {
        group.bench_with_input(
            BenchmarkId::new("file_samples", name),
            content,
            |b, content| b.iter(|| parse_diagram(black_box(content))),
        );
    }

    group.finish();
}

fn benchmark_detection_overhead(c: &mut Criterion) {
    let samples = vec![
        ("flowchart", "flowchart TD\n    A --> B\n    B --> C\n    C --> D"),
        ("sankey", "sankey-beta\n    A,B,10\n    B,C,5\n    C,D,3"),
        ("timeline", "timeline\n    title Development Timeline\n    2021 : Planning\n    2022 : Development"),
        ("sequence", "sequenceDiagram\n    participant A\n    participant B\n    A->>B: Message 1\n    B->>A: Response"),
    ];

    let mut group = c.benchmark_group("detection_overhead");

    for (diagram_type, content) in samples {
        group.bench_with_input(
            BenchmarkId::new("auto_detect", diagram_type),
            &content,
            |b, content| b.iter(|| parse_diagram(black_box(content))),
        );
    }

    group.finish();
}

fn benchmark_batch_parsing(c: &mut Criterion) {
    let all_flowcharts = load_all_flowchart_samples();

    let mut group = c.benchmark_group("batch_parsing");

    // Benchmark different batch sizes
    for &batch_size in &[10, 50, 100, 500] {
        if batch_size <= all_flowcharts.len() {
            let batch = &all_flowcharts[..batch_size];
            group.bench_with_input(
                BenchmarkId::new("flowchart_batch", batch_size),
                batch,
                |b, batch| {
                    b.iter(|| {
                        for diagram in batch {
                            let _ = parse_diagram(black_box(diagram));
                        }
                    })
                },
            );
        }
    }

    // The key benchmark: parsing 1000+ diagrams
    if all_flowcharts.len() >= 576 {
        group.bench_function("1000_plus_diagrams", |b| {
            // Repeat flowchart samples to get over 1000
            let mut large_batch = all_flowcharts.clone();
            while large_batch.len() < 1000 {
                large_batch.extend_from_slice(&all_flowcharts);
            }
            let batch_1000 = &large_batch[..1000];

            b.iter(|| {
                for diagram in batch_1000 {
                    let _ = parse_diagram(black_box(diagram));
                }
            })
        });
    }

    group.finish();
}

fn benchmark_large_diagrams(c: &mut Criterion) {
    // Create increasingly complex diagrams to test scaling
    let small_flowchart = "flowchart TD\n    A --> B";

    let medium_flowchart = r#"
flowchart TD
    A --> B
    B --> C
    B --> D
    C --> E
    D --> E
    E --> F
    A --> G
    G --> H
    H --> I
    I --> J
"#;

    let large_flowchart = format!(
        "flowchart TD\n{}",
        (0..100)
            .map(|i| format!("    N{} --> N{}", i, i + 1))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let mut group = c.benchmark_group("diagram_sizes");

    group.bench_function("small_diagram", |b| {
        b.iter(|| parse_diagram(black_box(small_flowchart)))
    });

    group.bench_function("medium_diagram", |b| {
        b.iter(|| parse_diagram(black_box(medium_flowchart)))
    });

    group.bench_function("large_diagram", |b| {
        b.iter(|| parse_diagram(black_box(&large_flowchart)))
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_individual_parsers,
    benchmark_detection_overhead,
    benchmark_batch_parsing,
    benchmark_large_diagrams
);
criterion_main!(benches);
