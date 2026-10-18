//! Thin wrapper isolating all unsafe ObjC interop with CoreML.

use std::path::Path;
use std::ptr;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::AnyThread;
use objc2_core_ml::{
    MLComputeUnits, MLDictionaryFeatureProvider, MLFeatureProvider, MLFeatureValue, MLModel,
    MLModelConfiguration, MLModelDescription, MLMultiArray, MLMultiArrayDataType,
};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString, NSURL};
use swissarmyhammer_common::command::detach_stdio;

use crate::error::EmbeddingError;

/// Output from a CoreML prediction.
pub(crate) struct PredictionOutput {
    pub embedding: Vec<f32>,
}

/// Thin wrapper around an MLModel loaded from a .mlpackage.
pub(crate) struct CoreMLModel {
    model: Retained<MLModel>,
}

impl CoreMLModel {
    /// Load a CoreML model from a .mlpackage path.
    ///
    /// On first load, compiles the .mlpackage to a `.mlmodelc` directory next to
    /// it and caches the result. Subsequent loads skip compilation entirely by
    /// loading the cached `.mlmodelc` directly. This saves 2-5 seconds per load.
    pub fn load(path: &Path) -> crate::error::Result<Self> {
        unsafe {
            let config = MLModelConfiguration::new();
            config.setComputeUnits(MLComputeUnits::CPUAndNeuralEngine);

            // Derive a cache path: foo.mlpackage → foo.mlmodelc (sibling directory)
            let cache_path = path.with_extension("mlmodelc");

            // If cached compiled model exists and is newer than the source, load directly
            if cache_path.exists() {
                if is_cache_valid(path, &cache_path) {
                    tracing::info!(
                        cache = %cache_path.display(),
                        "Loading cached compiled CoreML model"
                    );
                    let cache_str = NSString::from_str(&cache_path.to_string_lossy());
                    let cache_url = NSURL::fileURLWithPath_isDirectory(&cache_str, true);
                    let model =
                        MLModel::modelWithContentsOfURL_configuration_error(&cache_url, &config)
                            .map_err(|e| {
                                EmbeddingError::coreml(format!("Failed to load cached model: {e}"))
                            })?;
                    return Ok(Self { model });
                }
                // Cache is stale — remove and recompile
                let _ = std::fs::remove_dir_all(&cache_path);
            }

            // Compile .mlpackage → temporary .mlmodelc
            tracing::info!(
                source = %path.display(),
                "Compiling CoreML model (first load, will be cached)"
            );
            let path_str = NSString::from_str(&path.to_string_lossy());
            let source_url = NSURL::fileURLWithPath_isDirectory(&path_str, true);

            #[allow(deprecated)]
            let compiled_url = MLModel::compileModelAtURL_error(&source_url).map_err(|e| {
                EmbeddingError::coreml(format!("Failed to compile .mlpackage: {e}"))
            })?;

            // Copy compiled model to cache location for next time
            let compiled_path_ns = compiled_url
                .path()
                .ok_or_else(|| EmbeddingError::coreml("Compiled URL has no path".to_string()))?;
            let compiled_path_string = compiled_path_ns.to_string();
            let compiled_path = Path::new(compiled_path_string.as_str());
            if compiled_path.exists() && !cache_path.exists() {
                // Use a command to copy the directory tree (fs::copy doesn't handle dirs)
                let status = copy_tree_command(compiled_path, &cache_path).status();
                match status {
                    Ok(s) if s.success() => {
                        tracing::info!(cache = %cache_path.display(), "Cached compiled CoreML model");
                    }
                    _ => {
                        tracing::warn!("Failed to cache compiled model — will recompile next time");
                    }
                }
            }

            let model = MLModel::modelWithContentsOfURL_configuration_error(&compiled_url, &config)
                .map_err(|e| {
                    EmbeddingError::coreml(format!("Failed to load compiled model: {e}"))
                })?;

            Ok(Self { model })
        }
    }

