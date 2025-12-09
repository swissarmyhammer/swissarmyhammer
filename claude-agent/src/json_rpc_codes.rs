//! JSON-RPC 2.0 Error Codes
//!
//! Standard error codes as defined in JSON-RPC 2.0 specification:
//! https://www.jsonrpc.org/specification#error_object

/// Parse error - Invalid JSON was received by the server
pub const PARSE_ERROR: i32 = -32700;

/// Invalid Request - The JSON sent is not a valid Request object
pub const INVALID_REQUEST: i32 = -32600;

/// Method not found - The method does not exist / is not available
pub const METHOD_NOT_FOUND: i32 = -32601;

/// Invalid params - Invalid method parameter(s)
pub const INVALID_PARAMS: i32 = -32602;

/// Internal error - Internal JSON-RPC error
pub const INTERNAL_ERROR: i32 = -32603;

/// Server error - Reserved for implementation-defined server errors
pub const SERVER_ERROR: i32 = -32000;

/// Check if error code is a standard JSON-RPC error
pub fn is_standard_error(code: i32) -> bool {
    matches!(
        code,
        PARSE_ERROR | INVALID_REQUEST | METHOD_NOT_FOUND | INVALID_PARAMS | INTERNAL_ERROR
    )
}

/// Check if error code is a server error (implementation-defined)
pub fn is_server_error(code: i32) -> bool {
    (-32099..=-32000).contains(&code)
}

/// Get human-readable description of error code
pub fn error_description(code: i32) -> &'static str {
    match code {
        PARSE_ERROR => "Parse error - Invalid JSON",
        INVALID_REQUEST => "Invalid Request - Not a valid Request object",
        METHOD_NOT_FOUND => "Method not found",
        INVALID_PARAMS => "Invalid params - Invalid method parameter(s)",
        INTERNAL_ERROR => "Internal error",
        code if is_server_error(code) => "Server error",
        _ => "Unknown error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_have_correct_values() {
        assert_eq!(PARSE_ERROR, -32700);
        assert_eq!(INVALID_REQUEST, -32600);
        assert_eq!(METHOD_NOT_FOUND, -32601);
        assert_eq!(INVALID_PARAMS, -32602);
        assert_eq!(INTERNAL_ERROR, -32603);
        assert_eq!(SERVER_ERROR, -32000);
    }

    #[test]
    fn test_is_standard_error_identifies_standard_errors() {
        assert!(is_standard_error(PARSE_ERROR));
        assert!(is_standard_error(INVALID_REQUEST));
        assert!(is_standard_error(METHOD_NOT_FOUND));
        assert!(is_standard_error(INVALID_PARAMS));
        assert!(is_standard_error(INTERNAL_ERROR));
    }

    #[test]
    fn test_is_standard_error_rejects_server_errors() {
        assert!(!is_standard_error(SERVER_ERROR));
        assert!(!is_standard_error(-32001));
        assert!(!is_standard_error(-32099));
    }

    #[test]
    fn test_is_standard_error_rejects_unknown_codes() {
        assert!(!is_standard_error(0));
        assert!(!is_standard_error(1));
        assert!(!is_standard_error(-1));
        assert!(!is_standard_error(-32100));
    }

    #[test]
    fn test_is_server_error_identifies_server_error_range() {
        assert!(is_server_error(-32000));
        assert!(is_server_error(-32001));
        assert!(is_server_error(-32050));
        assert!(is_server_error(-32099));
    }

    #[test]
    fn test_is_server_error_rejects_standard_errors() {
        assert!(!is_server_error(PARSE_ERROR));
        assert!(!is_server_error(INVALID_REQUEST));
        assert!(!is_server_error(METHOD_NOT_FOUND));
        assert!(!is_server_error(INVALID_PARAMS));
        assert!(!is_server_error(INTERNAL_ERROR));
    }

    #[test]
    fn test_is_server_error_rejects_out_of_range() {
        assert!(!is_server_error(-31999));
        assert!(!is_server_error(-32100));
        assert!(!is_server_error(0));
    }

    #[test]
    fn test_error_description_for_standard_errors() {
        assert_eq!(error_description(PARSE_ERROR), "Parse error - Invalid JSON");
        assert_eq!(
            error_description(INVALID_REQUEST),
            "Invalid Request - Not a valid Request object"
        );
        assert_eq!(error_description(METHOD_NOT_FOUND), "Method not found");
        assert_eq!(
            error_description(INVALID_PARAMS),
            "Invalid params - Invalid method parameter(s)"
        );
        assert_eq!(error_description(INTERNAL_ERROR), "Internal error");
    }

    #[test]
    fn test_error_description_for_server_errors() {
        assert_eq!(error_description(SERVER_ERROR), "Server error");
        assert_eq!(error_description(-32001), "Server error");
        assert_eq!(error_description(-32099), "Server error");
    }

    #[test]
    fn test_error_description_for_unknown_codes() {
        assert_eq!(error_description(0), "Unknown error");
        assert_eq!(error_description(1), "Unknown error");
        assert_eq!(error_description(-32100), "Unknown error");
    }
}
