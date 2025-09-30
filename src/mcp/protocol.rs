//! MCP JSONRPC Protocol Types
//!
//! This module defines the JSONRPC protocol structures used for MCP communication.

use serde::{Deserialize, Serialize};

/// JSONRPC request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSONRPC version (always "2.0")
    pub jsonrpc: String,
    /// Request ID (optional for notifications)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    /// Method name
    pub method: String,
    /// Method parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// JSONRPC response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSONRPC version (always "2.0")
    pub jsonrpc: String,
    /// Request ID (matches the request)
    pub id: serde_json::Value,
    /// Result (present on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error (present on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

impl JsonRpcResponse {
    /// Create a successful response
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: serde_json::Value, error: McpError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// JSONRPC error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl McpError {
    /// Method not found error (-32601)
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {method}"),
            data: None,
        }
    }

    /// Invalid params error (-32602)
    pub fn invalid_params(message: String) -> Self {
        Self {
            code: -32602,
            message,
            data: None,
        }
    }

    /// Internal error (-32603)
    pub fn internal_error(message: String) -> Self {
        Self {
            code: -32603,
            message,
            data: None,
        }
    }

    /// Tool not found error (custom code -32001)
    pub fn tool_not_found(tool_name: &str) -> Self {
        Self {
            code: -32001,
            message: format!("Tool not found: {tool_name}"),
            data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_jsonrpc_request_serialization() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };

        let serialized = serde_json::to_string(&request).unwrap();
        assert!(serialized.contains("\"jsonrpc\":\"2.0\""));
        assert!(serialized.contains("\"method\":\"tools/list\""));
    }

    #[test]
    fn test_jsonrpc_success_response() {
        let response = JsonRpcResponse::success(json!(1), json!({"result": "ok"}));

        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, json!(1));
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_jsonrpc_error_response() {
        let error = McpError::method_not_found("unknown/method");
        let response = JsonRpcResponse::error(json!(1), error);

        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, json!(1));
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let err = response.error.unwrap();
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("Method not found"));
    }

    #[test]
    fn test_mcp_error_codes() {
        assert_eq!(McpError::method_not_found("test").code, -32601);
        assert_eq!(McpError::invalid_params("test".to_string()).code, -32602);
        assert_eq!(McpError::internal_error("test".to_string()).code, -32603);
        assert_eq!(McpError::tool_not_found("test").code, -32001);
    }
}
