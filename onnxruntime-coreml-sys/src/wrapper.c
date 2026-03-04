// Thin C wrapper around ONNX Runtime C API.
// Provides stable function signatures so Rust doesn't need to replicate
// the OrtApi vtable layout.

#include "wrapper.h"
#include "onnxruntime_c_api.h"

#ifdef __APPLE__
#include "coreml_provider_factory.h"
#endif

#include <string.h>
#include <stdlib.h>
#include <stdio.h>

// Global API pointer, initialized once
static const OrtApi* g_api = NULL;

static void set_error(OrtWrapperError* error, OrtWrapperStatusCode code, const char* msg) {
    if (error) {
        error->code = code;
        strncpy(error->message, msg, sizeof(error->message) - 1);
        error->message[sizeof(error->message) - 1] = '\0';
    }
}

static int check_status(OrtStatus* status, OrtWrapperError* error) {
    if (status == NULL) {
        return 0; // success
    }

    OrtErrorCode code = g_api->GetErrorCode(status);
    const char* msg = g_api->GetErrorMessage(status);

    if (error) {
        error->code = (OrtWrapperStatusCode)code;
        strncpy(error->message, msg, sizeof(error->message) - 1);
        error->message[sizeof(error->message) - 1] = '\0';
    }

    g_api->ReleaseStatus(status);
    return -1; // failure
}

// --- Initialization ---

int ort_wrapper_init(void) {
    if (g_api != NULL) {
        return 0; // already initialized
    }

    const OrtApiBase* base = OrtGetApiBase();
    if (base == NULL) {
        return -1;
    }

    g_api = base->GetApi(ORT_API_VERSION);
    if (g_api == NULL) {
        return -1;
    }

    return 0;
}

// --- Environment ---

int ort_wrapper_create_env(
    OrtWrapperLoggingLevel level,
    const char* logid,
    OrtWrapperEnv* out_env,
    OrtWrapperError* error
) {
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    OrtEnv* env = NULL;
    OrtStatus* status = g_api->CreateEnv((OrtLoggingLevel)level, logid, &env);
    if (check_status(status, error) != 0) return -1;

    *out_env = (OrtWrapperEnv)env;
    return 0;
}

void ort_wrapper_release_env(OrtWrapperEnv env) {
    if (g_api && env) {
        g_api->ReleaseEnv((OrtEnv*)env);
    }
}

// --- Session options ---

int ort_wrapper_create_session_options(
    OrtWrapperSessionOptions* out_options,
    OrtWrapperError* error
) {
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    OrtSessionOptions* opts = NULL;
    OrtStatus* status = g_api->CreateSessionOptions(&opts);
    if (check_status(status, error) != 0) return -1;

    *out_options = (OrtWrapperSessionOptions)opts;
    return 0;
}

void ort_wrapper_release_session_options(OrtWrapperSessionOptions options) {
    if (g_api && options) {
        g_api->ReleaseSessionOptions((OrtSessionOptions*)options);
    }
}

int ort_wrapper_session_options_append_coreml(
    OrtWrapperSessionOptions options,
    uint32_t coreml_flags,
    OrtWrapperError* error
) {
#ifdef __APPLE__
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    OrtStatus* status = OrtSessionOptionsAppendExecutionProvider_CoreML(
        (OrtSessionOptions*)options, coreml_flags
    );
    return check_status(status, error);
#else
    set_error(error, ORT_WRAPPER_NOT_IMPLEMENTED, "CoreML is only available on Apple platforms");
    return -1;
#endif
}

int ort_wrapper_session_options_set_intra_op_threads(
    OrtWrapperSessionOptions options,
    int num_threads,
    OrtWrapperError* error
) {
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    OrtStatus* status = g_api->SetIntraOpNumThreads((OrtSessionOptions*)options, num_threads);
    return check_status(status, error);
}

// --- Session ---

int ort_wrapper_create_session(
    OrtWrapperEnv env,
    const char* model_path,
    OrtWrapperSessionOptions options,
    OrtWrapperSession* out_session,
    OrtWrapperError* error
) {
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    OrtSession* session = NULL;
    OrtStatus* status = g_api->CreateSession(
        (OrtEnv*)env, model_path, (OrtSessionOptions*)options, &session
    );
    if (check_status(status, error) != 0) return -1;

    *out_session = (OrtWrapperSession)session;
    return 0;
}

void ort_wrapper_release_session(OrtWrapperSession session) {
    if (g_api && session) {
        g_api->ReleaseSession((OrtSession*)session);
    }
}

int ort_wrapper_session_get_input_count(
    OrtWrapperSession session,
    size_t* out_count,
    OrtWrapperError* error
) {
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    OrtStatus* status = g_api->SessionGetInputCount((OrtSession*)session, out_count);
    return check_status(status, error);
}

int ort_wrapper_session_get_output_count(
    OrtWrapperSession session,
    size_t* out_count,
    OrtWrapperError* error
) {
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    OrtStatus* status = g_api->SessionGetOutputCount((OrtSession*)session, out_count);
    return check_status(status, error);
}

int ort_wrapper_session_get_input_name(
    OrtWrapperSession session,
    size_t index,
    char** out_name,
    OrtWrapperError* error
) {
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    OrtAllocator* allocator = NULL;
    OrtStatus* status = g_api->GetAllocatorWithDefaultOptions(&allocator);
    if (check_status(status, error) != 0) return -1;

    char* name = NULL;
    status = g_api->SessionGetInputName((OrtSession*)session, index, allocator, &name);
    if (check_status(status, error) != 0) return -1;

    // Copy to our own allocation so caller can use free()
    *out_name = strdup(name);
    status = g_api->AllocatorFree(allocator, name);
    // Ignore free errors

    return 0;
}

