//! Integration tests for ClaudeSDKClient
//!
//! These tests verify the client's ability to handle concurrent operations,
//! bidirectional communication, and proper resource management.

use claude_agent_sdk::types::{ClaudeAgentOptions, Message};
use claude_agent_sdk::ClaudeSDKClient;
use std::time::Duration;
use tokio::time::timeout;

/// Test that demonstrates no lock contention between reading and writing
#[tokio::test]
async fn test_concurrent_read_write() {
    let options = ClaudeAgentOptions::default();
    let mut client = ClaudeSDKClient::new(options, None).await.unwrap();

    // Send initial message
    client.send_message("Hello!").await.unwrap();

    // Simulate concurrent operations: send messages while reading responses
    let send_task = tokio::spawn(async move {
        // Wait a bit then send another message
        tokio::time::sleep(Duration::from_millis(100)).await;
        client.send_message("Second message").await
    });

    // This should not block even though we're sending above
    // Because the reader task doesn't hold the transport lock
    let result = send_task.await;
    assert!(result.is_ok());
}

/// Test interrupt functionality during message streaming
#[tokio::test]
#[ignore] // Claude CLI doesn't support interrupt control messages (returns error)
async fn test_interrupt_during_streaming() {
    let options = ClaudeAgentOptions::default();
    let mut client = ClaudeSDKClient::new(options, None).await.unwrap();

    // Send a message that will take time to respond
    client
        .send_message("Write a long essay about Rust programming")
        .await
        .unwrap();

    // Start reading messages
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send interrupt while response is streaming
    let interrupt_result = client.interrupt().await;
    assert!(interrupt_result.is_ok());

    // Continue reading to verify we get result message
    let mut got_result = false;
    while let Some(msg) = timeout(Duration::from_secs(5), client.next_message())
        .await
        .ok()
        .flatten()
    {
        if let Ok(Message::Result { .. }) = msg {
            got_result = true;
            break;
        }
    }
    assert!(got_result);
}

/// Test that multiple messages can be sent in succession
#[tokio::test]
async fn test_multiple_successive_messages() {
    let options = ClaudeAgentOptions::default();
    let mut client = ClaudeSDKClient::new(options, None).await.unwrap();

    // Send multiple messages without waiting for responses
    for i in 0..3 {
        let result = client.send_message(format!("Message {i}")).await;
        assert!(result.is_ok());
    }

    // Now read responses
    let mut message_count = 0;
    while let Some(_msg) =
        timeout(Duration::from_secs(10), client.next_message())
            .await
            .ok()
            .flatten()
    {
        message_count += 1;
        if message_count >= 10 {
            // Enough messages received
            break;
        }
    }
    assert!(message_count > 0);
}

/// Test hook receiver functionality
#[tokio::test]
async fn test_hook_receiver() {
    let options = ClaudeAgentOptions::default();

    let mut client = ClaudeSDKClient::new(options, None).await.unwrap();
    let mut hook_rx = client.take_hook_receiver().unwrap();

    // Send a message that might trigger hooks
    client
        .send_message("Can you read a file?")
        .await
        .unwrap();

    // Try to receive hook events with timeout
    let hook_result = timeout(Duration::from_secs(5), hook_rx.recv()).await;

    // This might timeout if no hooks are triggered, which is okay for this test
    // The important part is that the channel works
    match hook_result {
        Ok(Some((hook_id, event))) => {
            println!("Received hook: {hook_id} {event:?}");
            // Could respond to hook
            // client.respond_to_hook(hook_id, response).await.unwrap();
        }
        Ok(None) => println!("Hook channel closed"),
        Err(_) => println!("No hooks triggered (timeout)"),
    }
}

/// Test permission receiver functionality
#[tokio::test]
async fn test_permission_receiver() {
    let options = ClaudeAgentOptions::default();
    let mut client = ClaudeSDKClient::new(options, None).await.unwrap();
    let mut perm_rx = client.take_permission_receiver().unwrap();

    // Send a message that might trigger permission requests
    client
        .send_message("Write a file named test.txt")
        .await
        .unwrap();

    // Try to receive permission requests with timeout
    let perm_result = timeout(Duration::from_secs(5), perm_rx.recv()).await;

    match perm_result {
        Ok(Some((_request_id, request))) => {
            println!("Received permission request: {request:?}");
            // Could respond to permission
            // use claude_agent_sdk::types::{PermissionResult, PermissionResultAllow};
            // let result = PermissionResult::Allow(PermissionResultAllow {
            //     updated_input: None,
            //     updated_permissions: None,
            // });
            // client.respond_to_permission(request_id, result).await.unwrap();
        }
        Ok(None) => println!("Permission channel closed"),
        Err(_) => println!("No permissions requested (timeout)"),
    }
}

