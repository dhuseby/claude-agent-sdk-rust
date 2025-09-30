//! Integration tests for the control protocol handler

use claude_agent_sdk::control::{
    ControlMessage, ControlRequest, ControlResponse, InitResponse, ProtocolHandler,
};
use claude_agent_sdk::types::{HookEvent, PermissionRequest, RequestId, ToolName};
use tokio::sync::mpsc;

#[tokio::test]
async fn test_protocol_initialization() {
    let handler = ProtocolHandler::new();

    // Create init request
    let init_req = handler.create_init_request();
    assert_eq!(init_req.protocol_version, "1.0");
    assert!(init_req.capabilities.bidirectional);
    assert!(init_req.capabilities.hooks);
    assert!(init_req.capabilities.permissions);
    assert!(init_req.capabilities.interrupts);

    // Simulate init response
    let init_response = InitResponse {
        protocol_version: "1.0".to_string(),
        cli_version: "1.0.0".to_string(),
        capabilities: claude_agent_sdk::control::protocol::ServerCapabilities {
            streaming: true,
            tools: true,
            mcp: true,
        },
        session_id: "test-session-123".to_string(),
    };

    let result = handler.handle_init_response(init_response);
    assert!(result.is_ok());
    assert!(handler.is_initialized());
}

#[tokio::test]
async fn test_protocol_version_mismatch() {
    let handler = ProtocolHandler::new();

    // Simulate init response with unsupported version
    let init_response = InitResponse {
        protocol_version: "2.0".to_string(),
        cli_version: "1.0.0".to_string(),
        capabilities: claude_agent_sdk::control::protocol::ServerCapabilities {
            streaming: true,
            tools: true,
            mcp: true,
        },
        session_id: "test-session-123".to_string(),
    };

    let result = handler.handle_init_response(init_response);
    assert!(result.is_err());
    assert!(!handler.is_initialized());
}

#[tokio::test]
async fn test_interrupt_request() {
    let handler = ProtocolHandler::new();
    handler.set_initialized(true);

    let request = handler.create_interrupt_request();
    match request {
        ControlRequest::Interrupt { id } => {
            assert!(id.as_str().starts_with("req-"));
        }
        _ => panic!("Expected Interrupt request"),
    }
}

#[tokio::test]
async fn test_send_message_request() {
    let handler = ProtocolHandler::new();
    handler.set_initialized(true);

    let request = handler.create_send_message_request("Hello!".to_string());
    match request {
        ControlRequest::SendMessage { id, content } => {
            assert!(id.as_str().starts_with("req-"));
            assert_eq!(content, "Hello!");
        }
        _ => panic!("Expected SendMessage request"),
    }
}

#[tokio::test]
async fn test_hook_response_creation() {
    let handler = ProtocolHandler::new();
    handler.set_initialized(true);

    let request = handler.create_hook_response(
        "hook-123".to_string(),
        serde_json::json!({"decision": "allow"}),
    );

    match request {
        ControlRequest::HookResponse {
            id,
            hook_id,
            response,
        } => {
            assert!(id.as_str().starts_with("req-"));
            assert_eq!(hook_id, "hook-123");
            assert_eq!(response["decision"], "allow");
        }
        _ => panic!("Expected HookResponse request"),
    }
}

#[tokio::test]
async fn test_permission_response_creation() {
    let handler = ProtocolHandler::new();
    handler.set_initialized(true);

    let request_id = RequestId::new("perm-req-1");
    let result = claude_agent_sdk::types::PermissionResult::Allow(
        claude_agent_sdk::types::PermissionResultAllow {
            updated_input: None,
            updated_permissions: None,
        },
    );

    let request = handler.create_permission_response(request_id.clone(), result);

    match request {
        ControlRequest::PermissionResponse {
            id,
            request_id: req_id,
            result: _,
        } => {
            assert!(id.as_str().starts_with("req-"));
            assert_eq!(req_id, request_id);
        }
        _ => panic!("Expected PermissionResponse request"),
    }
}

#[tokio::test]
async fn test_serialize_deserialize_interrupt() {
    let handler = ProtocolHandler::new();
    let request = handler.create_interrupt_request();
    let message = ControlMessage::Request(request);

    let serialized = handler.serialize_message(&message).unwrap();
    assert!(serialized.ends_with('\n'));

    let deserialized = handler.deserialize_message(serialized.trim()).unwrap();

    match deserialized {
        ControlMessage::Request(ControlRequest::Interrupt { .. }) => {}
        _ => panic!("Wrong message type after deserialization"),
    }
}

#[tokio::test]
async fn test_serialize_deserialize_response() {
    let handler = ProtocolHandler::new();
    let response = ControlResponse::Success {
        id: RequestId::new("req-1"),
        data: Some(serde_json::json!({"status": "ok"})),
    };
    let message = ControlMessage::Response(response);

    let serialized = handler.serialize_message(&message).unwrap();
    let deserialized = handler.deserialize_message(serialized.trim()).unwrap();

    match deserialized {
        ControlMessage::Response(ControlResponse::Success { id, data }) => {
            assert_eq!(id.as_str(), "req-1");
            assert!(data.is_some());
        }
        _ => panic!("Wrong message type after deserialization"),
    }
}

