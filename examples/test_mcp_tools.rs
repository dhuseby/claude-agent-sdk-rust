//! Test MCP Tools Recognition
//!
//! This example verifies that SDK MCP servers are properly recognized by Claude
//! and can be invoked correctly.

use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
use claude_agent_sdk::types::{
    ClaudeAgentOptions, McpServerConfig, McpServers, Message, SdkMcpServerMarker,
};
use claude_agent_sdk::ClaudeSDKClient;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MCP Tools Recognition Test ===\n");

    // Create a simple calculator MCP server
    let calculator = SdkMcpServer::new("calculator")
        .version("1.0.0")
        .tool(SdkMcpTool::new(
            "add",
            "Add two numbers together",
            json!({
                "type": "object",
                "properties": {
                    "a": {
                        "type": "number",
                        "description": "First number"
                    },
                    "b": {
                        "type": "number",
                        "description": "Second number"
                    }
                },
                "required": ["a", "b"]
            }),
            |input| {
                Box::pin(async move {
                    let a = input["a"].as_f64().unwrap_or(0.0);
                    let b = input["b"].as_f64().unwrap_or(0.0);
                    let sum = a + b;
                    println!("  🧮 [CALCULATOR] add({}, {}) = {}", a, b, sum);
                    Ok(ToolResult::text(format!("The sum is: {}", sum)))
                })
            },
        ))
        .tool(SdkMcpTool::new(
            "multiply",
            "Multiply two numbers together",
            json!({
                "type": "object",
                "properties": {
                    "a": {
                        "type": "number",
                        "description": "First number"
                    },
                    "b": {
                        "type": "number",
                        "description": "Second number"
                    }
                },
                "required": ["a", "b"]
            }),
            |input| {
                Box::pin(async move {
                    let a = input["a"].as_f64().unwrap_or(1.0);
                    let b = input["b"].as_f64().unwrap_or(1.0);
                    let product = a * b;
                    println!("  🧮 [CALCULATOR] multiply({}, {}) = {}", a, b, product);
                    Ok(ToolResult::text(format!("The product is: {}", product)))
                })
            },
        ));

    // Register the SDK MCP server
    let mut mcp_servers = HashMap::new();
    mcp_servers.insert(
        "calculator".to_string(),
        McpServerConfig::Sdk(SdkMcpServerMarker {
            name: "calculator".to_string(),
            instance: Arc::new(calculator),
        }),
    );

    // Create options with SDK MCP server
    let options = ClaudeAgentOptions {
        mcp_servers: McpServers::Dict(mcp_servers),
        max_turns: Some(5),
        permission_mode: Some(claude_agent_sdk::types::PermissionMode::BypassPermissions),
        ..Default::default()
    };

    println!("📡 Creating client with SDK MCP server...\n");
    let mut client = ClaudeSDKClient::new(options, None).await?;

    println!("📤 Sending message: 'Use the calculator to add 15 and 27, then multiply the result by 3'\n");
    client
        .send_message("Use the calculator to add 15 and 27, then multiply the result by 3")
        .await?;

    // Process messages
    while let Some(message) = client.next_message().await {
        match message {
            Ok(Message::Assistant { message, .. }) => {
                println!("🤖 [ASSISTANT MESSAGE]");
                for block in &message.content {
                    match block {
                        claude_agent_sdk::types::ContentBlock::Text { text } => {
                            println!("  Text: {}\n", text);
                        }
                        claude_agent_sdk::types::ContentBlock::ToolUse { name, input, .. } => {
                            println!("  🔧 [Tool use: {}]", name);
                            println!("     Input: {}\n", serde_json::to_string_pretty(input)?);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Message::Result {
                is_error,
                num_turns,
                ..
            }) => {
                if is_error {
                    println!("❌ Conversation ended with error");
                } else {
                    println!("✅ Conversation completed successfully!");
                }
                println!("   Total turns: {}\n", num_turns);
                break;
            }
            Ok(msg) => {
                println!("  [OTHER MESSAGE: {:?}]", std::mem::discriminant(&msg));
            }
            Err(e) => {
                eprintln!("❌ Error: {}", e);
                break;
            }
        }
    }

    client.close().await?;

    println!("\n=== Test Complete ===");
    println!("\nExpected behavior:");
    println!("1. ✅ Calculator tools should appear in Claude's available tools");
    println!("2. ✅ Claude should successfully invoke 'add' with (15, 27)");
    println!("3. ✅ Claude should successfully invoke 'multiply' with (42, 3)");
    println!("4. ✅ Final answer should be 126");

    Ok(())
}
