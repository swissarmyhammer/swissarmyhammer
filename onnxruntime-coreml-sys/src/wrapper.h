// Thin C wrapper around ONNX Runtime C API.
// This avoids needing to replicate the massive OrtApi vtable in Rust.
// Instead, each function we need gets a simple C wrapper that looks up the
// function pointer from the API and calls it.

#ifndef ORT_WRAPPER_H
#define ORT_WRAPPER_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// --- Status codes (prefixed to avoid collision with onnxruntime_c_api.h) ---
typedef enum {
    ORT_WRAPPER_OK = 0,
    ORT_WRAPPER_FAIL = 1,
    ORT_WRAPPER_INVALID_ARGUMENT = 2,
    ORT_WRAPPER_NO_SUCHFILE = 3,
    ORT_WRAPPER_NO_MODEL = 4,
    ORT_WRAPPER_ENGINE_ERROR = 5,
    ORT_WRAPPER_RUNTIME_EXCEPTION = 6,
    ORT_WRAPPER_INVALID_PROTOBUF = 7,
    ORT_WRAPPER_MODEL_LOADED = 8,
    ORT_WRAPPER_NOT_IMPLEMENTED = 9,
    ORT_WRAPPER_INVALID_GRAPH = 10,
    ORT_WRAPPER_EP_FAIL = 11,
} OrtWrapperStatusCode;

// --- Opaque handles ---
typedef void* OrtWrapperEnv;
typedef void* OrtWrapperSession;
typedef void* OrtWrapperSessionOptions;
typedef void* OrtWrapperValue;
typedef void* OrtWrapperMemoryInfo;

// --- Logging levels ---
typedef enum {
    ORT_WRAPPER_LOG_VERBOSE = 0,
    ORT_WRAPPER_LOG_INFO = 1,
    ORT_WRAPPER_LOG_WARNING = 2,
    ORT_WRAPPER_LOG_ERROR = 3,
    ORT_WRAPPER_LOG_FATAL = 4,
} OrtWrapperLoggingLevel;

// --- Tensor element types ---
typedef enum {
    ORT_WRAPPER_TENSOR_FLOAT = 1,
    ORT_WRAPPER_TENSOR_INT32 = 6,
    ORT_WRAPPER_TENSOR_INT64 = 7,
} OrtWrapperTensorElementType;

// --- CoreML flags ---
#define ORT_WRAPPER_COREML_FLAG_NONE                    0x000
#define ORT_WRAPPER_COREML_FLAG_CPU_ONLY                0x001
#define ORT_WRAPPER_COREML_FLAG_ENABLE_ON_SUBGRAPH      0x002
#define ORT_WRAPPER_COREML_FLAG_ONLY_ANE_DEVICE         0x004
#define ORT_WRAPPER_COREML_FLAG_STATIC_INPUT_SHAPES     0x008
#define ORT_WRAPPER_COREML_FLAG_CREATE_MLPROGRAM        0x010
#define ORT_WRAPPER_COREML_FLAG_CPU_AND_GPU             0x020

// --- Error info ---
typedef struct {
    OrtWrapperStatusCode code;
    char message[512];
} OrtWrapperError;

// --- Initialization ---

// Initialize ORT. Must be called before any other function.
// Returns 0 on success.
int ort_wrapper_init(void);

// --- Environment ---

int ort_wrapper_create_env(
    OrtWrapperLoggingLevel level,
    const char* logid,
    OrtWrapperEnv* out_env,
    OrtWrapperError* error
);

void ort_wrapper_release_env(OrtWrapperEnv env);

// --- Session options ---

int ort_wrapper_create_session_options(
    OrtWrapperSessionOptions* out_options,
    OrtWrapperError* error
);

void ort_wrapper_release_session_options(OrtWrapperSessionOptions options);

// Append CoreML execution provider
int ort_wrapper_session_options_append_coreml(
    OrtWrapperSessionOptions options,
    uint32_t coreml_flags,
    OrtWrapperError* error
);

// Set intra-op thread count
int ort_wrapper_session_options_set_intra_op_threads(
    OrtWrapperSessionOptions options,
    int num_threads,
    OrtWrapperError* error
);

// --- Session ---

int ort_wrapper_create_session(
    OrtWrapperEnv env,
    const char* model_path,
    OrtWrapperSessionOptions options,
    OrtWrapperSession* out_session,
    OrtWrapperError* error
);

void ort_wrapper_release_session(OrtWrapperSession session);

// Get input/output counts
int ort_wrapper_session_get_input_count(
    OrtWrapperSession session,
    size_t* out_count,
    OrtWrapperError* error
);

int ort_wrapper_session_get_output_count(
    OrtWrapperSession session,
    size_t* out_count,
    OrtWrapperError* error
);

// Get input/output names. Caller must free returned string with ort_wrapper_free_string.
int ort_wrapper_session_get_input_name(
    OrtWrapperSession session,
    size_t index,
    char** out_name,
    OrtWrapperError* error
);

int ort_wrapper_session_get_output_name(
    OrtWrapperSession session,
    size_t index,
    char** out_name,
    OrtWrapperError* error
);

void ort_wrapper_free_string(char* str);

// --- Tensors ---

int ort_wrapper_create_tensor_float(
    const float* data,
    size_t data_count,
    const int64_t* shape,
    size_t shape_len,
    OrtWrapperValue* out_value,
    OrtWrapperError* error
);

int ort_wrapper_create_tensor_int64(
    const int64_t* data,
    size_t data_count,
    const int64_t* shape,
    size_t shape_len,
    OrtWrapperValue* out_value,
    OrtWrapperError* error
);

void ort_wrapper_release_value(OrtWrapperValue value);

// Get float data from output tensor
int ort_wrapper_get_tensor_float_data(
    OrtWrapperValue value,
    const float** out_data,
    size_t* out_count,
    OrtWrapperError* error
);

// Get tensor shape
int ort_wrapper_get_tensor_shape(
    OrtWrapperValue value,
    int64_t* out_dims,
    size_t max_dims,
    size_t* out_num_dims,
    OrtWrapperError* error
);

// --- Inference ---

int ort_wrapper_run(
    OrtWrapperSession session,
    const char* const* input_names,
    const OrtWrapperValue* inputs,
    size_t input_count,
    const char* const* output_names,
    size_t output_count,
    OrtWrapperValue* outputs,
    OrtWrapperError* error
);

// --- Utility ---

// Get ORT version string
const char* ort_wrapper_version(void);

// Check if CoreML is available
int ort_wrapper_coreml_available(void);

#ifdef __cplusplus
}
#endif

#endif // ORT_WRAPPER_H
