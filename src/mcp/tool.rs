//! Tool Definitions for SDK MCP Server
//!
//! This module provides types for defining custom tools that Claude can invoke.

use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::Arc;

use crate::error::Result;
use super::ToolHandler;

/// A tool that can be invoked by Claude
///
/// Tools are defined with:
/// - A unique name
/// - A description explaining what the tool does
/// - A JSON schema defining the input parameters
/// - An async handler function that processes the input
///
/// # Example
///
/// ```rust
/// use claude_agent_sdk::mcp::{SdkMcpTool, ToolResult, ToolContent};
/// use serde_json::json;
///
/// let calculator = SdkMcpTool::new(
///     "add",
///     "Add two numbers together",
///     json!({
///         "type": "object",
///         "properties": {
///             "a": {"type": "number"},
///             "b": {"type": "number"}
///         },
///         "required": ["a", "b"]
///     }),
///     |input| Box::pin(async move {
///         let a = input["a"].as_f64().unwrap_or(0.0);
///         let b = input["b"].as_f64().unwrap_or(0.0);
///         let sum = a + b;
///
///         Ok(ToolResult {
///             content: vec![ToolContent::Text {
///                 text: format!("{} + {} = {}", a, b, sum),
///             }],
///             is_error: None,
///         })
///     }),
/// );
/// ```
pub struct SdkMcpTool {
    /// Tool name (must be unique within a server)
    pub(crate) name: String,
    /// Human-readable description
    pub(crate) description: String,
    /// JSON schema for input validation
    pub(crate) input_schema: serde_json::Value,
    /// Async handler function
    pub(crate) handler: ToolHandler,
}

impl SdkMcpTool {
    /// Create a new tool
    ///
    /// # Arguments
    ///
    /// * `name` - Unique tool identifier
    /// * `description` - What the tool does
    /// * `input_schema` - JSON schema for parameters
    /// * `handler` - Async function to execute the tool
    ///
    /// # Example
    ///
    /// ```rust
    /// use claude_agent_sdk::mcp::{SdkMcpTool, ToolResult, ToolContent};
    /// use serde_json::json;
    ///
    /// let tool = SdkMcpTool::new(
    ///     "echo",
    ///     "Echo back the input",
    ///     json!({"type": "object", "properties": {"text": {"type": "string"}}}),
    ///     |input| Box::pin(async move {
    ///         let text = input["text"].as_str().unwrap_or("");
    ///         Ok(ToolResult::text(text))
    ///     }),
    /// );
    /// ```
    pub fn new<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
        handler: F,
    ) -> Self
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ToolResult>> + Send + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            handler: Arc::new(move |input| Box::pin(handler(input))),
        }
    }

    /// Get the tool name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the tool description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get the input schema
    pub fn input_schema(&self) -> &serde_json::Value {
        &self.input_schema
    }

    /// Invoke the tool with the given input
    pub async fn invoke(&self, input: serde_json::Value) -> Result<ToolResult> {
        (self.handler)(input).await
    }

    /// Convert tool to MCP tools/list format
    pub(crate) fn to_tool_info(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "inputSchema": self.input_schema,
        })
    }
}

impl std::fmt::Debug for SdkMcpTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SdkMcpTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .finish()
    }
}

/// Result returned by a tool handler
///
/// Contains the tool output as a list of content blocks and an optional error flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Content blocks (text, images, etc.)
    pub content: Vec<ToolContent>,
    /// Whether this result represents an error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl ToolResult {
    /// Create a successful text result
    ///
    /// # Example
    ///
    /// ```rust
    /// use claude_agent_sdk::mcp::ToolResult;
    ///
    /// let result = ToolResult::text("Operation completed successfully");
    /// ```
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: text.into() }],
            is_error: None,
        }
    }

    /// Create an error result
    ///
    /// # Example
    ///
    /// ```rust
    /// use claude_agent_sdk::mcp::ToolResult;
    ///
    /// let result = ToolResult::error("Failed to connect to database");
    /// ```
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: text.into() }],
            is_error: Some(true),
        }
    }
}

/// Content block in a tool result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    /// Text content
    Text {
        /// The text content
        text: String,
    },
    /// Image content (base64 encoded)
    Image {
        /// Base64 encoded image data
        data: String,
        /// MIME type (e.g., "image/png")
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_tool_creation() {
        let tool = SdkMcpTool::new(
            "test",
            "Test tool",
            json!({"type": "object"}),
            |_input| async { Ok(ToolResult::text("test")) },
        );

        assert_eq!(tool.name(), "test");
        assert_eq!(tool.description(), "Test tool");
    }

    #[tokio::test]
    async fn test_tool_invocation() {
        let tool = SdkMcpTool::new(
            "echo",
            "Echo tool",
            json!({"type": "object"}),
            |input| async move {
                let text = input["text"].as_str().unwrap_or("empty");
                Ok(ToolResult::text(text))
            },
        );

        let result = tool.invoke(json!({"text": "hello"})).await.unwrap();
        assert_eq!(result.content.len(), 1);

        if let ToolContent::Text { text } = &result.content[0] {
            assert_eq!(text, "hello");
        } else {
            panic!("Expected text content");
        }
    }

    #[test]
    fn test_tool_result_text() {
        let result = ToolResult::text("success");
        assert_eq!(result.content.len(), 1);
        assert!(result.is_error.is_none());
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("failed");
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_tool_content_serialization() {
        let content = ToolContent::Text {
            text: "test".to_string(),
        };
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"test\""));
    }

    #[test]
    fn test_tool_info() {
        let tool = SdkMcpTool::new(
            "test",
            "Test tool",
            json!({"type": "object"}),
            |_| async { Ok(ToolResult::text("test")) },
        );

        let info = tool.to_tool_info();
        assert_eq!(info["name"], "test");
        assert_eq!(info["description"], "Test tool");
        assert!(info["inputSchema"].is_object());
    }
}
