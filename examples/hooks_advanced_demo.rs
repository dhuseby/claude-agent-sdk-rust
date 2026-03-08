//! Advanced Hooks Demo - Validation and Logging
//!
//! This example demonstrates:
//! - Validating and blocking dangerous commands
//! - Logging all tool usage for auditing
//! - Multiple hooks on the same event
//! - Tool-specific vs global hooks

use claude_agent_sdk::types::{
    ClaudeAgentOptions, HookContext, HookEvent, HookMatcher, HookOutput, Message,
};
use claude_agent_sdk::ClaudeSDKClient;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Advanced Hooks Demo ===\n");
    println!("This demo shows:");
    println!("1. Blocking dangerous bash commands");
    println!("2. Logging all tool usage\n");

    // Hook 1: Validate and block dangerous bash commands
    let validate_bash_command = |input: serde_json::Value,
                                 tool_name: Option<String>,
                                 _context: HookContext| {
        Box::pin(async move {
            if let Some(tool) = &tool_name {
                if tool == "Bash" {
                    // Check for dangerous command
                    if let Some(command) = input
                        .get("tool_input")
                        .and_then(|i| i.get("command"))
                        .and_then(|c| c.as_str())
                    {
                        // Block any rm command
                        if command.contains("rm ") || command.starts_with("rm ") {
                            println!("  🚫 [HOOK] Blocking dangerous rm command: {}", command);

                            return Ok(HookOutput {
                                hook_specific_output: Some(serde_json::json!({
                                    "hookEventName": "PreToolUse",
                                    "permissionDecision": "deny",
                                    "permissionDecisionReason": "Dangerous rm command blocked for safety"
                                })),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
            Ok(HookOutput::default())
        })
            as std::pin::Pin<
                Box<
                    dyn std::future::Future<
                            Output = Result<HookOutput, claude_agent_sdk::error::ClaudeError>,
                        > + Send,
                >,
            >
    };

    // Hook 2: Log all tool usage for auditing
    let log_tool_use =
        |input: serde_json::Value, tool_name: Option<String>, _context: HookContext| {
            Box::pin(async move {
                let tool = tool_name.as_deref().unwrap_or("unknown");
                let command = input
                    .get("tool_input")
                    .and_then(|i| i.get("command"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("");

                if !command.is_empty() {
                    println!("  📝 [AUDIT] Tool used: {} - Command: {}", tool, command);
                } else {
                    println!("  📝 [AUDIT] Tool used: {}", tool);
                }

                Ok(HookOutput::default())
            })
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = Result<HookOutput, claude_agent_sdk::error::ClaudeError>,
                            > + Send,
                    >,
                >
        };

    // Another logging hook for post-execution
    let log_tool_result =
        |_input: serde_json::Value, tool_name: Option<String>, _context: HookContext| {
            Box::pin(async move {
                let tool = tool_name.as_deref().unwrap_or("unknown");
                println!("  ✓ [AUDIT] Tool completed: {}", tool);

                Ok(HookOutput::default())
            })
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = Result<HookOutput, claude_agent_sdk::error::ClaudeError>,
                            > + Send,
                    >,
                >
        };

    // Configure hooks
    let mut hooks = HashMap::new();

    // PreToolUse hooks
    hooks.insert(
        HookEvent::PreToolUse,
        vec![
            // Matcher 1: Only for Bash tools - validates commands
            HookMatcher {
                matcher: Some("Bash".to_string()),
                hooks: vec![Arc::new(validate_bash_command)],
            },
            // Matcher 2: All tools - logs usage
            HookMatcher {
                matcher: None, // None = matches all tools
                hooks: vec![Arc::new(log_tool_use)],
            },
        ],
    );

    // PostToolUse hooks
    hooks.insert(
        HookEvent::PostToolUse,
        vec![HookMatcher {
            matcher: None,
            hooks: vec![Arc::new(log_tool_result)],
        }],
    );

    let options = ClaudeAgentOptions {
        hooks: Some(hooks),
        max_turns: Some(3),
        ..Default::default()
    };

    println!("--- Test 1: Normal Command ---");
    println!("Sending: List files in current directory\n");

    let mut client = ClaudeSDKClient::new(options.clone(), None).await?;
    client
        .send_message("List files in the current directory using ls".to_string())
        .await?;

    // Process messages
    while let Some(message) = client.next_message().await {
        match message {
            Ok(Message::Assistant { message, .. }) => {
                println!("  [ASSISTANT MESSAGE]");
                for block in &message.content {
                    match block {
                        claude_agent_sdk::types::ContentBlock::Text { text } => {
                            println!("  Claude: {}\n", text);
                        }
                        claude_agent_sdk::types::ContentBlock::ToolUse { name, .. } => {
                            println!("  [Tool use: {}]", name);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Message::Result { is_error, .. }) => {
                if is_error {
                    println!("❌ Conversation ended with error\n");
                } else {
                    println!("✓ Conversation completed\n");
                }
                break;
            }
            Ok(msg) => {
                println!("  [OTHER MESSAGE: {:?}]", std::mem::discriminant(&msg));
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }

    println!("\n--- Test 2: Dangerous Command (Should be blocked) ---");
    println!("Sending: Try to run 'rm'\n");

    let mut client2 = ClaudeSDKClient::new(options, None).await?;
    client2
        .send_message("try to use rm delete file delete.md".to_string())
        .await?;

    // Process messages
    while let Some(message) = client2.next_message().await {
        match message {
            Ok(Message::Assistant { message, .. }) => {
                println!("  [ASSISTANT MESSAGE]");
                for block in &message.content {
                    match block {
                        claude_agent_sdk::types::ContentBlock::Text { text } => {
                            println!("  Claude: {}\n", text);
                        }
                        claude_agent_sdk::types::ContentBlock::ToolUse { name, .. } => {
                            println!("  [Tool use: {}]", name);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Message::Result { is_error, .. }) => {
                if is_error {
                    println!("❌ Conversation ended (command blocked as expected)\n");
                } else {
                    println!("✓ Conversation completed\n");
                }
                break;
            }
            Ok(msg) => {
                println!("  [OTHER MESSAGE: {:?}]", std::mem::discriminant(&msg));
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }

    println!("=== Advanced Hooks Demo Complete ===\n");
    println!("Demonstrated:");
    println!("✓ Command validation and blocking");
    println!("✓ Audit logging for all tools");
    println!("✓ Multiple hooks on same event");
    println!("✓ Tool-specific vs global matchers");

    Ok(())
}
