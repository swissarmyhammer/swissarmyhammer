//! Static ONNX Runtime bindings with CoreML execution provider.
//!
//! This crate provides safe Rust wrappers around ONNX Runtime's C API,
//! statically linked with CoreML support for Apple Neural Engine inference.
//!
//! Instead of replicating the massive OrtApi vtable in Rust, we use a thin
//! C wrapper (`wrapper.c`) that provides stable function signatures.
//!
//! # Example
//!
//! ```rust,no_run
//! use onnxruntime_coreml_sys::*;
//!
//! init().expect("ORT init");
//! let env = Env::new(LoggingLevel::Warning, "example").unwrap();
//! let opts = SessionOptions::new().unwrap();
//! let session = Session::new(&env, "model.onnx", &opts).unwrap();
//! ```

use std::ffi::{c_char, c_void, CStr, CString};
use std::fmt;
use std::ptr;

// --- CoreML flags ---

pub const COREML_FLAG_NONE: u32 = 0x000;
pub const COREML_FLAG_CPU_ONLY: u32 = 0x001;
pub const COREML_FLAG_ENABLE_ON_SUBGRAPH: u32 = 0x002;
pub const COREML_FLAG_ONLY_ANE_DEVICE: u32 = 0x004;
pub const COREML_FLAG_STATIC_INPUT_SHAPES: u32 = 0x008;
pub const COREML_FLAG_CREATE_MLPROGRAM: u32 = 0x010;
pub const COREML_FLAG_CPU_AND_GPU: u32 = 0x020;

// --- Raw C bindings ---

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoggingLevel {
    Verbose = 0,
    Info = 1,
    Warning = 2,
    Error = 3,
    Fatal = 4,
}

#[repr(C)]
struct OrtWrapperError {
    code: i32,
    message: [u8; 512],
}

impl OrtWrapperError {
    fn new() -> Self {
        Self {
            code: 0,
            message: [0u8; 512],
        }
    }

    fn to_error(&self) -> OrtError {
        let msg = CStr::from_bytes_until_nul(&self.message)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "unknown error".to_string());
        OrtError {
            code: self.code as u32,
            message: msg,
        }
    }
}

// Opaque handle types
type OrtWrapperEnv = *mut c_void;
type OrtWrapperSession = *mut c_void;
type OrtWrapperSessionOptions = *mut c_void;
type OrtWrapperValue = *mut c_void;

extern "C" {
    fn ort_wrapper_init() -> i32;

    fn ort_wrapper_create_env(
        level: i32,
        logid: *const c_char,
        out_env: *mut OrtWrapperEnv,
        error: *mut OrtWrapperError,
    ) -> i32;
    fn ort_wrapper_release_env(env: OrtWrapperEnv);

    fn ort_wrapper_create_session_options(
        out_options: *mut OrtWrapperSessionOptions,
        error: *mut OrtWrapperError,
    ) -> i32;
    fn ort_wrapper_release_session_options(options: OrtWrapperSessionOptions);

    fn ort_wrapper_session_options_append_coreml(
        options: OrtWrapperSessionOptions,
        coreml_flags: u32,
        error: *mut OrtWrapperError,
    ) -> i32;

    fn ort_wrapper_session_options_set_intra_op_threads(
        options: OrtWrapperSessionOptions,
        num_threads: i32,
        error: *mut OrtWrapperError,
    ) -> i32;

    fn ort_wrapper_create_session(
        env: OrtWrapperEnv,
        model_path: *const c_char,
        options: OrtWrapperSessionOptions,
        out_session: *mut OrtWrapperSession,
        error: *mut OrtWrapperError,
    ) -> i32;
    fn ort_wrapper_release_session(session: OrtWrapperSession);

    fn ort_wrapper_session_get_input_count(
        session: OrtWrapperSession,
        out_count: *mut usize,
        error: *mut OrtWrapperError,
    ) -> i32;
    fn ort_wrapper_session_get_output_count(
        session: OrtWrapperSession,
        out_count: *mut usize,
        error: *mut OrtWrapperError,
    ) -> i32;

    fn ort_wrapper_session_get_input_name(
        session: OrtWrapperSession,
        index: usize,
        out_name: *mut *mut c_char,
        error: *mut OrtWrapperError,
    ) -> i32;
    fn ort_wrapper_session_get_output_name(
        session: OrtWrapperSession,
        index: usize,
        out_name: *mut *mut c_char,
        error: *mut OrtWrapperError,
    ) -> i32;
    fn ort_wrapper_free_string(str_ptr: *mut c_char);

    fn ort_wrapper_create_tensor_float(
        data: *const f32,
        data_count: usize,
        shape: *const i64,
        shape_len: usize,
        out_value: *mut OrtWrapperValue,
        error: *mut OrtWrapperError,
    ) -> i32;

    fn ort_wrapper_create_tensor_int64(
        data: *const i64,
        data_count: usize,
        shape: *const i64,
        shape_len: usize,
        out_value: *mut OrtWrapperValue,
        error: *mut OrtWrapperError,
    ) -> i32;

    fn ort_wrapper_release_value(value: OrtWrapperValue);

    fn ort_wrapper_get_tensor_float_data(
        value: OrtWrapperValue,
        out_data: *mut *const f32,
        out_count: *mut usize,
        error: *mut OrtWrapperError,
    ) -> i32;

    fn ort_wrapper_get_tensor_shape(
        value: OrtWrapperValue,
        out_dims: *mut i64,
        max_dims: usize,
        out_num_dims: *mut usize,
        error: *mut OrtWrapperError,
    ) -> i32;

    fn ort_wrapper_run(
        session: OrtWrapperSession,
        input_names: *const *const c_char,
        inputs: *const OrtWrapperValue,
        input_count: usize,
        output_names: *const *const c_char,
        output_count: usize,
        outputs: *mut OrtWrapperValue,
        error: *mut OrtWrapperError,
    ) -> i32;

    fn ort_wrapper_version() -> *const c_char;
    fn ort_wrapper_coreml_available() -> i32;
}

