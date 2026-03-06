//! Test that coreml-rs can load the converted .mlpackage and run inference.
//! Requires the .mlpackage to exist at var/data/models/qwen3-embedding-0.6b/

use coreml_rs::mlarray::MLArray;
use coreml_rs::{ComputePlatform, CoreMLModelOptions, CoreMLModelWithState};
use ndarray::Array2;
use std::path::PathBuf;

fn mlpackage_path() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .unwrap()
        .join("var/data/models/qwen3-embedding-0.6b/Qwen3-Embedding-0.6B.mlpackage")
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

    let seq_len = 512;
    // Use f32 inputs: coreml-rs 0.5.4 has a bug in bindInputI32 where it
    // tags i32 data as float32, corrupting the values. CoreML coerces f32→i32.
    let input_ids = Array2::<f32>::ones((1, seq_len)).into_dyn();
    let attention_mask = Array2::<f32>::ones((1, seq_len)).into_dyn();

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

    // Verify non-zero output
    let sum: f32 = embedding_f32.iter().sum();
    assert!(sum.abs() > 0.0, "Embedding should not be all zeros");
}
