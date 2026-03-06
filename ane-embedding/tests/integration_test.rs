use ane_embedding::{AneEmbeddingConfig, AneEmbeddingModel};
use model_embedding::{BatchProcessor, TextEmbedder};
use serial_test::serial;
use std::path::PathBuf;

fn mlpackage_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .unwrap()
        .join("var/data/models/qwen3-embedding-0.6b")
}

fn skip_if_no_model() -> Option<AneEmbeddingConfig> {
    let dir = mlpackage_dir();
    let config = AneEmbeddingConfig {
        model_dir: dir,
        ..AneEmbeddingConfig::default()
    };
    if !config.model_path().exists() {
        eprintln!(
            "Skipping: .mlpackage not found at {}",
            config.model_path().display()
        );
        return None;
    }
    Some(config)
}

#[tokio::test]
#[serial]
async fn test_load_model() {
    let Some(config) = skip_if_no_model() else {
        return;
    };
    let model = AneEmbeddingModel::new(config);

    assert!(!model.is_loaded());

    model.load().await.expect("Failed to load model");

    assert!(model.is_loaded());
    assert_eq!(
        model.embedding_dimension(),
        Some(1024),
        "Qwen3-Embedding-0.6B should produce 1024-dimensional embeddings"
    );
}

#[tokio::test]
#[serial]
async fn test_embed_single_text() {
    let Some(config) = skip_if_no_model() else {
        return;
    };
    let model = AneEmbeddingModel::new(config);
    model.load().await.expect("Failed to load model");

    let result = model
        .embed_text("Hello world")
        .await
        .expect("Failed to generate embedding");

    assert_eq!(result.embedding().len(), 1024);
    assert_eq!(result.text(), "Hello world");
    assert!(!result.text_hash().is_empty());
    assert!(result.processing_time_ms() > 0);
    assert!(result.sequence_length() > 0);
}

#[tokio::test]
#[serial]
async fn test_embed_consistency() {
    let Some(config) = skip_if_no_model() else {
        return;
    };
    let model = AneEmbeddingModel::new(config);
    model.load().await.expect("Failed to load model");

    let r1 = model.embed_text("test sentence").await.unwrap();
    let r2 = model.embed_text("test sentence").await.unwrap();

    // Same text should produce same hash
    assert_eq!(r1.text_hash(), r2.text_hash());

    // Same text should produce same embedding
    assert_eq!(r1.embedding(), r2.embedding());
}

#[tokio::test]
#[serial]
async fn test_embed_different_texts_differ() {
    let Some(config) = skip_if_no_model() else {
        return;
    };
    let model = AneEmbeddingModel::new(config);
    model.load().await.expect("Failed to load model");

    let r1 = model.embed_text("The cat sat on the mat").await.unwrap();
    let r2 = model
        .embed_text("Quantum computing uses qubits")
        .await
        .unwrap();

    // Different texts should produce different embeddings
    assert_ne!(r1.embedding(), r2.embedding());
    assert_ne!(r1.text_hash(), r2.text_hash());
}

#[tokio::test]
#[serial]
async fn test_normalization() {
    let Some(config) = skip_if_no_model() else {
        return;
    };
    let config = AneEmbeddingConfig {
        normalize_embeddings: true,
        ..config
    };
    let model = AneEmbeddingModel::new(config);
    model.load().await.expect("Failed to load model");

    let result = model.embed_text("Normalization test").await.unwrap();

    let magnitude: f32 = result
        .embedding()
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        .sqrt();
    assert!(
        (magnitude - 1.0).abs() < 1e-3,
        "Normalized embedding should have magnitude ~1.0, got: {}",
        magnitude
    );
}

#[tokio::test]
#[serial]
async fn test_trait_object_usage() {
    let Some(config) = skip_if_no_model() else {
        return;
    };
    let model = AneEmbeddingModel::new(config);
    model.load().await.expect("Failed to load model");

    let embedder: &dyn TextEmbedder = &model;
    assert!(embedder.is_loaded());
    assert_eq!(embedder.embedding_dimension(), Some(1024));

    let result = embedder.embed_text("Trait object test").await.unwrap();
    assert_eq!(result.embedding().len(), 1024);
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

#[tokio::test]
#[serial]
async fn test_cosine_similarity() {
    let Some(config) = skip_if_no_model() else {
        return;
    };
    let model = AneEmbeddingModel::new(config);
    model.load().await.expect("Failed to load model");

    let r_dog = model.embed_text("The dog ran across the park").await.unwrap();
    let r_puppy = model.embed_text("A puppy sprinted through the garden").await.unwrap();
    let r_quantum = model.embed_text("Quantum entanglement violates Bell inequalities").await.unwrap();

    let sim_similar = cosine_similarity(r_dog.embedding(), r_puppy.embedding());
    let sim_different = cosine_similarity(r_dog.embedding(), r_quantum.embedding());

    eprintln!("Similar texts cosine: {sim_similar:.4}");
    eprintln!("Different texts cosine: {sim_different:.4}");

    assert!(
        sim_similar > sim_different,
        "Similar texts ({sim_similar:.4}) should have higher similarity than dissimilar ({sim_different:.4})"
    );
}

#[tokio::test]
#[serial]
async fn test_batch_processing() {
    let Some(config) = skip_if_no_model() else {
        return;
    };
    let model = AneEmbeddingModel::new(config);
    model.load().await.expect("Failed to load model");

    let texts = vec![
        "First sentence".to_string(),
        "Second sentence".to_string(),
        "Third sentence".to_string(),
    ];

    let mut processor = BatchProcessor::new(&model, 2);
    let results = processor.process_texts(texts).await.expect("Batch failed");

    assert_eq!(results.len(), 3);
    for result in &results {
        assert_eq!(result.embedding().len(), 1024);
    }

    let stats = processor.stats();
    assert_eq!(stats.total_texts, 3);
    assert_eq!(stats.failed_embeddings, 0);
}
