//! SDK MCP Server demonstration example
//!
//! This example demonstrates how to create custom tools using the SDK MCP server.
//! Custom tools allow you to extend Claude's capabilities with your own functions
//! that run in-process without subprocess overhead.
//!
//! Run with: cargo run --example mcp_demo

use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolContent, ToolResult};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SDK MCP Server Demonstration ===\n");

    // Example 1: Simple Calculator Tools
    println!("--- Example 1: Calculator Tools ---");
    println!("Creating MCP server with arithmetic operations\n");

    let calculator_server = create_calculator_server();

    // Test the server directly
    println!("Testing MCP server directly:");
    test_mcp_server(&calculator_server).await?;

    println!("\n✓ Example 1 complete: MCP server working\n");

    // Example 2: Text Processing Tools
    println!("--- Example 2: Text Processing Tools ---");
    println!("Creating MCP server with text utilities\n");

    let text_server = create_text_server();

    // Test text processing
    println!("Testing text processing tools:");
    test_text_server(&text_server).await?;

    println!("\n✓ Example 2 complete: Text processing tools working\n");

    // Example 3: Integration with ClaudeSDKClient
    println!("--- Example 3: Integration with ClaudeSDKClient (Optional) ---");
    println!("Note: Full integration requires Claude CLI to support SDK MCP servers");
    println!("This demonstrates the registration pattern:\n");

    show_integration_pattern();

    println!("\n✓ Example 3 complete: Integration pattern shown\n");

    // Summary
    println!("=== MCP Demo Complete ===\n");
    println!("Demonstrated MCP capabilities:");
    println!("1. ✓ Creating custom tools with async handlers");
    println!("2. ✓ JSONRPC protocol handling (tools/list, tools/call)");
    println!("3. ✓ Error handling in tools");
    println!("4. ✓ Integration pattern with ClaudeSDKClient");
    println!("\nSDK MCP servers are powerful for:");
    println!("  • In-process tool execution (no subprocess overhead)");
    println!("  • Custom business logic integration");
    println!("  • Performance-critical operations");
    println!("  • Type-safe tool definitions");

    Ok(())
}

/// Create a calculator MCP server with arithmetic operations
fn create_calculator_server() -> SdkMcpServer {
    let add_tool = SdkMcpTool::new(
        "add",
        "Add two numbers together",
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number", "description": "First number"},
                "b": {"type": "number", "description": "Second number"}
            },
            "required": ["a", "b"]
        }),
        |input| {
            Box::pin(async move {
                let a = input["a"].as_f64().unwrap_or(0.0);
                let b = input["b"].as_f64().unwrap_or(0.0);
                let result = a + b;

                println!("  [TOOL] add({a}, {b}) = {result}");

                Ok(ToolResult {
                    content: vec![ToolContent::Text {
                        text: format!("{a} + {b} = {result}"),
                    }],
                    is_error: None,
                })
            })
        },
    );

    let multiply_tool = SdkMcpTool::new(
        "multiply",
        "Multiply two numbers",
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"}
            },
            "required": ["a", "b"]
        }),
        |input| {
            Box::pin(async move {
                let a = input["a"].as_f64().unwrap_or(0.0);
                let b = input["b"].as_f64().unwrap_or(0.0);
                let result = a * b;

                println!("  [TOOL] multiply({a}, {b}) = {result}");

                Ok(ToolResult {
                    content: vec![ToolContent::Text {
                        text: format!("{a} × {b} = {result}"),
                    }],
                    is_error: None,
                })
            })
        },
    );

    let divide_tool = SdkMcpTool::new(
        "divide",
        "Divide two numbers (with error handling)",
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"}
            },
            "required": ["a", "b"]
        }),
        |input| {
            Box::pin(async move {
                let a = input["a"].as_f64().unwrap_or(0.0);
                let b = input["b"].as_f64().unwrap_or(0.0);

                if b == 0.0 {
                    println!("  [TOOL] divide({a}, {b}) = ERROR: division by zero");
                    return Ok(ToolResult::error("Cannot divide by zero"));
                }

                let result = a / b;
                println!("  [TOOL] divide({a}, {b}) = {result}");

                Ok(ToolResult {
                    content: vec![ToolContent::Text {
                        text: format!("{a} ÷ {b} = {result}"),
                    }],
                    is_error: None,
                })
            })
        },
    );

    SdkMcpServer::new("calculator")
        .version("1.0.0")
        .tool(add_tool)
        .tool(multiply_tool)
        .tool(divide_tool)
}