// --- Public safe API ---

/// Error from ONNX Runtime operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrtError {
    pub code: u32,
    pub message: String,
}

impl fmt::Display for OrtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ORT error ({}): {}", self.code, self.message)
    }
}

impl std::error::Error for OrtError {}

pub type Result<T> = std::result::Result<T, OrtError>;

/// Initialize the ONNX Runtime. Must be called once before using any other functions.
pub fn init() -> Result<()> {
    let ret = unsafe { ort_wrapper_init() };
    if ret != 0 {
        return Err(OrtError {
            code: 1,
            message: "Failed to initialize ONNX Runtime".to_string(),
        });
    }
    Ok(())
}

/// Get the ONNX Runtime version string.
pub fn version() -> String {
    unsafe {
        let ptr = ort_wrapper_version();
        if ptr.is_null() {
            "unknown".to_string()
        } else {
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }
}

/// Check if CoreML is available on this platform.
pub fn coreml_available() -> bool {
    unsafe { ort_wrapper_coreml_available() != 0 }
}

/// ONNX Runtime environment.
pub struct Env {
    raw: OrtWrapperEnv,
}

impl fmt::Debug for Env {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Env").finish_non_exhaustive()
    }
}

impl Env {
    pub fn new(level: LoggingLevel, name: &str) -> Result<Self> {
        let c_name = CString::new(name).map_err(|_| OrtError {
            code: 2,
            message: "Invalid name string".to_string(),
        })?;
        let mut raw: OrtWrapperEnv = ptr::null_mut();
        let mut err = OrtWrapperError::new();
        let ret = unsafe { ort_wrapper_create_env(level as i32, c_name.as_ptr(), &mut raw, &mut err) };
        if ret != 0 {
            return Err(err.to_error());
        }
        Ok(Self { raw })
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        unsafe { ort_wrapper_release_env(self.raw) }
    }
}

/// Session options builder.
pub struct SessionOptions {
    raw: OrtWrapperSessionOptions,
}

impl fmt::Debug for SessionOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionOptions").finish_non_exhaustive()
    }
}

impl SessionOptions {
    pub fn new() -> Result<Self> {
        let mut raw: OrtWrapperSessionOptions = ptr::null_mut();
        let mut err = OrtWrapperError::new();
        let ret = unsafe { ort_wrapper_create_session_options(&mut raw, &mut err) };
        if ret != 0 {
            return Err(err.to_error());
        }
        Ok(Self { raw })
    }

    /// Append CoreML execution provider with given flags.
    pub fn with_coreml(self, flags: u32) -> Result<Self> {
        let mut err = OrtWrapperError::new();
        let ret = unsafe { ort_wrapper_session_options_append_coreml(self.raw, flags, &mut err) };
        if ret != 0 {
            return Err(err.to_error());
        }
        Ok(self)
    }