#[tokio::test]
async fn test_hook_channel_integration() {
    let mut handler = ProtocolHandler::new();
    let (hook_tx, mut hook_rx) = mpsc::unbounded_channel();
    handler.set_hook_channel(hook_tx);

    // Simulate receiving a hook event
    let hook_response = ControlResponse::Hook {
        id: "hook-123".to_string(),
        event: HookEvent::PreToolUse,
    };

    handler.handle_response(hook_response).await.unwrap();

    // Verify hook was received
    let (hook_id, event) = hook_rx.recv().await.unwrap();
    assert_eq!(hook_id, "hook-123");
    assert_eq!(event, HookEvent::PreToolUse);
}

#[tokio::test]
async fn test_permission_channel_integration() {
    let mut handler = ProtocolHandler::new();
    let (permission_tx, mut permission_rx) = mpsc::unbounded_channel();
    handler.set_permission_channel(permission_tx);

    // Simulate receiving a permission request
    let perm_request = PermissionRequest {
        tool_name: ToolName::new("Bash"),
        tool_input: serde_json::json!({"command": "ls"}),
        context: claude_agent_sdk::types::ToolPermissionContext {
            suggestions: vec![],
        },
    };

    let perm_response = ControlResponse::Permission {
        id: RequestId::new("perm-1"),
        request: perm_request.clone(),
    };

    handler.handle_response(perm_response).await.unwrap();

    // Verify permission request was received
    let (req_id, request) = permission_rx.recv().await.unwrap();
    assert_eq!(req_id.as_str(), "perm-1");
    assert_eq!(request.tool_name, ToolName::new("Bash"));
}

#[tokio::test]
async fn test_request_id_uniqueness() {
    let handler = ProtocolHandler::new();
    handler.set_initialized(true);

    let req1 = handler.create_interrupt_request();
    let req2 = handler.create_interrupt_request();
    let req3 = handler.create_send_message_request("test".to_string());

    let id1 = match req1 {
        ControlRequest::Interrupt { id } => id,
        _ => panic!(),
    };
    let id2 = match req2 {
        ControlRequest::Interrupt { id } => id,
        _ => panic!(),
    };
    let id3 = match req3 {
        ControlRequest::SendMessage { id, .. } => id,
        _ => panic!(),
    };

    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);
}

#[tokio::test]
async fn test_error_response_handling() {
    let handler = ProtocolHandler::new();
    handler.set_initialized(true);

    // Send a request
    let request = handler.create_interrupt_request();
    let request_id = match &request {
        ControlRequest::Interrupt { id } => id.clone(),
        _ => panic!(),
    };

    let _response_rx = handler.send_request(request).await.unwrap();

    // Simulate error response
    let error_response = ControlResponse::Error {
        id: request_id,
        message: "Test error".to_string(),
        code: Some("TEST_ERROR".to_string()),
    };

    let result = handler.handle_response(error_response).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_request_without_initialization() {
    let handler = ProtocolHandler::new();
    assert!(!handler.is_initialized());

    let request = handler.create_interrupt_request();
    let result = handler.send_request(request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multiple_hook_events() {
    let mut handler = ProtocolHandler::new();
    let (hook_tx, mut hook_rx) = mpsc::unbounded_channel();
    handler.set_hook_channel(hook_tx);

    // Send multiple hook events
    let events = [HookEvent::PreToolUse,
        HookEvent::PostToolUse,
        HookEvent::Stop];

    for (i, event) in events.iter().enumerate() {
        let response = ControlResponse::Hook {
            id: format!("hook-{i}"),
            event: *event,
        };
        handler.handle_response(response).await.unwrap();
    }

    // Verify all hooks received
    for (i, expected_event) in events.iter().enumerate() {
        let (hook_id, event) = hook_rx.recv().await.unwrap();
        assert_eq!(hook_id, format!("hook-{i}"));
        assert_eq!(event, *expected_event);
    }
}

#[tokio::test]
async fn test_init_message_serialization() {
    let handler = ProtocolHandler::new();
    let init_req = handler.create_init_request();
    let message = ControlMessage::Init(init_req);

    let serialized = handler.serialize_message(&message).unwrap();
    let deserialized = handler.deserialize_message(serialized.trim()).unwrap();

    match deserialized {
        ControlMessage::Init(init) => {
            assert_eq!(init.protocol_version, "1.0");
            assert!(init.capabilities.bidirectional);
        }
        _ => panic!("Wrong message type"),
    }
}

#[tokio::test]
async fn test_init_response_message_serialization() {
    let handler = ProtocolHandler::new();
    let init_response = InitResponse {
        protocol_version: "1.0".to_string(),
        cli_version: "1.0.0".to_string(),
        capabilities: claude_agent_sdk::control::protocol::ServerCapabilities {
            streaming: true,
            tools: true,
            mcp: true,
        },
        session_id: "test-session".to_string(),
    };
    let message = ControlMessage::InitResponse(init_response);

    let serialized = handler.serialize_message(&message).unwrap();
    let deserialized = handler.deserialize_message(serialized.trim()).unwrap();

    match deserialized {
        ControlMessage::InitResponse(resp) => {
            assert_eq!(resp.protocol_version, "1.0");
            assert_eq!(resp.cli_version, "1.0.0");
            assert_eq!(resp.session_id, "test-session");
        }
        _ => panic!("Wrong message type"),
    }
}
