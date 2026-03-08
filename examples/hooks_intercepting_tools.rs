//! Hooks by intercepting tool use in messages
//!
//! This demonstrates triggering hooks by parsing Assistant messages
//! for tool use, since the CLI doesn't send PreToolUse events in stream-json mode.

use claude_agent_sdk::hooks::HookManager;
use claude_agent_sdk::types::{ClaudeAgentOptions, ContentBlock, HookContext, HookOutput, Message};
use claude_agent_sdk::ClaudeSDKClient;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Intercepting Tool Use for Hooks ===\n");

    // Create hook that will be triggered manually
    let logging_hook = HookManager::callback(|event_data, tool_name, _context| async move {
        println!("  🔔 [HOOK TRIGGERED]");
        println!("  📦 Tool: {:?}", tool_name.unwrap_or_else(|| "unknown".to_string()));
        println!("  📄 Data: {}", serde_json::to_string_pretty(&event_data)?);

        Ok(HookOutput {
            decision: None,
            system_message: Some("Hook logged the event".to_string()),
            hook_specific_output: None,
        })
    });

    // Build hook manager
    let matcher = claude_agent_sdk::hooks::HookMatcherBuilder::new(Some("*"))
        .add_hook(logging_hook)
        .build();

    let mut hook_manager = HookManager::default();
    hook_manager.register(matcher);

    // Create client without hooks in options (since CLI won't trigger them)
    let options = ClaudeAgentOptions::builder()
        .max_turns(5)
        .build();

    println!("Creating client...");
    let mut client = ClaudeSDKClient::new(options, None).await?;

    println!("Sending message that will trigger tool use...\n");
    client
        .send_message("List all files in the current directory using bash")
        .await?;

    // Read responses and manually trigger hooks
    loop {
        match tokio::time::timeout(Duration::from_secs(15), client.next_message()).await {
            Ok(Some(message)) => match message {
                Ok(Message::Result { .. }) => {
                    println!("\n✓ Complete: Hooks triggered via message interception\n");
                    break;
                }
                Ok(Message::Assistant { message: content, .. }) => {
                    println!("\n📨 Got assistant message with {} content blocks", content.content.len());

                    // Check each content block for tool use
                    for (i, block) in content.content.iter().enumerate() {
                        match block {
                            ContentBlock::ToolUse { id, name, input } => {
                                println!("\n  ⚙️  Detected tool use #{}: {}", i + 1, name);
                                println!("  🆔 Tool Use ID: {}", id);

                                // Manually trigger PreToolUse hook
                                let hook_data = serde_json::json!({
                                    "tool_name": name,
                                    "tool_input": input,
                                    "tool_use_id": id
                                });

                                println!("\n  🎣 Triggering PreToolUse hook...");
                                match hook_manager.invoke(
                                    hook_data,
                                    Some(name.clone()),
                                    HookContext {}
                                ).await {
                                    Ok(output) => {
                                        println!("  ✅ Hook executed successfully");
                                        if let Some(msg) = output.system_message {
                                            println!("  💬 Hook message: {}", msg);
                                        }
                                        if let Some(decision) = output.decision {
                                            println!("  ⚖️  Hook decision: {:?}", decision);
                                        }
                                    }
                                    Err(e) => {
                                        println!("  ❌ Hook error: {}", e);
                                    }
                                }
                            }
                            ContentBlock::Text { text } => {
                                let preview = if text.len() > 60 {
                                    format!("{}...", &text[..60])
                                } else {
                                    text.clone()
                                };
                                println!("  💬 Text block: {}", preview);
                            }
                            _ => {
                                println!("  📦 Other content block");
                            }
                        }
                    }
                }
                Ok(Message::User { .. }) => {
                    println!("  📤 User message (tool result)");
                }
                Ok(Message::System { subtype, .. }) => {
                    println!("  ⚙️  System message: {}", subtype);
                }
                Ok(_) => {
                    // Other message types
                }
                Err(e) => {
                    eprintln!("  ❌ Error: {}", e);
                    break;
                }
            },
            Ok(None) => {
                println!("  🔚 Stream ended");
                break;
            }
            Err(_) => {
                println!("  ⏱️  Timeout");
                break;
            }
        }
    }

    client.close().await?;

    println!("\n=== Summary ===");
    println!("This example shows how to:");
    println!("1. ✓ Parse Assistant messages for ToolUse content blocks");
    println!("2. ✓ Manually trigger hooks when tool use is detected");
    println!("3. ✓ Log tool usage even though CLI doesn't send PreToolUse events");
    println!("\nNote: The tool still executes (CLI does that), but you can");
    println!("observe, log, and potentially block tools with this approach.");

    Ok(())
}