    /// Set intra-op thread count.
    pub fn with_intra_op_threads(self, num_threads: i32) -> Result<Self> {
        let mut err = OrtWrapperError::new();
        let ret = unsafe {
            ort_wrapper_session_options_set_intra_op_threads(self.raw, num_threads, &mut err)
        };
        if ret != 0 {
            return Err(err.to_error());
        }
        Ok(self)
    }
}

impl Drop for SessionOptions {
    fn drop(&mut self) {
        unsafe { ort_wrapper_release_session_options(self.raw) }
    }
}

/// Helper to release a raw session on error paths.
unsafe fn release_session_raw(raw: OrtWrapperSession) {
    ort_wrapper_release_session(raw);
}

/// An ONNX Runtime inference session.
pub struct Session {
    raw: OrtWrapperSession,
    input_names: Vec<String>,
    output_names: Vec<String>,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("inputs", &self.input_names)
            .field("outputs", &self.output_names)
            .finish()
    }
}

impl Session {
    /// Create a session from a model file.
    pub fn new(env: &Env, model_path: &str, options: &SessionOptions) -> Result<Self> {
        let c_path = CString::new(model_path).map_err(|_| OrtError {
            code: 2,
            message: "Invalid model path".to_string(),
        })?;
        let mut raw: OrtWrapperSession = ptr::null_mut();
        let mut err = OrtWrapperError::new();
        let ret = unsafe {
            ort_wrapper_create_session(env.raw, c_path.as_ptr(), options.raw, &mut raw, &mut err)
        };
        if ret != 0 {
            return Err(err.to_error());
        }

        // Get input names — check every return code (B2 fix)
        let mut input_count: usize = 0;
        let ret = unsafe {
            ort_wrapper_session_get_input_count(raw, &mut input_count, &mut err)
        };
        if ret != 0 {
            unsafe { release_session_raw(raw) };
            return Err(err.to_error());
        }

        let mut input_names = Vec::with_capacity(input_count);
        for i in 0..input_count {
            let mut name_ptr: *mut c_char = ptr::null_mut();
            let ret = unsafe {
                ort_wrapper_session_get_input_name(raw, i, &mut name_ptr, &mut err)
            };
            if ret != 0 {
                unsafe { release_session_raw(raw) };
                return Err(err.to_error());
            }
            if name_ptr.is_null() {
                unsafe { release_session_raw(raw) };
                return Err(OrtError {
                    code: 1,
                    message: format!("Input name {} returned null", i),
                });
            }
            let name = unsafe { CStr::from_ptr(name_ptr).to_string_lossy().into_owned() };
            unsafe { ort_wrapper_free_string(name_ptr) };
            input_names.push(name);
        }

        // Get output names
        let mut output_count: usize = 0;
        let ret = unsafe {
            ort_wrapper_session_get_output_count(raw, &mut output_count, &mut err)
        };
        if ret != 0 {
            unsafe { release_session_raw(raw) };
            return Err(err.to_error());
        }

        let mut output_names = Vec::with_capacity(output_count);
        for i in 0..output_count {
            let mut name_ptr: *mut c_char = ptr::null_mut();
            let ret = unsafe {
                ort_wrapper_session_get_output_name(raw, i, &mut name_ptr, &mut err)
            };
            if ret != 0 {
                unsafe { release_session_raw(raw) };
                return Err(err.to_error());
            }
            if name_ptr.is_null() {
                unsafe { release_session_raw(raw) };
                return Err(OrtError {
                    code: 1,
                    message: format!("Output name {} returned null", i),
                });
            }
            let name = unsafe { CStr::from_ptr(name_ptr).to_string_lossy().into_owned() };
            unsafe { ort_wrapper_free_string(name_ptr) };
            output_names.push(name);
        }

        Ok(Self {
            raw,
            input_names,
            output_names,
        })
    }

    /// Get input names.
    pub fn input_names(&self) -> &[String] {
        &self.input_names
    }

    /// Get output names.
    pub fn output_names(&self) -> &[String] {
        &self.output_names
    }

    /// Run inference with the given input tensors.
    /// Returns output tensors in the order of `output_names()`.
    pub fn run(&self, inputs: &[&Tensor]) -> Result<Vec<Tensor>> {
        // Nit 1 fix: use map_err instead of unwrap on CString::new
        let input_name_cstrings: Vec<CString> = self
            .input_names
            .iter()
            .map(|n| CString::new(n.as_str()))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| OrtError {
                code: 2,
                message: "Input name contains null byte".to_string(),
            })?;
        let input_name_ptrs: Vec<*const c_char> =
            input_name_cstrings.iter().map(|cs| cs.as_ptr()).collect();