int ort_wrapper_session_get_output_name(
    OrtWrapperSession session,
    size_t index,
    char** out_name,
    OrtWrapperError* error
) {
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    OrtAllocator* allocator = NULL;
    OrtStatus* status = g_api->GetAllocatorWithDefaultOptions(&allocator);
    if (check_status(status, error) != 0) return -1;

    char* name = NULL;
    status = g_api->SessionGetOutputName((OrtSession*)session, index, allocator, &name);
    if (check_status(status, error) != 0) return -1;

    *out_name = strdup(name);
    status = g_api->AllocatorFree(allocator, name);

    return 0;
}

void ort_wrapper_free_string(char* str) {
    free(str);
}

// --- Tensors ---

int ort_wrapper_create_tensor_float(
    const float* data,
    size_t data_count,
    const int64_t* shape,
    size_t shape_len,
    OrtWrapperValue* out_value,
    OrtWrapperError* error
) {
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    OrtMemoryInfo* mem_info = NULL;
    OrtStatus* status = g_api->CreateCpuMemoryInfo(
        OrtArenaAllocator, OrtMemTypeDefault, &mem_info
    );
    if (check_status(status, error) != 0) return -1;

    OrtValue* value = NULL;
    status = g_api->CreateTensorWithDataAsOrtValue(
        mem_info,
        (void*)data,
        data_count * sizeof(float),
        shape,
        shape_len,
        ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT,
        &value
    );
    g_api->ReleaseMemoryInfo(mem_info);

    if (check_status(status, error) != 0) return -1;

    *out_value = (OrtWrapperValue)value;
    return 0;
}

int ort_wrapper_create_tensor_int64(
    const int64_t* data,
    size_t data_count,
    const int64_t* shape,
    size_t shape_len,
    OrtWrapperValue* out_value,
    OrtWrapperError* error
) {
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    OrtMemoryInfo* mem_info = NULL;
    OrtStatus* status = g_api->CreateCpuMemoryInfo(
        OrtArenaAllocator, OrtMemTypeDefault, &mem_info
    );
    if (check_status(status, error) != 0) return -1;

    OrtValue* value = NULL;
    status = g_api->CreateTensorWithDataAsOrtValue(
        mem_info,
        (void*)data,
        data_count * sizeof(int64_t),
        shape,
        shape_len,
        ONNX_TENSOR_ELEMENT_DATA_TYPE_INT64,
        &value
    );
    g_api->ReleaseMemoryInfo(mem_info);

    if (check_status(status, error) != 0) return -1;

    *out_value = (OrtWrapperValue)value;
    return 0;
}

void ort_wrapper_release_value(OrtWrapperValue value) {
    if (g_api && value) {
        g_api->ReleaseValue((OrtValue*)value);
    }
}

int ort_wrapper_get_tensor_float_data(
    OrtWrapperValue value,
    const float** out_data,
    size_t* out_count,
    OrtWrapperError* error
) {
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    float* data = NULL;
    OrtStatus* status = g_api->GetTensorMutableData((OrtValue*)value, (void**)&data);
    if (check_status(status, error) != 0) return -1;

    // Get element count from shape
    OrtTensorTypeAndShapeInfo* info = NULL;
    status = g_api->GetTensorTypeAndShape((OrtValue*)value, &info);
    if (check_status(status, error) != 0) return -1;

    size_t count = 0;
    status = g_api->GetTensorShapeElementCount(info, &count);
    g_api->ReleaseTensorTypeAndShapeInfo(info);
    if (check_status(status, error) != 0) return -1;

    *out_data = data;
    *out_count = count;
    return 0;
}

int ort_wrapper_get_tensor_shape(
    OrtWrapperValue value,
    int64_t* out_dims,
    size_t max_dims,
    size_t* out_num_dims,
    OrtWrapperError* error
) {
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    OrtTensorTypeAndShapeInfo* info = NULL;
    OrtStatus* status = g_api->GetTensorTypeAndShape((OrtValue*)value, &info);
    if (check_status(status, error) != 0) return -1;

    size_t num_dims = 0;
    status = g_api->GetDimensionsCount(info, &num_dims);
    if (check_status(status, error) != 0) {
        g_api->ReleaseTensorTypeAndShapeInfo(info);
        return -1;
    }

    if (num_dims > max_dims) {
        g_api->ReleaseTensorTypeAndShapeInfo(info);
        set_error(error, ORT_WRAPPER_INVALID_ARGUMENT, "Too many dimensions");
        return -1;
    }

    status = g_api->GetDimensions(info, out_dims, num_dims);
    g_api->ReleaseTensorTypeAndShapeInfo(info);
    if (check_status(status, error) != 0) return -1;

    *out_num_dims = num_dims;
    return 0;
}

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
) {
    if (!g_api) { set_error(error, ORT_WRAPPER_FAIL, "ORT not initialized"); return -1; }

    OrtStatus* status = g_api->Run(
        (OrtSession*)session,
        NULL, // run options
        input_names,
        (const OrtValue* const*)inputs,
        input_count,
        output_names,
        output_count,
        (OrtValue**)outputs
    );
    return check_status(status, error);
}

// --- Utility ---

const char* ort_wrapper_version(void) {
    const OrtApiBase* base = OrtGetApiBase();
    if (base) {
        return base->GetVersionString();
    }
    return "unknown";
}

int ort_wrapper_coreml_available(void) {
#ifdef __APPLE__
    return 1;
#else
    return 0;
#endif
}
