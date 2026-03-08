//! Basic test to verify MCP config works
//! Tests if CLI accepts SDK MCP server config

use claude_agent_sdk::mcp::SdkMcpServer;
use claude_agent_sdk::types::{ClaudeAgentOptions, McpServerConfig, McpServers, SdkMcpServerMarker};
use claude_agent_sdk::ClaudeSDKClient;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MCP Basic Communication Test ===\n");

    // Test 1: No MCP servers - baseline
    println!("Test 1: Basic query without MCP...");
    let options = ClaudeAgentOptions {
        max_turns: Some(1),
        ..Default::default()
    };

    println!("  Creating client...");
    let mut client = ClaudeSDKClient::new(options, None).await?;
    println!("  Client created");

    println!("  Sending message...");
    client.send_message("Say hello in one word".to_string()).await?;
    println!("  Message sent, waiting for response...");

    let mut got_response = false;
    while let Some(message) = client.next_message().await {
        if let Ok(claude_agent_sdk::Message::Assistant { message, .. }) = message {
            println!("✓ Got response without MCP:");
            for block in &message.content {
                if let claude_agent_sdk::types::ContentBlock::Text { text } = block {
                    println!("  Claude: {text}");
                }
            }
            got_response = true;
            break;
        }
        if let Ok(claude_agent_sdk::Message::Result { .. }) = message {
            break;
        }
    }

    if !got_response {
        eprintln!("✗ No response - basic communication failing");
        return Ok(());
    }

    println!("\nTest 2: With SDK MCP server configured...");

    // Create empty SDK MCP server
    let server = Arc::new(SdkMcpServer::new("test").version("1.0.0"));

    let mut mcp_servers = HashMap::new();
    mcp_servers.insert(
        "test".to_string(),
        McpServerConfig::Sdk(SdkMcpServerMarker {
            name: "test".to_string(),
            instance: server,
        }),
    );

    let options = ClaudeAgentOptions {
        mcp_servers: McpServers::Dict(mcp_servers),
        max_turns: Some(1),
        ..Default::default()
    };

    println!("Creating client with SDK MCP server...");
    match ClaudeSDKClient::new(options, None).await {
        Ok(mut client) => {
            println!("✓ Client created with MCP config");
            client.send_message("Say hello in one word".to_string()).await?;

            let mut got_response = false;
            while let Some(message) = client.next_message().await {
                if let Ok(claude_agent_sdk::Message::Assistant { message, .. }) = message {
                    println!("✓ Got response with MCP configured:");
                    for block in &message.content {
                        if let claude_agent_sdk::types::ContentBlock::Text { text } = block {
                            println!("  Claude: {text}");
                        }
                    }
                    got_response = true;
                    break;
                }
                if let Ok(claude_agent_sdk::Message::Result { .. }) = message {
                    break;
                }
            }

            if !got_response {
                eprintln!("✗ No response with MCP - may not be supported");
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to create client with MCP: {e}");
        }
    }

    println!("\n=== Test Complete ===");
    Ok(())
}
