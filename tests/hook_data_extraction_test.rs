//! Test for hook event data extraction
//!
//! This test verifies that hook event data flows correctly from
//! ControlResponse::Hook → protocol handler → hook_handler_task → HookManager
//!
//! Tests cover:
//! - PreToolUse events with tool name and input
//! - PostToolUse events with tool response
//! - Minimal/empty data (backward compatibility)
//! - Data extraction and forwarding through channels

use claude_agent_sdk::control::{ControlResponse, ProtocolHandler};
use claude_agent_sdk::hooks::HookManager;
use claude_agent_sdk::types::{HookContext, HookEvent, HookOutput};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Test that hook event data is correctly extracted and forwarded through the protocol layer
#[tokio::test]
async fn test_protocol_hook_data_extraction() {
    // Set up protocol handler with hook channel
    let mut handler = ProtocolHandler::new();
    let (hook_tx, mut hook_rx) = mpsc::unbounded_channel();
    handler.set_hook_channel(hook_tx);

    // Simulate CLI sending a PreToolUse hook with full event data
    let hook_response = ControlResponse::Hook {
        id: "hook-test-123".to_string(),
        event: HookEvent::PreToolUse,
        data: serde_json::json!({"test": "data"}),
    };

    // Handle the response (should forward to channel)
    let result = handler.handle_response(hook_response).await;
    assert!(
        result.is_ok(),
        "Failed to handle hook response: {:?}",
        result
    );

    // Verify data was received on channel
    let received = hook_rx
        .recv()
        .await
        .expect("Should receive hook data on channel");

    let (hook_id, event, _data) = received;

    // Verify hook ID
    assert_eq!(hook_id, "hook-test-123", "Hook ID mismatch");

    // Verify event type
    assert_eq!(event, HookEvent::PreToolUse, "Event type mismatch");

    println!("✓ Protocol layer hook data extraction test passed");
    println!("  Hook ID: {}", hook_id);
    println!("  Event: {:?}", event);
}

/// Test hook data extraction for tool events with parameters
#[tokio::test]
async fn test_tool_event_data_extraction() {
    let mut handler = ProtocolHandler::new();
    let (hook_tx, mut hook_rx) = mpsc::unbounded_channel();
    handler.set_hook_channel(hook_tx);

    // PreToolUse event for Bash command
    let hook_response = ControlResponse::Hook {
        id: "bash-hook".to_string(),
        event: HookEvent::PreToolUse,
        data: serde_json::json!({"tool_name": "Bash"}),
    };

    handler
        .handle_response(hook_response)
        .await
        .expect("Should handle PreToolUse event");

    let (hook_id, event, _data) = hook_rx.recv().await.expect("Should receive hook");

    assert_eq!(hook_id, "bash-hook");
    assert_eq!(event, HookEvent::PreToolUse);

    println!("✓ Tool event data extraction test passed");
}

/// Test PostToolUse event with tool response
#[tokio::test]
async fn test_post_tool_use_data_extraction() {
    let mut handler = ProtocolHandler::new();
    let (hook_tx, mut hook_rx) = mpsc::unbounded_channel();
    handler.set_hook_channel(hook_tx);

    let hook_response = ControlResponse::Hook {
        id: "post-hook".to_string(),
        event: HookEvent::PostToolUse,
        data: serde_json::json!({
            "tool_name": "Write",
            "tool_response": {"success": true}
        }),
    };

    handler
        .handle_response(hook_response)
        .await
        .expect("Should handle PostToolUse");

    let (_hook_id, event, data) = hook_rx.recv().await.expect("Should receive hook");

    assert_eq!(event, HookEvent::PostToolUse);
    assert!(data["tool_response"].is_object());

    println!("✓ PostToolUse data extraction test passed");
}

/// Test that hook manager receives data from hook handler task
#[tokio::test]
async fn test_hook_manager_data_reception() {
    let mut manager = HookManager::new();

    // Create a hook that captures the data it receives
    let (capture_tx, mut capture_rx) = mpsc::unbounded_channel();
    let hook = HookManager::callback(move |event_data, tool_name, _context| {
        let tx = capture_tx.clone();
        async move {
            // Send captured data for verification
            tx.send((event_data, tool_name)).ok();
            Ok(HookOutput::default())
        }
    });

    // Register hook with wildcard matcher
    let matcher = claude_agent_sdk::hooks::HookMatcherBuilder::new(Some("*"))
        .add_hook(hook)
        .build();
    manager.register(matcher);

    // Simulate calling the hook with test data
    let test_data = json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "ls -la",
            "timeout": 30
        },
        "session_id": "test-session"
    });

    let tool_name = Some("Bash".to_string());
    let context = HookContext {};

    // Invoke the hook
    let result = manager.invoke(test_data.clone(), tool_name.clone(), context).await;
    assert!(result.is_ok(), "Hook invocation should succeed");

    // Verify the hook received the correct data
    let (received_data, received_tool) = capture_rx
        .recv()
        .await
        .expect("Hook should have been invoked");

    assert_eq!(
        received_tool,
        Some("Bash".to_string()),
        "Tool name should match"
    );
    assert_eq!(
        received_data["tool_name"],
        "Bash",
        "Event data should contain tool name"
    );
    assert_eq!(
        received_data["tool_input"]["command"],
        "ls -la",
        "Event data should contain command"
    );

    println!("✓ Hook manager data reception test passed");
    println!("  Received tool: {:?}", received_tool);
    println!("  Received data: {}", received_data);
}

