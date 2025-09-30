//! SDK MCP Server Implementation
//!
//! Provides an in-process MCP server for registering and invoking custom tools.

use std::collections::HashMap;

use super::protocol::{JsonRpcRequest, JsonRpcResponse, McpError};
use super::tool::SdkMcpTool;
use crate::error::{ClaudeError, Result};

/// An in-process MCP server that hosts custom tools
///
/// The SDK MCP server allows you to register custom tools that Claude can invoke
/// directly without subprocess management. Tools are registered via the builder
/// pattern and invoked through JSONRPC.
///
/// # Example
///
/// ```rust
/// use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
/// use serde_json::json;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let server = SdkMcpServer::new("calculator")
///     .version("1.0.0")
///     .tool(SdkMcpTool::new(
///         "add",
///         "Add two numbers",
///         json!({
///             "type": "object",
///             "properties": {
///                 "a": {"type": "number"},
///                 "b": {"type": "number"}
///             }
///         }),
///         |input| Box::pin(async move {
///             let sum = input["a"].as_f64().unwrap_or(0.0)
///                     + input["b"].as_f64().unwrap_or(0.0);
///             Ok(ToolResult::text(format!("Sum: {}", sum)))
///         }),
///     ));
///
/// // Server can now handle MCP requests
/// # Ok(())
/// # }
/// ```
pub struct SdkMcpServer {
    /// Server name
    name: String,
    /// Server version
    version: String,
    /// Registered tools (keyed by tool name)
    tools: HashMap<String, SdkMcpTool>,
}

impl SdkMcpServer {
    /// Create a new MCP server with the given name
    ///
    /// # Example
    ///
    /// ```rust
    /// use claude_agent_sdk::mcp::SdkMcpServer;
    ///
    /// let server = SdkMcpServer::new("my-tools");
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "1.0.0".to_string(),
            tools: HashMap::new(),
        }
    }

    /// Set the server version
    ///
    /// # Example
    ///
    /// ```rust
    /// use claude_agent_sdk::mcp::SdkMcpServer;
    ///
    /// let server = SdkMcpServer::new("my-tools")
    ///     .version("2.0.0");
    /// ```
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Register a tool with the server
    ///
    /// Tool names must be unique within a server. If a tool with the same name
    /// already exists, it will be replaced.
    ///
    /// # Example
    ///
    /// ```rust
    /// use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
    /// use serde_json::json;
    ///
    /// let server = SdkMcpServer::new("my-tools")
    ///     .tool(SdkMcpTool::new(
    ///         "greet",
    ///         "Greet someone",
    ///         json!({"type": "object"}),
    ///         |_| Box::pin(async { Ok(ToolResult::text("Hello!")) }),
    ///     ));
    /// ```
    pub fn tool(mut self, tool: SdkMcpTool) -> Self {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
        self
    }

    /// Register multiple tools at once
    ///
    /// # Example
    ///
    /// ```rust
    /// use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
    /// use serde_json::json;
    ///
    /// let tools = vec![
    ///     SdkMcpTool::new("tool1", "First tool", json!({}),
    ///         |_| Box::pin(async { Ok(ToolResult::text("1")) })),
    ///     SdkMcpTool::new("tool2", "Second tool", json!({}),
    ///         |_| Box::pin(async { Ok(ToolResult::text("2")) })),
    /// ];
    ///
    /// let server = SdkMcpServer::new("my-tools").tools(tools);
    /// ```
    pub fn tools(mut self, tools: Vec<SdkMcpTool>) -> Self {
        for tool in tools {
            let name = tool.name().to_string();
            self.tools.insert(name, tool);
        }
        self
    }

    /// Get the server name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the server version
    pub fn server_version(&self) -> &str {
        &self.version
    }

    /// Get a tool by name
    pub fn get_tool(&self, name: &str) -> Option<&SdkMcpTool> {
        self.tools.get(name)
    }

    /// Get all registered tools
    pub fn list_tools(&self) -> Vec<&SdkMcpTool> {
        self.tools.values().collect()
    }

    /// Handle an MCP JSONRPC request
    ///
    /// Processes JSONRPC requests according to the MCP protocol:
    /// - `tools/list` - Returns list of available tools
    /// - `tools/call` - Invokes a specific tool
    ///
    /// # Errors
    ///
    /// Returns [`ClaudeError::Mcp`] if the request is invalid or tool invocation fails.
    pub async fn handle_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let request_id = request.id.clone().unwrap_or(serde_json::json!(null));

        match request.method.as_str() {
            "tools/list" => self.handle_tools_list(request_id).await,
            "tools/call" => self.handle_tools_call(request_id, request.params).await,
            _ => Ok(JsonRpcResponse::error(
                request_id,
                McpError::method_not_found(&request.method),
            )),
        }
    }

    /// Handle `tools/list` request
    async fn handle_tools_list(&self, request_id: serde_json::Value) -> Result<JsonRpcResponse> {
        let tools: Vec<_> = self.tools.values().map(|tool| tool.to_tool_info()).collect();

        Ok(JsonRpcResponse::success(
            request_id,
            serde_json::json!({
                "tools": tools
            }),
        ))
    }

    /// Handle `tools/call` request
    async fn handle_tools_call(
        &self,
        request_id: serde_json::Value,
        params: Option<serde_json::Value>,
    ) -> Result<JsonRpcResponse> {
        // Validate parameters
        let params = match params {
            Some(p) => p,
            None => {
                return Ok(JsonRpcResponse::error(
                    request_id,
                    McpError::invalid_params("tools/call requires parameters".to_string()),
                ));
            }
        };

        let tool_name = match params["name"].as_str() {
            Some(name) => name,
            None => {
                return Ok(JsonRpcResponse::error(
                    request_id,
                    McpError::invalid_params("Missing tool name in parameters".to_string()),
                ));
            }
        };

        // Check if tool exists
        let tool = match self.tools.get(tool_name) {
            Some(t) => t,
            None => {
                return Ok(JsonRpcResponse::error(
                    request_id,
                    McpError::tool_not_found(tool_name),
                ));
            }
        };

        let arguments = params["arguments"].clone();

        // TODO: Add JSON schema validation here
        // For production use, validate arguments against tool.input_schema
        // using a crate like jsonschema or valico:
        //
        // if let Err(errors) = validate_schema(&arguments, &tool.input_schema) {
        //     return Ok(JsonRpcResponse::error(
        //         request_id,
        //         McpError::invalid_params(format!("Schema validation failed: {:?}", errors)),
        //     ));
        // }

        // Invoke the tool
        match tool.invoke(arguments).await {
            Ok(result) => {
                let result_json = serde_json::to_value(result)
                    .map_err(|e| ClaudeError::Mcp(format!("Failed to serialize result: {e}")))?;

                Ok(JsonRpcResponse::success(request_id, result_json))
            }
            Err(e) => Ok(JsonRpcResponse::error(
                request_id,
                McpError::internal_error(format!("Tool execution failed: {e}")),
            )),
        }
    }
}

