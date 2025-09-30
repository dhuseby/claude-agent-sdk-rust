//! Permissions demonstration example
//!
//! This example demonstrates how to use the permission system to control
//! which tools Claude can use and with what parameters.
//!
//! The permission system allows you to:
//! - Allow or deny specific tools
//! - Modify tool inputs before execution
//! - Implement custom authorization logic
//! - Create allowlists and denylists
//!
//! Run with: cargo run --example permissions_demo

use claude_agent_sdk::permissions::PermissionManager;
use claude_agent_sdk::types::{
    ClaudeAgentOptions, Message, PermissionResult, PermissionResultAllow, PermissionResultDeny,
};
use claude_agent_sdk::ClaudeSDKClient;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Permissions System Demonstration ===\n");

    // Example 1: Simple Allow/Deny
    println!("--- Example 1: Basic Allow/Deny Permissions ---");
    println!("Only allowing Read and Glob tools\n");

    let permission_callback = PermissionManager::callback(
        |tool_name, _tool_input, _context| async move {
            let tool = tool_name.as_str();
            println!("  [PERMISSION] Checking tool: {tool}");

            match tool {
                "Read" | "Glob" => {
                    println!("  [PERMISSION] ✓ Allowed");
                    Ok(PermissionResult::Allow(PermissionResultAllow {
                        updated_input: None,
                        updated_permissions: None,
                    }))
                }
                _ => {
                    println!("  [PERMISSION] ✗ Denied");
                    Ok(PermissionResult::Deny(PermissionResultDeny {
                        message: format!("Tool {tool} is not in the allowed list"),
                        interrupt: false,
                    }))
                }
            }
        },
    );

    let options = ClaudeAgentOptions::builder()
        .max_turns(3)
        .can_use_tool(permission_callback)
        .build();

    println!("Creating client with permission callback...");
    let mut client = ClaudeSDKClient::new(options, None).await?;

    println!("Asking Claude to read a file...\n");
    client.send_message("Read the Cargo.toml file").await?;

    // Read responses
    let mut message_count = 0;
    loop {
        match tokio::time::timeout(Duration::from_secs(15), client.next_message()).await {
            Ok(Some(message)) => match message {
                Ok(Message::Result { .. }) => {
                    println!("\n✓ Example 1 complete: Permission checks executed\n");
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

    // Example 2: Input Modification
    println!("--- Example 2: Modifying Tool Inputs ---");
    println!("Sanitizing Bash commands for safety\n");

    let sanitizing_callback = PermissionManager::callback(
        |tool_name, tool_input, _context| async move {
            let tool = tool_name.as_str();

            if tool == "Bash" {
                // Check if command contains dangerous operations
                if let Some(command) = tool_input.get("command").and_then(|v| v.as_str()) {
                    println!("  [PERMISSION] Checking Bash command: {command}");

                    // Block dangerous commands
                    if command.contains("rm -rf") || command.contains("sudo") {
                        println!("  [PERMISSION] ✗ Blocked dangerous command");
                        return Ok(PermissionResult::Deny(PermissionResultDeny {
                            message: "Dangerous command blocked by security policy".to_string(),
                            interrupt: true, // Stop execution immediately
                        }));
                    }

                    // Sanitize: add safety flags
                    let sanitized_command = if command.contains("rm") {
                        format!("{command} -i") // Add interactive flag
                    } else {
                        command.to_string()
                    };

                    if sanitized_command != command {
                        println!("  [PERMISSION] ⚠ Modified command for safety");
                        println!("    Original: {command}");
                        println!("    Modified: {sanitized_command}");
                    } else {
                        println!("  [PERMISSION] ✓ Command approved as-is");
                    }

                    // Return with modified input
                    let mut modified_input = tool_input.clone();
                    modified_input["command"] = serde_json::json!(sanitized_command);

                    return Ok(PermissionResult::Allow(PermissionResultAllow {
                        updated_input: Some(modified_input),
                        updated_permissions: None,
                    }));
                }
            }

            // Allow other tools
            println!("  [PERMISSION] ✓ Tool {tool} allowed");
            Ok(PermissionResult::Allow(PermissionResultAllow {
                updated_input: None,
                updated_permissions: None,
            }))
        },
    );

    let options = ClaudeAgentOptions::builder()
        .max_turns(2)
        .can_use_tool(sanitizing_callback)
        .build();

    println!("Creating client with input sanitization...");
    let mut client = ClaudeSDKClient::new(options, None).await?;

    println!("Asking Claude to run a command...\n");
    client.send_message("List all .rs files in the src directory").await?;

    // Read responses
    loop {
        match tokio::time::timeout(Duration::from_secs(15), client.next_message()).await {
            Ok(Some(message)) => match message {
                Ok(Message::Result { .. }) => {
                    println!("\n✓ Example 2 complete: Input sanitization executed\n");
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

    // Example 3: Context-Aware Permissions
    println!("--- Example 3: Context-Aware Permissions ---");
    println!("Using permission context to make decisions\n");

    let context_aware_callback = PermissionManager::callback(
        |tool_name, _tool_input, context| async move {
            let tool = tool_name.as_str();
            println!("  [PERMISSION] Tool: {tool}");
            println!("  [PERMISSION] Context suggestions: {:?}", context.suggestions);

            // Check if there are permission suggestions
            if !context.suggestions.is_empty() {
                println!("  [PERMISSION] Using suggested permissions");
                // In a real implementation, you might apply the suggestions
            }

            // Allow Read-only tools by default
            match tool {
                "Read" | "Glob" | "Grep" => {
                    println!("  [PERMISSION] ✓ Read-only tool allowed");
                    Ok(PermissionResult::Allow(PermissionResultAllow {
                        updated_input: None,
                        updated_permissions: None,
                    }))
                }
                "Write" | "Edit" => {
                    println!("  [PERMISSION] ⚠ Write operation requires approval");
                    // In a real application, you might prompt the user here
                    Ok(PermissionResult::Deny(PermissionResultDeny {
                        message: "Write operations require explicit user approval".to_string(),
                        interrupt: false,
                    }))
                }
                _ => {
                    println!("  [PERMISSION] ✓ Default allow for tool: {tool}");
                    Ok(PermissionResult::Allow(PermissionResultAllow {
                        updated_input: None,
                        updated_permissions: None,
                    }))
                }
            }
        },
    );

    let options = ClaudeAgentOptions::builder()
        .max_turns(2)
        .can_use_tool(context_aware_callback)
        .build();

    println!("Creating client with context-aware permissions...");
    let mut client = ClaudeSDKClient::new(options, None).await?;

    println!("Asking Claude to analyze code...\n");
    client.send_message("Find all TODO comments in the src directory").await?;

    // Read responses
    loop {
        match tokio::time::timeout(Duration::from_secs(15), client.next_message()).await {
            Ok(Some(message)) => match message {
                Ok(Message::Result { .. }) => {
                    println!("\n✓ Example 3 complete: Context-aware permissions executed\n");
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
    println!("=== Permissions Demo Complete ===\n");
    println!("Demonstrated permission capabilities:");
    println!("1. ✓ Basic allow/deny - Simple allowlist/denylist");
    println!("2. ✓ Input modification - Sanitize dangerous inputs");
    println!("3. ✓ Context-aware - Use context for smart decisions");
    println!("\nPermissions are essential for:");
    println!("  • Security and access control");
    println!("  • Input validation and sanitization");
    println!("  • Compliance and audit trails");
    println!("  • User approval workflows");

    Ok(())
}