    /// Run prediction and extract the f32 embedding vector.
    pub fn predict_embedding(
        &self,
        input_ids: &[i32],
        attention_mask: &[i32],
        seq_len: usize,
    ) -> crate::error::Result<PredictionOutput> {
        use objc2::rc::autoreleasepool;

        autoreleasepool(|_| unsafe {
            let shape = nsarray_shape(&[1, seq_len]);

            // Create MLMultiArrays with Int32 dtype — the critical bug fix
            let ids_array = make_int32_multiarray(&shape, input_ids)?;
            let mask_array = make_int32_multiarray(&shape, attention_mask)?;

            // Build feature provider
            let ids_fv = MLFeatureValue::featureValueWithMultiArray(&ids_array);
            let mask_fv = MLFeatureValue::featureValueWithMultiArray(&mask_array);

            let key_ids = NSString::from_str("input_ids");
            let key_mask = NSString::from_str("attention_mask");

            // initWithDictionary_error expects NSDictionary<NSString, AnyObject>
            let dict: Retained<NSDictionary<NSString, MLFeatureValue>> =
                NSDictionary::from_retained_objects(&[&*key_ids, &*key_mask], &[ids_fv, mask_fv]);

            let provider = MLDictionaryFeatureProvider::initWithDictionary_error(
                MLDictionaryFeatureProvider::alloc(),
                // Safe: MLFeatureValue is a subclass of NSObject (AnyObject)
                &*((&*dict) as *const NSDictionary<NSString, MLFeatureValue>
                    as *const NSDictionary<NSString, objc2::runtime::AnyObject>),
            )
            .map_err(|e| {
                EmbeddingError::coreml(format!("Failed to create feature provider: {e}"))
            })?;

            let provider = ProtocolObject::from_retained(provider);

            // Run prediction
            let output = self
                .model
                .predictionFromFeatures_error(&provider)
                .map_err(|e| EmbeddingError::coreml(format!("Prediction failed: {e}")))?;

            // Extract embedding output
            let emb_key = NSString::from_str("embedding");
            let emb_fv = output
                .featureValueForName(&emb_key)
                .ok_or_else(|| EmbeddingError::text_processing("No 'embedding' output found"))?;

            let ml_array = emb_fv.multiArrayValue().ok_or_else(|| {
                EmbeddingError::text_processing("'embedding' output is not a multi-array")
            })?;

            let embedding = extract_f32(&ml_array)?;
            Ok(PredictionOutput { embedding })
        })
    }

    /// Probe the embedding dimension from the model description.
    pub fn embedding_dim(&self) -> crate::error::Result<Option<usize>> {
        unsafe {
            let desc: Retained<MLModelDescription> = self.model.modelDescription();
            let outputs = desc.outputDescriptionsByName();
            let emb_key = NSString::from_str("embedding");
            let Some(feat_desc) = outputs.objectForKey(&emb_key) else {
                return Ok(None);
            };
            let Some(constraint) = feat_desc.multiArrayConstraint() else {
                return Ok(None);
            };
            let shape = constraint.shape();
            // Shape is typically [1, dim] — take the last element
            let count = shape.count();
            if count == 0 {
                return Ok(None);
            }
            let last = shape.objectAtIndex(count - 1);
            let dim = last.integerValue() as usize;
            if dim > 0 {
                Ok(Some(dim))
            } else {
                Ok(None)
            }
        }
    }
}

/// Check if a cached .mlmodelc is still valid (newer than the source .mlpackage).
fn is_cache_valid(source: &Path, cache: &Path) -> bool {
    let source_mtime = source.metadata().and_then(|m| m.modified()).ok();
    let cache_mtime = cache.metadata().and_then(|m| m.modified()).ok();
    match (source_mtime, cache_mtime) {
        (Some(s), Some(c)) => c >= s,
        // If we can't read timestamps, assume cache is valid if it exists
        _ => true,
    }
}

