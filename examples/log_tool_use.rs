//! Minimal example: Log all tool usage
//!
//! Run: cargo run --example log_tool_use

use claude_agent_sdk::hooks::HookManager;
use claude_agent_sdk::types::{ClaudeAgentOptions, HookEvent, HookOutput, Message};
use claude_agent_sdk::{ClaudeSDKClient, Result};
use std::collections::HashMap;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Tool Usage Logger ===\n");

    // Create hook that logs tool usage
    let log_hook = HookManager::callback(|data, tool_name, _ctx| async move {
        println!("🔧 Tool: {}", tool_name.as_deref().unwrap_or("unknown"));
        if let Some(input) = data.get("tool_input") {
            println!("   Input: {}", input);
        }
        println!();
        Ok(HookOutput::default())
    });

    // Register hook
    let matcher = claude_agent_sdk::hooks::HookMatcherBuilder::new(Some("*"))
        .add_hook(log_hook)
        .build();

    let mut hooks = HashMap::new();
    hooks.insert(HookEvent::PreToolUse, vec![matcher]);

    let options = ClaudeAgentOptions::builder()
        .max_turns(3)
        .hooks(hooks)
        .build();

    println!("Connecting to Claude CLI...");
    let mut client = ClaudeSDKClient::new(options, None).await?;

    println!("Sending message...\n");
    client.send_message("Use the Bash tool to run 'pwd' and tell me what directory we're in").await?;

    // Wait for response
    let timeout = Duration::from_secs(30);
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        match tokio::time::timeout(Duration::from_secs(5), client.next_message()).await {
            Ok(Some(Ok(Message::Result { .. }))) => {
                println!("✅ Done!");
                break;
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => {
                eprintln!("Error: {}", e);
                break;
            }
            Ok(None) => break,
            Err(_) => continue,
        }
    }

    client.close().await
}
