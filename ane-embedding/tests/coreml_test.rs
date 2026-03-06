//! Test that coreml-rs can load the converted .mlpackage and run inference.
//! Requires the .mlpackage to exist at var/data/models/qwen3-embedding-0.6b/

use coreml_rs::mlarray::MLArray;
use coreml_rs::{ComputePlatform, CoreMLModelOptions, CoreMLModelWithState};
use ndarray::Array2;
use std::path::PathBuf;

fn model_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .unwrap()
        .join("var/data/models/qwen3-embedding-0.6b")
}

fn mlpackage_path() -> PathBuf {
    model_dir().join("Qwen3-Embedding-0.6B-seq128.mlpackage")
}

fn skip_if_no_model() -> Option<PathBuf> {
    let path = mlpackage_path();
    if path.exists() {
        Some(path)
    } else {
        eprintln!("Skipping: .mlpackage not found at {}", path.display());
        None
    }
}

/// Load the tokenizer from the model directory.
fn load_tokenizer() -> tokenizers::Tokenizer {
    let tok_path = model_dir().join("tokenizer.json");
    tokenizers::Tokenizer::from_file(&tok_path)
        .unwrap_or_else(|e| panic!("Failed to load tokenizer at {}: {e}", tok_path.display()))
}

/// Tokenize text and produce i32 input arrays padded to `seq_len`.
fn tokenize_to_i32(
    tokenizer: &tokenizers::Tokenizer,
    text: &str,
    seq_len: usize,
) -> (ndarray::ArrayD<i32>, ndarray::ArrayD<i32>) {
    let encoding = tokenizer.encode(text, true).expect("tokenization failed");
    let input_ids = encoding.get_ids();
    let attention_mask = encoding.get_attention_mask();
    let token_count = input_ids.len().min(seq_len);

    let mut ids = vec![0i32; seq_len];
    let mut mask = vec![0i32; seq_len];
    for i in 0..token_count {
        ids[i] = input_ids[i] as i32;
        mask[i] = attention_mask[i] as i32;
    }

    let ids_array = Array2::from_shape_vec((1, seq_len), ids)
        .unwrap()
        .into_dyn();
    let mask_array = Array2::from_shape_vec((1, seq_len), mask)
        .unwrap()
        .into_dyn();
    (ids_array, mask_array)
}

/// Extract f32 values from an MLArray, handling both f32 and f16 outputs.
fn mlarray_to_f32(array: MLArray) -> Vec<f32> {
    match array {
        MLArray::Float32Array(a) => a.into_raw_vec(),
        MLArray::Float16Array(a) => a.into_raw_vec().iter().map(|v| v.to_f32()).collect(),
        other => panic!("Unexpected output type, shape: {:?}", other.shape()),
    }
}

#[test]
fn test_coreml_load_model() {
    let Some(path) = skip_if_no_model() else {
        return;
    };

    let opts = CoreMLModelOptions {
        compute_platform: ComputePlatform::CpuAndANE,
        ..Default::default()
    };

    let model = CoreMLModelWithState::new(path.to_string_lossy().to_string(), opts);
    let model = model.load().expect("Failed to load .mlpackage");

    let desc = model.description().expect("Failed to get description");
    eprintln!("Model description: {:?}", desc);
}

#[test]
fn test_coreml_inference() {
    let Some(path) = skip_if_no_model() else {
        return;
    };

    let opts = CoreMLModelOptions {
        compute_platform: ComputePlatform::CpuAndANE,
        ..Default::default()
    };

    let model = CoreMLModelWithState::new(path.to_string_lossy().to_string(), opts);
    let mut model = model.load().expect("Failed to load .mlpackage");

    // Use the tokenizer to produce valid inputs, matching what
    // AneEmbeddingModel does in production.
    let tokenizer = load_tokenizer();
    let seq_len = 128;
    let (input_ids, attention_mask) = tokenize_to_i32(&tokenizer, "Hello world", seq_len);

    eprintln!("Input shape: {:?}", input_ids.shape());

    model
        .add_input("input_ids", input_ids)
        .expect("Failed to add input_ids");
    model
        .add_input("attention_mask", attention_mask)
        .expect("Failed to add attention_mask");

    let output = model.predict().expect("Failed to predict");

    eprintln!(
        "Output keys: {:?}",
        output.outputs.keys().collect::<Vec<_>>()
    );

    let embedding_array = output
        .outputs
        .into_iter()
        .find(|(k, _)| k == "embedding")
        .expect("No 'embedding' output found")
        .1;

    let shape = embedding_array.shape().to_vec();
    eprintln!("Embedding shape: {:?}", shape);

    let embedding_f32 = mlarray_to_f32(embedding_array);

    eprintln!("Embedding length: {}", embedding_f32.len());
    eprintln!(
        "First 5 values: {:?}",
        &embedding_f32[..5.min(embedding_f32.len())]
    );

    assert_eq!(
        embedding_f32.len(),
        1024,
        "Expected 1024-dim embedding, got {}",
        embedding_f32.len()
    );

    // Verify finite, non-zero output
    let nan_count = embedding_f32.iter().filter(|v| v.is_nan()).count();
    assert_eq!(nan_count, 0, "Embedding should not contain NaN values");

    let sum: f32 = embedding_f32.iter().sum();
    assert!(sum.abs() > 0.0, "Embedding should not be all zeros");
}
