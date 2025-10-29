//! Test that hooks receive event data correctly
//!
//! Run with: cargo run --example test_hook_data

use claude_agent_sdk::hooks::HookManager;
use claude_agent_sdk::types::{ClaudeAgentOptions, HookEvent, HookOutput, Message};
use claude_agent_sdk::ClaudeSDKClient;
use std::collections::HashMap;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Hook Data Extraction ===\n");

    // Create a hook that prints the data it receives
    let test_hook = HookManager::callback(|event_data, tool_name, _context| async move {
        println!("🎯 Hook received data:");
        println!("   Tool name: {:?}", tool_name);
        println!("   Event data: {}", serde_json::to_string_pretty(&event_data).unwrap());
        println!();

        // Check if we got real data (not empty)
        if event_data.is_object() && !event_data.as_object().unwrap().is_empty() {
            println!("✅ SUCCESS: Hook received actual data (not empty)!");
        } else {
            println!("❌ FAILED: Hook received empty data");
        }

        Ok(HookOutput::default())
    });

    // Register the hook for PreToolUse events
    let matcher = claude_agent_sdk::hooks::HookMatcherBuilder::new(Some("*"))
        .add_hook(test_hook)
        .build();

    let mut hooks = HashMap::new();
    hooks.insert(HookEvent::PreToolUse, vec![matcher]);

    let options = ClaudeAgentOptions::builder()
        .max_turns(2)
        .hooks(hooks)
        .build();

    println!("Creating client with hook...");
    let mut client = ClaudeSDKClient::new(options, None).await?;

    println!("Sending message that will trigger tool use...\n");
    client
        .send_message("What is the current directory? Use pwd command")
        .await?;

    // Read responses for a bit
    let timeout_duration = Duration::from_secs(15);
    let start = std::time::Instant::now();

    while start.elapsed() < timeout_duration {
        match tokio::time::timeout(Duration::from_secs(2), client.next_message()).await {
            Ok(Some(message)) => match message {
                Ok(Message::Result { .. }) => {
                    println!("\n✅ Test complete!");
                    break;
                }
                Ok(Message::Assistant { .. }) => {
                    println!("📝 Received assistant message");
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Error: {}", e);
                    break;
                }
            },
            Ok(None) => break,
            Err(_) => {
                println!("Waiting for response...");
                continue;
            }
        }
    }

    client.close().await?;
    Ok(())
}