impl std::fmt::Debug for SdkMcpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SdkMcpServer")
            .field("name", &self.name)
            .field("version", &self.version)
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::ToolResult;
    use serde_json::json;

    fn create_test_tool(name: &str) -> SdkMcpTool {
        SdkMcpTool::new(
            name,
            format!("Test tool {name}"),
            json!({"type": "object"}),
            |input| {
                Box::pin(async move {
                    let text = input["text"].as_str().unwrap_or("default");
                    Ok(ToolResult::text(text))
                })
            },
        )
    }

    #[test]
    fn test_server_creation() {
        let server = SdkMcpServer::new("test-server").version("1.0.0");

        assert_eq!(server.name(), "test-server");
        assert_eq!(server.server_version(), "1.0.0");
    }

    #[test]
    fn test_tool_registration() {
        let server = SdkMcpServer::new("test")
            .tool(create_test_tool("tool1"))
            .tool(create_test_tool("tool2"));

        assert_eq!(server.list_tools().len(), 2);
        assert!(server.get_tool("tool1").is_some());
        assert!(server.get_tool("tool2").is_some());
        assert!(server.get_tool("tool3").is_none());
    }

    #[test]
    fn test_multiple_tools_registration() {
        let tools = vec![create_test_tool("a"), create_test_tool("b")];

        let server = SdkMcpServer::new("test").tools(tools);

        assert_eq!(server.list_tools().len(), 2);
    }

    #[tokio::test]
    async fn test_tools_list_request() {
        let server = SdkMcpServer::new("test")
            .tool(create_test_tool("tool1"))
            .tool(create_test_tool("tool2"));

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = server.handle_request(request).await.unwrap();

        assert!(response.result.is_some());
        let result = response.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
    }

    #[tokio::test]
    async fn test_tools_call_request() {
        let server = SdkMcpServer::new("test").tool(create_test_tool("echo"));

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "echo",
                "arguments": {
                    "text": "hello"
                }
            })),
        };

        let response = server.handle_request(request).await.unwrap();

        assert!(response.result.is_some());
        let result = response.result.unwrap();
        assert_eq!(result["content"][0]["text"], "hello");
    }

    #[tokio::test]
    async fn test_unknown_method() {
        let server = SdkMcpServer::new("test");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "unknown/method".to_string(),
            params: None,
        };

        let response = server.handle_request(request).await.unwrap();

        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32601);
    }

    #[tokio::test]
    async fn test_tool_not_found() {
        let server = SdkMcpServer::new("test");

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "nonexistent",
                "arguments": {}
            })),
        };

        let response = server.handle_request(request).await.unwrap();

        assert!(response.error.is_some());
    }
}