        let output_name_cstrings: Vec<CString> = self
            .output_names
            .iter()
            .map(|n| CString::new(n.as_str()))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| OrtError {
                code: 2,
                message: "Output name contains null byte".to_string(),
            })?;
        let output_name_ptrs: Vec<*const c_char> =
            output_name_cstrings.iter().map(|cs| cs.as_ptr()).collect();

        let input_values: Vec<OrtWrapperValue> = inputs.iter().map(|t| t.raw).collect();
        let mut output_values: Vec<OrtWrapperValue> =
            vec![ptr::null_mut(); self.output_names.len()];

        let mut err = OrtWrapperError::new();
        let ret = unsafe {
            ort_wrapper_run(
                self.raw,
                input_name_ptrs.as_ptr(),
                input_values.as_ptr(),
                input_values.len(),
                output_name_ptrs.as_ptr(),
                output_name_ptrs.len(),
                output_values.as_mut_ptr(),
                &mut err,
            )
        };
        if ret != 0 {
            return Err(err.to_error());
        }

        let tensors = output_values
            .into_iter()
            .map(|raw| Tensor {
                raw,
                owned: true,
                _data: TensorData::Unowned,
            })
            .collect();
        Ok(tensors)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { ort_wrapper_release_session(self.raw) }
    }
}

/// Owned data backing a tensor, ensuring the buffer outlives the ORT value.
///
/// Variants hold owned data to keep it alive for the duration of the ORT tensor.
/// The data is never read back — it exists solely for lifetime management.
#[allow(dead_code)]
enum TensorData {
    Float(Vec<f32>),
    Int64(Vec<i64>),
    /// Output tensors — data is owned by ORT, freed when the OrtValue is released.
    Unowned,
}

/// A tensor value (input or output).
///
/// Input tensors own their data buffer (stored in `_data`) to ensure the
/// pointer passed to `CreateTensorWithDataAsOrtValue` remains valid for
/// the lifetime of the tensor. ORT does NOT copy the data.
pub struct Tensor {
    raw: OrtWrapperValue,
    owned: bool,
    _data: TensorData,
}

impl fmt::Debug for Tensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tensor")
            .field("owned", &self.owned)
            .finish_non_exhaustive()
    }
}

impl Tensor {
    /// Create a float tensor from data and shape.
    ///
    /// The data is copied into an owned buffer held by this Tensor,
    /// ensuring the pointer passed to ORT remains valid.
    pub fn from_f32(data: &[f32], shape: &[i64]) -> Result<Self> {
        let owned_data = data.to_vec();
        let mut raw: OrtWrapperValue = ptr::null_mut();
        let mut err = OrtWrapperError::new();
        let ret = unsafe {
            ort_wrapper_create_tensor_float(
                owned_data.as_ptr(),
                owned_data.len(),
                shape.as_ptr(),
                shape.len(),
                &mut raw,
                &mut err,
            )
        };
        if ret != 0 {
            return Err(err.to_error());
        }
        Ok(Self {
            raw,
            owned: true,
            _data: TensorData::Float(owned_data),
        })
    }

    /// Create an i64 tensor from data and shape.
    ///
    /// The data is copied into an owned buffer held by this Tensor,
    /// ensuring the pointer passed to ORT remains valid.
    pub fn from_i64(data: &[i64], shape: &[i64]) -> Result<Self> {
        let owned_data = data.to_vec();
        let mut raw: OrtWrapperValue = ptr::null_mut();
        let mut err = OrtWrapperError::new();
        let ret = unsafe {
            ort_wrapper_create_tensor_int64(
                owned_data.as_ptr(),
                owned_data.len(),
                shape.as_ptr(),
                shape.len(),
                &mut raw,
                &mut err,
            )
        };
        if ret != 0 {
            return Err(err.to_error());
        }
        Ok(Self {
            raw,
            owned: true,
            _data: TensorData::Int64(owned_data),
        })
    }

    /// Get float data from the tensor.
    pub fn as_f32_slice(&self) -> Result<&[f32]> {
        let mut data_ptr: *const f32 = ptr::null();
        let mut count: usize = 0;
        let mut err = OrtWrapperError::new();
        let ret = unsafe {
            ort_wrapper_get_tensor_float_data(self.raw, &mut data_ptr, &mut count, &mut err)
        };
        if ret != 0 {
            return Err(err.to_error());
        }
        Ok(unsafe { std::slice::from_raw_parts(data_ptr, count) })
    }

