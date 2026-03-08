//! Test data flow without Claude CLI
//!
//! Run: cargo run --example test_data_flow

use claude_agent_sdk::control::{ControlResponse, ProtocolHandler};
use claude_agent_sdk::hooks::HookManager;
use claude_agent_sdk::types::{HookContext, HookEvent, HookOutput};
use serde_json::json;

#[tokio::main]
async fn main() {
    println!("=== Testing Hook Data Flow ===\n");

    // 1. Create a hook that logs what it receives
    let mut manager = HookManager::new();
    let hook = HookManager::callback(|data, tool_name, _ctx| async move {
        println!("✅ Hook received:");
        println!("   Tool: {:?}", tool_name);
        println!("   Data: {}\n", serde_json::to_string_pretty(&data).unwrap());
        Ok(HookOutput::default())
    });

    let matcher = claude_agent_sdk::hooks::HookMatcherBuilder::new(Some("*"))
        .add_hook(hook)
        .build();
    manager.register(matcher);

    // 2. Simulate CLI sending hook data
    let mut handler = ProtocolHandler::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    handler.set_hook_channel(tx);

    let cli_message = ControlResponse::Hook {
        id: "test-1".to_string(),
        event: HookEvent::PreToolUse,
        data: json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "ls -la",
                "timeout": 30
            },
            "session_id": "abc123"
        }),
    };

    // 3. Send through protocol
    handler.handle_response(cli_message).await.unwrap();

    // 4. Receive and invoke hook
    if let Some((hook_id, event, data)) = rx.recv().await {
        println!("📨 Protocol forwarded:");
        println!("   ID: {}", hook_id);
        println!("   Event: {:?}", event);

        // Extract tool name
        let tool_name = data.get("tool_name")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Invoke hook with data
        manager.invoke(data, tool_name, HookContext {}).await.unwrap();
    }

    println!("✅ Test complete! Hooks now receive actual data.");
}