/// Create a text processing MCP server
fn create_text_server() -> SdkMcpServer {
    let reverse_tool = SdkMcpTool::new(
        "reverse",
        "Reverse a string",
        json!({
            "type": "object",
            "properties": {
                "text": {"type": "string"}
            },
            "required": ["text"]
        }),
        |input| {
            Box::pin(async move {
                let text = input["text"].as_str().unwrap_or("");
                let reversed: String = text.chars().rev().collect();

                println!("  [TOOL] reverse('{text}') = '{reversed}'");

                Ok(ToolResult::text(reversed))
            })
        },
    );

    let uppercase_tool = SdkMcpTool::new(
        "uppercase",
        "Convert text to uppercase",
        json!({
            "type": "object",
            "properties": {
                "text": {"type": "string"}
            },
            "required": ["text"]
        }),
        |input| {
            Box::pin(async move {
                let text = input["text"].as_str().unwrap_or("");
                let uppercase = text.to_uppercase();

                println!("  [TOOL] uppercase('{text}') = '{uppercase}'");

                Ok(ToolResult::text(uppercase))
            })
        },
    );

    let word_count_tool = SdkMcpTool::new(
        "word_count",
        "Count words in text",
        json!({
            "type": "object",
            "properties": {
                "text": {"type": "string"}
            },
            "required": ["text"]
        }),
        |input| {
            Box::pin(async move {
                let text = input["text"].as_str().unwrap_or("");
                let count = text.split_whitespace().count();

                println!("  [TOOL] word_count('{text}') = {count}");

                Ok(ToolResult::text(format!("Word count: {count}")))
            })
        },
    );

    SdkMcpServer::new("text-tools")
        .version("1.0.0")
        .tools(vec![reverse_tool, uppercase_tool, word_count_tool])
}

/// Test the calculator server
async fn test_mcp_server(server: &SdkMcpServer) -> Result<(), Box<dyn std::error::Error>> {
    use claude_agent_sdk::mcp::protocol::JsonRpcRequest;

    // Test tools/list
    let list_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/list".to_string(),
        params: None,
    };

    let response = server.handle_request(list_request).await?;
    if let Some(result) = response.result {
        let tools = result["tools"].as_array().unwrap();
        println!("  Available tools: {}", tools.len());
        for tool in tools {
            println!("    - {}: {}", tool["name"], tool["description"]);
        }
    }

    // Test add
    println!("\n  Calling add(5, 3):");
    let add_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(2)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "add",
            "arguments": {"a": 5, "b": 3}
        })),
    };

    let response = server.handle_request(add_request).await?;
    if let Some(result) = response.result {
        println!("  Result: {}", result["content"][0]["text"]);
    }

    // Test divide with error
    println!("\n  Calling divide(10, 0) - should error:");
    let divide_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(3)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "divide",
            "arguments": {"a": 10, "b": 0}
        })),
    };

    let response = server.handle_request(divide_request).await?;
    if let Some(result) = response.result {
        if let Some(true) = result["is_error"].as_bool() {
            println!("  Error (expected): {}", result["content"][0]["text"]);
        }
    }

    Ok(())
}

/// Test the text processing server
async fn test_text_server(server: &SdkMcpServer) -> Result<(), Box<dyn std::error::Error>> {
    use claude_agent_sdk::mcp::protocol::JsonRpcRequest;

    // Test reverse
    println!("  Calling reverse('hello'):");
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "reverse",
            "arguments": {"text": "hello"}
        })),
    };

    let response = server.handle_request(request).await?;
    if let Some(result) = response.result {
        println!("  Result: {}", result["content"][0]["text"]);
    }

    // Test word_count
    println!("\n  Calling word_count('the quick brown fox'):");
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(2)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "word_count",
            "arguments": {"text": "the quick brown fox"}
        })),
    };

    let response = server.handle_request(request).await?;
    if let Some(result) = response.result {
        println!("  Result: {}", result["content"][0]["text"]);
    }

    Ok(())
}

/// Show how to integrate MCP servers with ClaudeSDKClient
fn show_integration_pattern() {
    println!(r#"
// Create your MCP server
let server = SdkMcpServer::new("my-tools")
    .tool(/* ... */);

// Option 1: Register via ClaudeAgentOptions (future feature)
let options = ClaudeAgentOptions::builder()
    .add_mcp_server("my-tools", server)
    .add_allowed_tool("add")
    .add_allowed_tool("multiply")
    .build();

// Option 2: Handle MCP requests manually via control protocol
// When Claude requests a tool, the control protocol will route
// the request to your MCP server's handle_request() method
"#);
}