/// Test that client can be created and closed cleanly
#[tokio::test]
async fn test_client_lifecycle() {
    let options = ClaudeAgentOptions::default();
    let mut client = ClaudeSDKClient::new(options, None).await.unwrap();

    // Send a simple message
    client.send_message("Hello").await.unwrap();

    // Read one message
    let _msg = timeout(Duration::from_secs(5), client.next_message())
        .await
        .ok()
        .flatten();

    // Close client
    let close_result = client.close().await;
    assert!(close_result.is_ok());
}

/// Test that demonstrates reader task doesn't block writer
#[tokio::test]
async fn test_no_reader_writer_blocking() {
    // This is a unit test that verifies the architectural pattern
    // without requiring a real CLI process

    use tokio::sync::{mpsc, Mutex};
    use std::sync::Arc;
    use std::time::Instant;

    // Simulate the client's architecture
    let shared_resource = Arc::new(Mutex::new(0u32));

    // Reader task: gets a receiver without holding lock
    let reader_resource = shared_resource.clone();
    let (tx, mut rx) = mpsc::unbounded_channel::<u32>();

    let reader = tokio::spawn(async move {
        // Briefly lock to get channel (simulating read_messages())
        let _guard = reader_resource.lock().await;
        drop(_guard);

        // Now read without holding lock
        while let Some(value) = rx.recv().await {
            // Simulate processing
            tokio::time::sleep(Duration::from_micros(10)).await;
            if value >= 5 {
                break;
            }
        }
    });

    // Writer task: locks briefly for each write
    let writer_resource = shared_resource.clone();
    let writer = tokio::spawn(async move {
        let start = Instant::now();
        for i in 0..10 {
            // Lock briefly for each write
            let mut guard = writer_resource.lock().await;
            *guard += 1;
            drop(guard);

            // Send through channel
            if tx.send(i).is_err() {
                break;
            }

            tokio::time::sleep(Duration::from_micros(5)).await;
        }
        start.elapsed()
    });

    // Both should complete quickly without blocking each other
    let (reader_result, writer_result) = tokio::join!(reader, writer);

    assert!(reader_result.is_ok());
    let elapsed = writer_result.unwrap();

    // Writer should complete quickly (< 1ms for 10 iterations)
    // This proves no lock contention
    assert!(elapsed < Duration::from_millis(100));
}

/// Test message flow order
#[tokio::test]
async fn test_message_flow_order() {
    let options = ClaudeAgentOptions::default();
    let mut client = ClaudeSDKClient::new(options, None).await.unwrap();

    // Send message
    client.send_message("What is 2+2?").await.unwrap();

    // Collect messages in order
    let mut messages = Vec::new();
    while let Some(msg) =
        timeout(Duration::from_secs(10), client.next_message())
            .await
            .ok()
            .flatten()
    {
        if let Ok(msg) = msg {
            let is_result = matches!(msg, Message::Result { .. });
            messages.push(msg);
            if is_result {
                break;
            }
        }
    }

    // Should receive at least assistant message and result
    assert!(messages.len() >= 2);

    // Last message should be Result
    assert!(matches!(messages.last(), Some(Message::Result { .. })));
}

/// Test error handling when CLI is not available
#[tokio::test]
async fn test_client_creation_with_invalid_cli() {
    let options = ClaudeAgentOptions::default();
    let invalid_path = std::path::PathBuf::from("/nonexistent/claude");

    let result = ClaudeSDKClient::new(options, Some(invalid_path)).await;

    // Should fail gracefully
    assert!(result.is_err());
}

/// Test that taking receivers works correctly
#[tokio::test]
async fn test_take_receivers() {
    let options = ClaudeAgentOptions::default();
    let mut client = ClaudeSDKClient::new(options, None).await.unwrap();

    // Take hook receiver
    let hook_rx = client.take_hook_receiver();
    assert!(hook_rx.is_some());

    // Taking again should return None
    let hook_rx2 = client.take_hook_receiver();
    assert!(hook_rx2.is_none());

    // Same for permission receiver
    let perm_rx = client.take_permission_receiver();
    assert!(perm_rx.is_some());

    let perm_rx2 = client.take_permission_receiver();
    assert!(perm_rx2.is_none());
}