/// Build the `cp -R` command that copies the compiled model tree to the cache.
///
/// This command can run in `sah serve`, which speaks MCP on its stdin and
/// stdout. Thus the child gets a null stdin and a null stdout from
/// [`detach_stdio`]: it cannot read MCP frames, and it cannot write text into
/// the MCP channel.
fn copy_tree_command(source: &Path, destination: &Path) -> std::process::Command {
    let mut command = std::process::Command::new("cp");
    detach_stdio(command.arg("-R").arg(source).arg(destination));
    command
}

/// Create an NSArray<NSNumber> representing a shape like [1, seq_len].
fn nsarray_shape(dims: &[usize]) -> Retained<NSArray<NSNumber>> {
    let nums: Vec<Retained<NSNumber>> = dims
        .iter()
        .map(|&d| NSNumber::new_isize(d as isize))
        .collect();
    NSArray::from_retained_slice(&nums)
}

/// Create an MLMultiArray with Int32 data type and copy data into it.
unsafe fn make_int32_multiarray(
    shape: &NSArray<NSNumber>,
    data: &[i32],
) -> crate::error::Result<Retained<MLMultiArray>> {
    let array = MLMultiArray::initWithShape_dataType_error(
        MLMultiArray::alloc(),
        shape,
        MLMultiArrayDataType::Int32,
    )
    .map_err(|e| EmbeddingError::coreml(format!("Failed to create MLMultiArray: {e}")))?;

    // Write data via getMutableBytesWithHandler
    let src = data.as_ptr();
    let len = data.len();
    let block = RcBlock::new(
        move |ptr: std::ptr::NonNull<std::ffi::c_void>,
              _size: isize,
              _strides: std::ptr::NonNull<NSArray<NSNumber>>| {
            ptr::copy_nonoverlapping(src, ptr.as_ptr() as *mut i32, len);
        },
    );
    array.getMutableBytesWithHandler(&block);

    Ok(array)
}

/// Extract f32 values from an MLMultiArray, handling f32 and f16 output types.
unsafe fn extract_f32(array: &MLMultiArray) -> crate::error::Result<Vec<f32>> {
    let dtype = array.dataType();
    let count = array.count() as usize;

    if count == 0 {
        return Ok(vec![]);
    }

    let mut result: Vec<f32> = Vec::with_capacity(count);
    let result_ptr = result.as_mut_ptr();
    let result_len = count;

    if dtype == MLMultiArrayDataType::Float32 {
        let block = RcBlock::new(
            move |ptr: std::ptr::NonNull<std::ffi::c_void>, _size: isize| {
                ptr::copy_nonoverlapping(ptr.as_ptr() as *const f32, result_ptr, result_len);
            },
        );
        array.getBytesWithHandler(&block);
        result.set_len(count);
    } else if dtype == MLMultiArrayDataType::Float16 {
        let block = RcBlock::new(
            move |ptr: std::ptr::NonNull<std::ffi::c_void>, _size: isize| {
                let src = ptr.as_ptr() as *const u16;
                for i in 0..result_len {
                    let bits = *src.add(i);
                    *result_ptr.add(i) = half::f16::from_bits(bits).to_f32();
                }
            },
        );
        array.getBytesWithHandler(&block);
        result.set_len(count);
    } else {
        return Err(EmbeddingError::coreml(format!(
            "Unsupported output dtype: {:?}",
            dtype
        )));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::test_utils::{stdin_is_null, stdout_is_null};

    #[test]
    fn copy_tree_command_does_not_use_the_stdin_or_stdout_of_this_process() {
        let command = copy_tree_command(Path::new("/compiled.mlmodelc"), Path::new("/cache"));

        assert!(stdin_is_null(&command), "stdin is not null: {command:#?}");
        assert!(stdout_is_null(&command), "stdout is not null: {command:#?}");
    }
}
