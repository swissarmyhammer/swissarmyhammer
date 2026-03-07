//! Smoke test: load a real ONNX model and run inference with CoreML EP.

use onnxruntime_coreml_sys::*;
use std::path::Path;

const MODEL_PATH: &str = "tests/fixtures/all-MiniLM-L6-v2/onnx/model.onnx";

fn model_available() -> bool {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(MODEL_PATH)
        .exists()
}

#[test]
fn test_load_model_with_coreml() {
    if !model_available() {
        eprintln!("Skipping: model not found at {}", MODEL_PATH);
        return;
    }

    init().expect("ORT init failed");

    let env = Env::new(LoggingLevel::Warning, "smoke-test").expect("env creation failed");

    // Create session with CoreML EP
    let opts = SessionOptions::new()
        .expect("session options failed")
        .with_coreml(COREML_FLAG_CREATE_MLPROGRAM | COREML_FLAG_STATIC_INPUT_SHAPES)
        .expect("CoreML append failed");

    let model_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(MODEL_PATH)
        .to_string_lossy()
        .to_string();

    let session = Session::new(&env, &model_path, &opts).expect("session creation failed");

    // Verify inputs/outputs
    let inputs = session.input_names();
    let outputs = session.output_names();

    println!("Model inputs: {:?}", inputs);
    println!("Model outputs: {:?}", outputs);

    assert!(!inputs.is_empty(), "Model should have inputs");
    assert!(!outputs.is_empty(), "Model should have outputs");

    // all-MiniLM-L6-v2 expects: input_ids, attention_mask, token_type_ids
    // and outputs: last_hidden_state (or sentence_embedding)
    println!(
        "ORT version: {}, CoreML available: {}",
        version(),
        coreml_available()
    );
}

#[test]
fn test_run_inference() {
    if !model_available() {
        eprintln!("Skipping: model not found at {}", MODEL_PATH);
        return;
    }

    init().expect("ORT init failed");

    let env = Env::new(LoggingLevel::Warning, "inference-test").expect("env creation failed");

    let opts = SessionOptions::new()
        .expect("session options failed")
        .with_coreml(COREML_FLAG_CREATE_MLPROGRAM | COREML_FLAG_STATIC_INPUT_SHAPES)
        .expect("CoreML append failed");

    let model_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(MODEL_PATH)
        .to_string_lossy()
        .to_string();

    let session = Session::new(&env, &model_path, &opts).expect("session creation failed");

    // Create dummy input tensors
    // all-MiniLM-L6-v2 expects:
    //   input_ids: int64 [batch, seq_len]
    //   attention_mask: int64 [batch, seq_len]
    //   token_type_ids: int64 [batch, seq_len]
    let seq_len = 8i64;
    let batch = 1i64;
    let shape = vec![batch, seq_len];

    // Dummy token IDs (CLS=101, tokens, SEP=102, PAD=0)
    let input_ids: Vec<i64> = vec![101, 7592, 2088, 102, 0, 0, 0, 0]; // "hello world"
    let attention_mask: Vec<i64> = vec![1, 1, 1, 1, 0, 0, 0, 0];
    let token_type_ids: Vec<i64> = vec![0, 0, 0, 0, 0, 0, 0, 0];

    let input_ids_tensor = Tensor::from_i64(&input_ids, &shape).expect("input_ids tensor");
    let attention_mask_tensor =
        Tensor::from_i64(&attention_mask, &shape).expect("attention_mask tensor");
    let token_type_ids_tensor =
        Tensor::from_i64(&token_type_ids, &shape).expect("token_type_ids tensor");

    // Run inference
    let outputs = session
        .run(&[
            &input_ids_tensor,
            &attention_mask_tensor,
            &token_type_ids_tensor,
        ])
        .expect("inference failed");

    assert!(!outputs.is_empty(), "Should have output tensors");

    // Get the first output (last_hidden_state or sentence_embedding)
    let output = &outputs[0];
    let output_shape = output.shape().expect("output shape");
    let output_data = output.as_f32_slice().expect("output data");

    println!("Output shape: {:?}", output_shape);
    println!("Output element count: {}", output_data.len());
    println!(
        "First 5 values: {:?}",
        &output_data[..5.min(output_data.len())]
    );

    // Verify we got reasonable output
    assert!(!output_data.is_empty(), "Output should have data");

    // Verify values are not all zeros (model actually computed something)
    let sum: f32 = output_data.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "Output values should not all be zero");

    println!(
        "Smoke test passed! Model produced {} output values",
        output_data.len()
    );
}
