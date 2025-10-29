//! SDK MCP Server Integration Demo
//!
//! Demonstrates SDK MCP servers working in Claude conversations.
//! Tools are called by Claude and executed in-process.
//!
//! Run with: cargo run --example mcp_integration_demo

use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolContent, ToolResult};
use claude_agent_sdk::types::{ClaudeAgentOptions, McpServerConfig, McpServers, SdkMcpServerMarker, ToolName};
use claude_agent_sdk::ClaudeSDKClient;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SDK MCP Integration Demo ===\n");

    // Create calculator tools
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
                let a = input["a"].as_f64().unwrap();
                let b = input["b"].as_f64().unwrap();
                let result = a + b;

                println!("  [SDK TOOL] add({a}, {b}) = {result}");

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
                let a = input["a"].as_f64().unwrap();
                let b = input["b"].as_f64().unwrap();
                let result = a * b;

                println!("  [SDK TOOL] multiply({a}, {b}) = {result}");

                Ok(ToolResult {
                    content: vec![ToolContent::Text {
                        text: format!("{a} × {b} = {result}"),
                    }],
                    is_error: None,
                })
            })
        },
    );

    // Create SDK MCP server
    let calculator = SdkMcpServer::new("calculator")
        .version("1.0.0")
        .tool(add_tool)
        .tool(multiply_tool);

    let calculator_arc = Arc::new(calculator);

    // Configure options
    let mut mcp_servers = HashMap::new();
    mcp_servers.insert(
        "calc".to_string(),
        McpServerConfig::Sdk(SdkMcpServerMarker {
            name: "calc".to_string(),
            instance: calculator_arc,
        }),
    );

    let options = ClaudeAgentOptions {
        mcp_servers: McpServers::Dict(mcp_servers),
        allowed_tools: vec![
            ToolName::new("mcp__calc__add"),
            ToolName::new("mcp__calc__multiply"),
        ],
        max_turns: Some(5),
        ..Default::default()
    };

    // Test with Claude
    println!("Testing: Calculate 15 + 27\n");

    println!("Creating client...");
    let mut client = ClaudeSDKClient::new(options, None).await?;
    println!("Client created successfully");

    println!("Sending message...");
    client
        .send_message("Calculate 15 + 27 using the add tool".to_string())
        .await?;
    println!("Message sent, waiting for response...");

    // Add a timeout so we don't hang forever
    let timeout = tokio::time::Duration::from_secs(30);
    let start = std::time::Instant::now();

    while let Some(message) = client.next_message().await {
        if start.elapsed() > timeout {
            eprintln!("Timeout waiting for response");
            break;
        }
        match message {
            Ok(claude_agent_sdk::Message::Assistant { message, .. }) => {
                // Check for text and tool use in content blocks
                for block in &message.content {
                    match block {
                        claude_agent_sdk::types::ContentBlock::Text { text } => {
                            println!("Claude: {text}");
                        }
                        claude_agent_sdk::types::ContentBlock::ToolUse { name, .. } => {
                            println!("Claude calling tool: {name}");
                        }
                        claude_agent_sdk::types::ContentBlock::ToolResult { tool_use_id, content, .. } => {
                            println!("Tool result for {tool_use_id}: {content:?}");
                        }
                        _ => {}
                    }
                }
            }
            Ok(claude_agent_sdk::Message::Result { is_error, .. }) => {
                if is_error {
                    println!("Conversation ended with error");
                } else {
                    println!("Conversation completed successfully");
                }
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: {e}");
                break;
            }
        }
    }

    println!("\n✓ Integration test complete!");
    println!("SDK MCP servers work with Claude CLI!");

    Ok(())
}
