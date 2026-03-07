//! Test that objc2-core-ml can load the converted .mlpackage and run inference.
//! Requires the .mlpackage to exist at var/data/models/qwen3-embedding-0.6b/

use std::path::PathBuf;
use std::ptr;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::AnyThread;
use objc2_core_ml::{
    MLComputeUnits, MLDictionaryFeatureProvider, MLFeatureProvider, MLFeatureValue, MLModel,
    MLModelConfiguration, MLMultiArray, MLMultiArrayDataType,
};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString, NSURL};

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

/// Load a CoreML model from a .mlpackage path using objc2-core-ml directly.
/// Compiles the .mlpackage first, then loads the compiled model.
unsafe fn load_model(path: &std::path::Path) -> Retained<MLModel> {
    let path_str = NSString::from_str(&path.to_string_lossy());
    let source_url = NSURL::fileURLWithPath_isDirectory(&path_str, true);

    #[allow(deprecated)]
    let compiled_url =
        MLModel::compileModelAtURL_error(&source_url).expect("Failed to compile .mlpackage");

    let config = MLModelConfiguration::new();
    config.setComputeUnits(MLComputeUnits::CPUAndNeuralEngine);
    MLModel::modelWithContentsOfURL_configuration_error(&compiled_url, &config)
        .expect("Failed to load compiled model")
}

/// Create an Int32 MLMultiArray with the given shape and data.
unsafe fn make_int32_array(shape: &[usize], data: &[i32]) -> Retained<MLMultiArray> {
    let ns_shape: Vec<Retained<NSNumber>> = shape
        .iter()
        .map(|&d| NSNumber::new_isize(d as isize))
        .collect();
    let ns_shape = NSArray::from_retained_slice(&ns_shape);

    let array = MLMultiArray::initWithShape_dataType_error(
        MLMultiArray::alloc(),
        &ns_shape,
        MLMultiArrayDataType::Int32,
    )
    .expect("Failed to create MLMultiArray");

    let src = data.as_ptr();
    let len = data.len();
    let block = RcBlock::new(
        move |ptr: ptr::NonNull<std::ffi::c_void>,
              _size: isize,
              _strides: ptr::NonNull<NSArray<NSNumber>>| {
            ptr::copy_nonoverlapping(src, ptr.as_ptr() as *mut i32, len);
        },
    );
    array.getMutableBytesWithHandler(&block);

    array
}

/// Tokenize text and produce padded i32 vectors.
fn tokenize_to_i32(
    tokenizer: &tokenizers::Tokenizer,
    text: &str,
    seq_len: usize,
) -> (Vec<i32>, Vec<i32>) {
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
    (ids, mask)
}

/// Extract f32 values from an MLMultiArray.
unsafe fn extract_f32(array: &MLMultiArray) -> Vec<f32> {
    let dtype = array.dataType();
    let count = array.count() as usize;
    let mut result: Vec<f32> = Vec::with_capacity(count);
    let result_ptr = result.as_mut_ptr();

    if dtype == MLMultiArrayDataType::Float32 {
        let block = RcBlock::new(move |ptr: ptr::NonNull<std::ffi::c_void>, _size: isize| {
            ptr::copy_nonoverlapping(ptr.as_ptr() as *const f32, result_ptr, count);
        });
        array.getBytesWithHandler(&block);
        result.set_len(count);
    } else if dtype == MLMultiArrayDataType::Float16 {
        let block = RcBlock::new(move |ptr: ptr::NonNull<std::ffi::c_void>, _size: isize| {
            let src = ptr.as_ptr() as *const u16;
            for i in 0..count {
                *result_ptr.add(i) = half::f16::from_bits(*src.add(i)).to_f32();
            }
        });
        array.getBytesWithHandler(&block);
        result.set_len(count);
    } else {
        panic!("Unsupported output dtype: {:?}", dtype);
    }

    result
}

#[test]
fn test_coreml_load_model() {
    let Some(path) = skip_if_no_model() else {
        return;
    };

    unsafe {
        let model = load_model(&path);
        let desc = model.modelDescription();
        let outputs = desc.outputDescriptionsByName();
        eprintln!("Output keys: {:?}", outputs.allKeys());
    }
}

#[test]
fn test_coreml_inference() {
    let Some(path) = skip_if_no_model() else {
        return;
    };

    unsafe {
        let model = load_model(&path);

        let tokenizer = load_tokenizer();
        let seq_len = 128;
        let (ids, mask) = tokenize_to_i32(&tokenizer, "Hello world", seq_len);

        let ids_array = make_int32_array(&[1, seq_len], &ids);
        let mask_array = make_int32_array(&[1, seq_len], &mask);

        let ids_fv = MLFeatureValue::featureValueWithMultiArray(&ids_array);
        let mask_fv = MLFeatureValue::featureValueWithMultiArray(&mask_array);

        let key_ids = NSString::from_str("input_ids");
        let key_mask = NSString::from_str("attention_mask");

        let dict: Retained<NSDictionary<NSString, MLFeatureValue>> =
            NSDictionary::from_retained_objects(&[&*key_ids, &*key_mask], &[ids_fv, mask_fv]);

        let provider = MLDictionaryFeatureProvider::initWithDictionary_error(
            MLDictionaryFeatureProvider::alloc(),
            &*((&*dict) as *const NSDictionary<NSString, MLFeatureValue>
                as *const NSDictionary<NSString, objc2::runtime::AnyObject>),
        )
        .expect("Failed to create feature provider");

        let provider = ProtocolObject::from_retained(provider);

        let output = model
            .predictionFromFeatures_error(&provider)
            .expect("Prediction failed");

        let emb_key = NSString::from_str("embedding");
        let emb_fv = output
            .featureValueForName(&emb_key)
            .expect("No 'embedding' output");
        let ml_array = emb_fv
            .multiArrayValue()
            .expect("'embedding' is not multi-array");

        let embedding_f32 = extract_f32(&ml_array);

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

        let nan_count = embedding_f32.iter().filter(|v| v.is_nan()).count();
        assert_eq!(nan_count, 0, "Embedding should not contain NaN values");

        let sum: f32 = embedding_f32.iter().sum();
        assert!(sum.abs() > 0.0, "Embedding should not be all zeros");
    }
}
