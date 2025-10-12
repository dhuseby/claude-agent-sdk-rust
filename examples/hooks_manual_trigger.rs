//! Manual hook trigger example
//!
//! This demonstrates how to manually trigger hooks for testing.
//! In production, hooks are triggered by the Claude CLI server.

use claude_agent_sdk::hooks::HookManager;
use claude_agent_sdk::types::{HookContext, HookOutput};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Manual Hook Trigger Demo ===\n");

    // Create a logging hook
    let logging_hook = HookManager::callback(|event_data, tool_name, _context| async move {
        println!("  [HOOK TRIGGERED]");
        println!("  Tool: {:?}", tool_name.unwrap_or_else(|| "unknown".to_string()));
        println!("  Data: {}", serde_json::to_string_pretty(&event_data)?);

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

    let mut manager = HookManager::default();
    manager.register(matcher);

    // Manually trigger hook with mock data
    println!("Triggering hook manually with Bash tool data...\n");

    let tool_data = json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "ls -la",
            "description": "List files"
        }
    });

    let context = HookContext {};
    let result = manager.invoke(
        tool_data.clone(),
        Some("Bash".to_string()),
        context
    ).await?;

    println!("\n✓ Hook executed successfully");
    println!("Result: {:#?}", result);

    println!("\n--- Triggering hook for Read tool ---\n");

    let read_data = json!({
        "tool_name": "Read",
        "tool_input": {
            "file_path": "/path/to/file.txt"
        }
    });

    let result2 = manager.invoke(
        read_data,
        Some("Read".to_string()),
        HookContext {}
    ).await?;

    println!("\n✓ Second hook executed");
    println!("Result: {:#?}", result2);

    Ok(())
}
