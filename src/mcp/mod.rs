//! SDK MCP Server Integration
//!
//! This module provides in-process MCP (Model Context Protocol) server support,
//! allowing you to create custom tools that Claude can invoke directly without
//! subprocess management.
//!
//! # Benefits
//!
//! - **No subprocess overhead** - Tools run in the same process
//! - **Better performance** - No IPC serialization/deserialization
//! - **Simpler deployment** - Single binary with embedded tools
//! - **Easier debugging** - Direct function calls with full stack traces
//! - **Type safety** - Compile-time verification of tool signatures
//!
//! # Quick Start
//!
//! ```rust
//! use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolResult, ToolContent};
//! use serde_json::json;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Define a custom tool
//! let greet_tool = SdkMcpTool::new(
//!     "greet",
//!     "Greet a user by name",
//!     json!({
//!         "type": "object",
//!         "properties": {
//!             "name": {"type": "string"}
//!         },
//!         "required": ["name"]
//!     }),
//!     |input| Box::pin(async move {
//!         let name = input["name"].as_str().unwrap_or("stranger");
//!         Ok(ToolResult {
//!             content: vec![ToolContent::Text {
//!                 text: format!("Hello, {}!", name),
//!             }],
//!             is_error: None,
//!         })
//!     }),
//! );
//!
//! // Create an MCP server with the tool
//! let server = SdkMcpServer::new("my-tools")
//!     .version("1.0.0")
//!     .tool(greet_tool);
//!
//! # Ok(())
//! # }
//! ```
//!
//! # Tool Handler
//!
//! Tool handlers are async functions that take a [`serde_json::Value`] as input
//! and return a [`ToolResult`]. The handler signature is:
//!
//! ```rust,ignore
//! async fn(serde_json::Value) -> Result<ToolResult>
//! ```
//!
//! # MCP Protocol
//!
//! The SDK implements the MCP JSONRPC protocol for tool invocation:
//! - `tools/list` - Lists available tools
//! - `tools/call` - Invokes a specific tool
//!
//! # Integration with Claude
//!
//! Register MCP servers via [`ClaudeAgentOptions`](crate::types::ClaudeAgentOptions):
//!
//! ```rust,ignore
//! use claude_agent_sdk::{ClaudeAgentOptions, query};
//! use claude_agent_sdk::mcp::SdkMcpServer;
//!
//! let server = SdkMcpServer::new("my-tools")
//!     .tool(/* ... */);
//!
//! let options = ClaudeAgentOptions::builder()
//!     .add_mcp_server("my-tools", server)
//!     .add_allowed_tool("greet")
//!     .build();
//!
//! let stream = query("Please greet Alice", Some(options)).await?;
//! ```

pub mod protocol;
mod server;
mod tool;

pub use protocol::{JsonRpcRequest, JsonRpcResponse, McpError};
pub use server::SdkMcpServer;
pub use tool::{SdkMcpTool, ToolContent, ToolResult};

/// Type alias for tool handler functions
///
/// A tool handler is an async function that receives input as JSON and returns
/// a [`ToolResult`]. Handlers must be `Send + Sync + 'static` to support
/// concurrent invocation.
pub type ToolHandler = Arc<
    dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = crate::error::Result<ToolResult>> + Send>>
        + Send
        + Sync,
>;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