    /// Get the tensor shape.
    pub fn shape(&self) -> Result<Vec<i64>> {
        let mut dims = [0i64; 8];
        let mut num_dims: usize = 0;
        let mut err = OrtWrapperError::new();
        let ret = unsafe {
            ort_wrapper_get_tensor_shape(self.raw, dims.as_mut_ptr(), 8, &mut num_dims, &mut err)
        };
        if ret != 0 {
            return Err(err.to_error());
        }
        Ok(dims[..num_dims].to_vec())
    }
}

impl Drop for Tensor {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ort_wrapper_release_value(self.raw) }
        }
    }
}

// SAFETY: ORT values are thread-safe per ORT documentation.
// Sessions support concurrent Run() calls; tensors are immutable after creation.
unsafe impl Send for Env {}
unsafe impl Sync for Env {}
unsafe impl Send for Session {}
unsafe impl Sync for Session {}
unsafe impl Send for Tensor {}
unsafe impl Sync for Tensor {}
unsafe impl Send for SessionOptions {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        init().expect("Failed to initialize ORT");
    }

    #[test]
    fn test_version() {
        init().unwrap();
        let v = version();
        assert!(!v.is_empty());
        assert_ne!(v, "unknown");
    }

    #[test]
    fn test_coreml_available() {
        let available = coreml_available();
        if cfg!(target_os = "macos") {
            assert!(available);
        }
    }

    #[test]
    fn test_create_env() {
        init().unwrap();
        let _env = Env::new(LoggingLevel::Warning, "test").unwrap();
    }

    #[test]
    fn test_session_options() {
        init().unwrap();
        let _opts = SessionOptions::new().unwrap();
    }

    #[test]
    fn test_session_options_with_coreml() {
        init().unwrap();
        let opts = SessionOptions::new().unwrap();
        let result = opts.with_coreml(
            COREML_FLAG_CREATE_MLPROGRAM | COREML_FLAG_STATIC_INPUT_SHAPES,
        );
        if cfg!(target_os = "macos") {
            assert!(result.is_ok(), "CoreML should be available: {:?}", result.err());
        }
    }

    #[test]
    fn test_create_tensor_f32() {
        init().unwrap();
        let data = vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        let shape = vec![2i64, 3];
        let tensor = Tensor::from_f32(&data, &shape).unwrap();

        let retrieved = tensor.as_f32_slice().unwrap();
        assert_eq!(retrieved, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);

        let tensor_shape = tensor.shape().unwrap();
        assert_eq!(tensor_shape, vec![2, 3]);
    }

    #[test]
    fn test_tensor_owns_data() {
        // B1 regression test: tensor must remain valid after input data is dropped
        init().unwrap();
        let tensor = {
            let data = vec![42.0f32, 43.0, 44.0];
            let shape = vec![1i64, 3];
            Tensor::from_f32(&data, &shape).unwrap()
            // data dropped here
        };
        // Tensor should still be readable because it owns a copy
        let retrieved = tensor.as_f32_slice().unwrap();
        assert_eq!(retrieved, &[42.0, 43.0, 44.0]);
    }

    #[test]
    fn test_create_tensor_i64() {
        init().unwrap();
        let data = vec![1i64, 2, 3];
        let shape = vec![1i64, 3];
        let _tensor = Tensor::from_i64(&data, &shape).unwrap();
    }

    #[test]
    fn test_coreml_flags() {
        assert_eq!(COREML_FLAG_NONE, 0);
        assert_eq!(COREML_FLAG_CREATE_MLPROGRAM, 0x010);
        assert_eq!(
            COREML_FLAG_CREATE_MLPROGRAM | COREML_FLAG_STATIC_INPUT_SHAPES,
            0x018
        );
    }

    #[test]
    fn test_ort_error_traits() {
        let err = OrtError { code: 1, message: "test".to_string() };
        let err2 = err.clone();
        assert_eq!(err, err2);
        assert_eq!(format!("{:?}", err), "OrtError { code: 1, message: \"test\" }");
    }

    #[test]
    fn test_debug_impls() {
        init().unwrap();
        let env = Env::new(LoggingLevel::Warning, "debug-test").unwrap();
        assert!(format!("{:?}", env).contains("Env"));

        let opts = SessionOptions::new().unwrap();
        assert!(format!("{:?}", opts).contains("SessionOptions"));
    }
}
