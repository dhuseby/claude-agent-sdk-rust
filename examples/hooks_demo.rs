//! Hooks demonstration example
//!
//! This example demonstrates how to use the hook system to intercept
//! and respond to events during Claude's execution.
//!
//! Hooks allow you to:
//! - Monitor tool usage (pre and post execution)
//! - Block certain operations
//! - Add custom logging or auditing
//! - Inject custom behavior at specific points
//!
//! Run with: cargo run --example hooks_demo

use claude_agent_sdk::hooks::HookManager;
use claude_agent_sdk::types::{ClaudeAgentOptions, HookDecision, HookEvent, HookOutput, Message};
use claude_agent_sdk::ClaudeSDKClient;
use std::collections::HashMap;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Hooks System Demonstration ===\n");

    // Example 1: Logging Hook
    println!("--- Example 1: Simple Logging Hook ---");
    println!("This hook logs all pre-tool-use events\n");

    let logging_hook = HookManager::callback(|event_data, tool_name, _context| async move {
        println!(
            "  [HOOK] Tool about to be used: {:?}",
            tool_name.unwrap_or_else(|| "unknown".to_string())
        );
        println!("  [HOOK] Event data: {event_data:?}");

        Ok(HookOutput {
            decision: None, // Allow the operation
            system_message: Some("Hook logged the event".to_string()),
            hook_specific_output: None,
        })
    });

    // Create hook matcher that matches all tools
    let matcher = claude_agent_sdk::hooks::HookMatcherBuilder::new(Some("*"))
        .add_hook(logging_hook)
        .build();

    // Configure hooks in options
    let mut hooks = HashMap::new();
    hooks.insert(HookEvent::PreToolUse, vec![matcher]);

    let options = ClaudeAgentOptions::builder()
        .max_turns(2)
        .hooks(hooks)
        .build();

    println!("Creating client with logging hook...");
    let mut client = ClaudeSDKClient::new(options, None).await?;

    println!("Sending message that will trigger tool use...\n");
    client.send_message("What files are in the current directory?").await?;

    // Read responses
    let mut message_count = 0;
    loop {
        match tokio::time::timeout(Duration::from_secs(15), client.next_message()).await {
            Ok(Some(message)) => match message {
                Ok(Message::Result { .. }) => {
                    println!("\n✓ Example 1 complete: Logging hook executed\n");
                    break;
                }
                Ok(Message::Assistant { .. }) => {
                    message_count += 1;
                    println!("  [Response {message_count}] Got assistant message");
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("  Error: {e}");
                    break;
                }
            },
            Ok(None) => break,
            Err(_) => {
                println!("  Timeout");
                break;
            }
        }
    }

    client.close().await?;

    // Example 2: Blocking Hook
    println!("--- Example 2: Blocking Hook ---");
    println!("This hook blocks specific tools from being used\n");

    let blocking_hook = HookManager::callback(|_event_data, tool_name, _context| async move {
        let tool = tool_name.unwrap_or_else(|| "unknown".to_string());

        // Block Write and Edit tools for safety
        if tool == "Write" || tool == "Edit" {
            println!("  [HOOK] Blocking tool: {tool}");
            return Ok(HookOutput {
                decision: Some(HookDecision::Block),
                system_message: Some(format!(
                    "Tool {tool} was blocked by security policy"
                )),
                hook_specific_output: None,
            });
        }

        println!("  [HOOK] Allowing tool: {tool}");
        Ok(HookOutput {
            decision: None,
            system_message: None,
            hook_specific_output: None,
        })
    });

    let matcher = claude_agent_sdk::hooks::HookMatcherBuilder::new(None::<String>) // Match all
        .add_hook(blocking_hook)
        .build();

    let mut hooks = HashMap::new();
    hooks.insert(HookEvent::PreToolUse, vec![matcher]);

    let options = ClaudeAgentOptions::builder()
        .max_turns(3)
        .hooks(hooks)
        .build();

    println!("Creating client with blocking hook...");
    let mut client = ClaudeSDKClient::new(options, None).await?;

    println!("Asking Claude to write a file (should be blocked)...\n");
    client.send_message("Create a file called test.txt with 'hello world'").await?;

    // Read responses
    message_count = 0;
    loop {
        match tokio::time::timeout(Duration::from_secs(15), client.next_message()).await {
            Ok(Some(message)) => match message {
                Ok(Message::Result { .. }) => {
                    println!("\n✓ Example 2 complete: Blocking hook executed\n");
                    break;
                }
                Ok(Message::Assistant { .. }) => {
                    message_count += 1;
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("  Error: {e}");
                    break;
                }
            },
            Ok(None) => break,
            Err(_) => {
                println!("  Timeout");
                break;
            }
        }
    }

    client.close().await?;

    // Example 3: Tool-Specific Hook
    println!("--- Example 3: Tool-Specific Hook ---");
    println!("This hook only triggers for the Bash tool\n");

    let bash_hook = HookManager::callback(|event_data, tool_name, _context| async move {
        println!(
            "  [HOOK] Bash tool intercepted: {:?}",
            tool_name.unwrap_or_default()
        );
        println!("  [HOOK] Command data: {event_data:?}");
        println!("  [HOOK] Adding audit log...");

        Ok(HookOutput {
            decision: None,
            system_message: Some("Command audited and approved".to_string()),
            hook_specific_output: Some(serde_json::json!({
                "audit": {
                    "timestamp": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    "approved": true
                }
            })),
        })
    });

    let matcher = claude_agent_sdk::hooks::HookMatcherBuilder::new(Some("Bash"))
        .add_hook(bash_hook)
        .build();

    let mut hooks = HashMap::new();
    hooks.insert(HookEvent::PreToolUse, vec![matcher]);

    let options = ClaudeAgentOptions::builder()
        .max_turns(2)
        .hooks(hooks)
        .build();

    println!("Creating client with Bash-specific hook...");
    let mut client = ClaudeSDKClient::new(options, None).await?;

    println!("Asking Claude to run a command...\n");
    client.send_message("What is the current date and time?").await?;

    // Read responses
    loop {
        match tokio::time::timeout(Duration::from_secs(15), client.next_message()).await {
            Ok(Some(message)) => match message {
                Ok(Message::Result { .. }) => {
                    println!("\n✓ Example 3 complete: Tool-specific hook executed\n");
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("  Error: {e}");
                    break;
                }
            },
            Ok(None) => break,
            Err(_) => {
                println!("  Timeout");
                break;
            }
        }
    }

    client.close().await?;

    // Summary
    println!("=== Hooks Demo Complete ===\n");
    println!("Demonstrated hook capabilities:");
    println!("1. ✓ Logging hooks - Monitor all tool usage");
    println!("2. ✓ Blocking hooks - Prevent specific operations");
    println!("3. ✓ Tool-specific hooks - Target specific tools");
    println!("\nHooks are powerful for:");
    println!("  • Security and compliance");
    println!("  • Auditing and logging");
    println!("  • Custom business logic");
    println!("  • Testing and debugging");

    Ok(())
}