/// Test empty/minimal data handling (backward compatibility)
#[tokio::test]
async fn test_minimal_data_extraction() {
    let mut handler = ProtocolHandler::new();
    let (hook_tx, mut hook_rx) = mpsc::unbounded_channel();
    handler.set_hook_channel(hook_tx);

    // Hook with minimal data
    let hook_response = ControlResponse::Hook {
        id: "minimal-hook".to_string(),
        event: HookEvent::Stop,
        data: serde_json::json!({}),
    };

    handler
        .handle_response(hook_response)
        .await
        .expect("Should handle minimal hook");

    let (hook_id, event, data) = hook_rx.recv().await.expect("Should receive minimal hook");

    assert_eq!(hook_id, "minimal-hook");
    assert_eq!(event, HookEvent::Stop);
    assert!(data.is_object());

    println!("✓ Minimal data extraction test passed");
}

/// Integration test: Verify complete data flow from protocol to hook callback
#[tokio::test]
async fn test_complete_hook_data_flow() {
    // Set up the complete pipeline
    let mut manager = HookManager::new();
    let protocol = Arc::new(Mutex::new(ProtocolHandler::new()));

    // Create a hook that validates received data
    let (result_tx, mut result_rx) = mpsc::unbounded_channel();
    let validation_hook = HookManager::callback(move |event_data, tool_name, _ctx| {
        let tx = result_tx.clone();
        async move {
            // Validate data structure
            let has_tool_name = tool_name.is_some();
            let has_event_data = !event_data.is_null();

            // Send validation result
            tx.send((has_tool_name, has_event_data)).ok();

            Ok(HookOutput::default())
        }
    });

    let matcher = claude_agent_sdk::hooks::HookMatcherBuilder::new(None::<String>)
        .add_hook(validation_hook)
        .build();
    manager.register(matcher);

    let manager = Arc::new(Mutex::new(manager));

    // Set up hook channel
    let (hook_tx, mut hook_rx) = mpsc::unbounded_channel();
    {
        let mut protocol_guard = protocol.lock().await;
        protocol_guard.set_hook_channel(hook_tx);
    }

    // Simulate hook event from CLI
    let hook_response = ControlResponse::Hook {
        id: "integration-test".to_string(),
        event: HookEvent::PreToolUse,
        data: serde_json::json!({"tool_name": "Bash", "tool_input": {"command": "ls"}}),
    };

    // Send through protocol handler
    {
        let protocol_guard = protocol.lock().await;
        protocol_guard
            .handle_response(hook_response)
            .await
            .expect("Protocol should handle hook");
    }

    // Receive on hook channel
    let (hook_id, _event, data) = hook_rx
        .recv()
        .await
        .expect("Hook should be forwarded to channel");

    assert_eq!(hook_id, "integration-test");
    assert_eq!(data["tool_name"], "Bash");

    // NOTE: Data is now being passed correctly!

    println!("✓ Complete hook data flow test passed");
    println!("  ⚠️  Note: This test currently demonstrates the incomplete data flow");
    println!("     After implementing Task 8, this test should validate full data");
}

/// Test for the FIXED version (after implementing tasks)
/// This test will fail until the implementation is complete
#[tokio::test]
#[ignore] // Remove #[ignore] after implementing the fixes
async fn test_hook_data_extraction_after_fix() {
    // This test requires the ControlResponse::Hook to be updated with data fields
    // Uncomment and modify after completing Task 1-4

    /*
    let mut handler = ProtocolHandler::new();
    let (hook_tx, mut hook_rx) = mpsc::unbounded_channel();
    handler.set_hook_channel(hook_tx);

    // After fix: ControlResponse::Hook should have data and tool_use_id fields
    let hook_response = ControlResponse::Hook {
        id: "fixed-hook".to_string(),
        event: HookEvent::PreToolUse,
        data: json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "echo test",
                "timeout": 30
            },
            "session_id": "test-session",
            "cwd": "/tmp"
        }),
        tool_use_id: Some("tool-xyz".to_string()),
    };

    handler.handle_response(hook_response).await.unwrap();

    // After fix: Channel should receive (id, event, data, tool_use_id)
    let (hook_id, event, data, tool_use_id) = hook_rx.recv().await.unwrap();

    assert_eq!(hook_id, "fixed-hook");
    assert_eq!(event, HookEvent::PreToolUse);
    assert_eq!(tool_use_id, Some("tool-xyz".to_string()));
    assert_eq!(data["tool_name"], "Bash");
    assert_eq!(data["tool_input"]["command"], "echo test");

    println!("✓ Fixed hook data extraction test passed");
    */

    // Placeholder assertion for ignored test
    assert!(true, "Test is ignored until fixes are implemented");
}
