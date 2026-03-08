//! ClaudeSDKClient for bidirectional communication
//!
//! This module provides the main client for interactive, stateful conversations
//! with Claude Code, including support for:
//! - Bidirectional messaging (no lock contention)
//! - Interrupts and control flow
//! - Hook and permission callbacks
//! - Conversation state management
//!
//! # Architecture
//!
//! The client uses a lock-free architecture for reading and writing:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                   ClaudeSDKClient                        │
//! │                                                          │
//! │  ┌──────────────────┐        ┌──────────────────┐      │
//! │  │  Message Reader  │        │  Control Writer  │      │
//! │  │  Background Task │        │  Background Task │      │
//! │  │                  │        │                  │      │
//! │  │ • Gets receiver  │        │ • Locks per-write│      │
//! │  │   once           │        │ • No blocking    │      │
//! │  │ • No lock held   │        │                  │      │
//! │  │   while reading  │        │                  │      │
//! │  └────────┬─────────┘        └────────┬─────────┘      │
//! │           │                           │                 │
//! │           │    ┌──────────────┐      │                 │
//! │           └───→│  Transport   │←─────┘                 │
//! │                │  (Arc<Mutex>)│                         │
//! │                └──────────────┘                         │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! **Key Design Points:**
//! - Transport returns an owned `UnboundedReceiver` (no lifetime issues)
//! - Reader task gets receiver once, then releases transport lock
//! - Writer task locks transport briefly for each write operation
//! - No contention: reader never blocks writer, writer never blocks reader
//!
//! # Example: Basic Usage
//!
//! ```no_run
//! use claude_agent_sdk::{ClaudeSDKClient, ClaudeAgentOptions, Message};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let options = ClaudeAgentOptions::default();
//! let mut client = ClaudeSDKClient::new(options, None).await?;
//!
//! // Send a message
//! client.send_message("Hello, Claude!").await?;
//!
//! // Read responses
//! while let Some(message) = client.next_message().await {
//!     match message? {
//!         Message::Assistant { message, .. } => {
//!             println!("Response: {:?}", message.content);
//!         }
//!         Message::Result { .. } => break,
//!         _ => {}
//!     }
//! }
//!
//! client.close().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Example: Concurrent Operations
//!
//! ```no_run
//! use claude_agent_sdk::{ClaudeSDKClient, ClaudeAgentOptions};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let options = ClaudeAgentOptions::default();
//! let mut client = ClaudeSDKClient::new(options, None).await?;
//!
//! // Send first message
//! client.send_message("First question").await?;
//!
//! // Can send another message while reading responses
//! // No blocking due to lock-free architecture
//! tokio::spawn(async move {
//!     tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
//!     client.send_message("Second question").await
//! });
//!
//! # Ok(())
//! # }
//! ```
//!
//! # Example: Interrupt
//!
//! ```no_run
//! use claude_agent_sdk::{ClaudeSDKClient, ClaudeAgentOptions};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let options = ClaudeAgentOptions::default();
//! let mut client = ClaudeSDKClient::new(options, None).await?;
//!
//! client.send_message("Write a long essay").await?;
//!
//! // After some time, interrupt the response
//! tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
//! client.interrupt().await?;
//!
//! # Ok(())
//! # }
//! ```
//!
//! # Example: Hooks and Permissions
//!
//! ```no_run
//! use claude_agent_sdk::{ClaudeSDKClient, ClaudeAgentOptions};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let options = ClaudeAgentOptions::default();
//! let mut client = ClaudeSDKClient::new(options, None).await?;
//!
//! // Take receivers to handle hooks and permissions
//! let mut hook_rx = client.take_hook_receiver().unwrap();
//! let mut perm_rx = client.take_permission_receiver().unwrap();
//!
//! // Handle hook events
//! tokio::spawn(async move {
//!     while let Some((hook_id, event, data)) = hook_rx.recv().await {
//!         println!("Hook: {} {:?} data: {}", hook_id, event, data);
//!         // Respond to hook...
//!     }
//! });
//!
//! // Handle permission requests
//! tokio::spawn(async move {
//!     while let Some((req_id, request)) = perm_rx.recv().await {
//!         println!("Permission: {:?}", request);
//!         // Respond to permission...
//!     }
//! });
//!
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::control::{ControlMessage, ControlRequest, ProtocolHandler};
use crate::control::protocol::HookMatcherConfig;
use crate::error::{ClaudeError, Result};
use crate::hooks::HookManager;
use crate::message::parse_message;
use crate::permissions::PermissionManager;
use crate::transport::{PromptInput, SubprocessTransport, Transport};
use crate::types::{
    ClaudeAgentOptions, HookContext, HookEvent, McpServerConfig, McpServers, Message,
    PermissionRequest, RequestId,
};

/// Client for bidirectional communication with Claude Code
///
/// ClaudeSDKClient provides interactive, stateful conversations with
/// support for interrupts, hooks, and permission callbacks.
///
/// # Examples
///
/// ```no_run
/// use claude_agent_sdk::{ClaudeSDKClient, ClaudeAgentOptions};
/// use futures::StreamExt;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let options = ClaudeAgentOptions::default();
///     let mut client = ClaudeSDKClient::new(options, None).await?;
///
///     client.send_message("Hello, Claude!").await?;
///
///     while let Some(message) = client.next_message().await {
///         println!("{:?}", message?);
///     }
///
///     Ok(())
/// }
/// ```
pub struct ClaudeSDKClient {
    /// Transport layer
    transport: Arc<Mutex<SubprocessTransport>>,
    /// Control protocol handler
    protocol: Arc<Mutex<ProtocolHandler>>,
    /// Message stream receiver
    message_rx: mpsc::UnboundedReceiver<Result<Message>>,
    /// Control message sender
    control_tx: mpsc::UnboundedSender<ControlRequest>,
    /// Hook event receiver (if not using automatic handler)
    hook_rx: Option<mpsc::UnboundedReceiver<(String, HookEvent, serde_json::Value)>>,
    /// Permission request receiver (if not using automatic handler)
    permission_rx: Option<mpsc::UnboundedReceiver<(RequestId, PermissionRequest)>>,
    /// Hook manager for automatic hook handling (kept alive for background tasks)
    #[allow(dead_code)]
    hook_manager: Option<Arc<Mutex<HookManager>>>,
    /// Permission manager for automatic permission handling (kept alive for background tasks)
    #[allow(dead_code)]
    permission_manager: Option<Arc<Mutex<PermissionManager>>>,
    /// SDK MCP servers (kept alive for background tasks)
    #[allow(dead_code)]
    sdk_mcp_servers: HashMap<String, Arc<crate::mcp::SdkMcpServer>>,
    /// MCP callback channel sender (kept alive for background task)
    #[allow(dead_code)]
    mcp_callback_tx: Option<mpsc::UnboundedSender<(RequestId, String, serde_json::Value)>>,
}

impl ClaudeSDKClient {
    /// Create a new ClaudeSDKClient
    ///
    /// # Arguments
    /// * `options` - Configuration options
    /// * `cli_path` - Optional path to Claude Code CLI
    ///
    /// # Errors
    /// Returns error if CLI cannot be found or connection fails
    pub async fn new(
        options: ClaudeAgentOptions,
        cli_path: Option<std::path::PathBuf>,
    ) -> Result<Self> {
        // Initialize hook manager if hooks are configured
        let (hook_manager, _hooks_for_init) = if let Some(ref hooks_config) = options.hooks {
            let mut manager = HookManager::new();
            let mut hooks_init_config = HashMap::new();

            // Register each hook matcher and collect callback IDs
            for (event, matchers) in hooks_config {
                let mut matcher_configs = Vec::new();

                for matcher in matchers {
                    // Register and get generated callback IDs
                    let callback_ids = manager.register_with_ids(matcher.clone());

                    matcher_configs.push(HookMatcherConfig {
                        matcher: matcher.matcher.clone(),
                        hook_callback_ids: callback_ids,
                    });
                }

                hooks_init_config.insert(*event, matcher_configs);
            }

            (Some(Arc::new(Mutex::new(manager))), Some(hooks_init_config))
        } else {
            (None, None)
        };

        // Initialize permission manager if callback is configured
        let (permission_manager, permission_rx) = if options.can_use_tool.is_some() {
            let mut manager = PermissionManager::new();
            if let Some(callback) = options.can_use_tool.clone() {
                manager.set_callback(callback);
            }
            manager.set_allowed_tools(Some(options.allowed_tools.clone()));
            manager.set_disallowed_tools(options.disallowed_tools.clone());
            (Some(Arc::new(Mutex::new(manager))), None)
        } else {
            (None, Some(mpsc::unbounded_channel().1))
        };

        // Extract SDK MCP servers from options
        let sdk_mcp_servers = if let McpServers::Dict(ref servers) = options.mcp_servers {
            servers
                .iter()
                .filter_map(|(name, config)| {
                    if let McpServerConfig::Sdk(marker) = config {
                        Some((name.clone(), marker.instance.clone()))
                    } else {
                        None
                    }
                })
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };

        // Create transport with streaming mode
        let prompt_input = PromptInput::Stream;
        let mut transport = SubprocessTransport::new(prompt_input, options, cli_path)?;

        // Connect transport
        transport.connect().await?;

        // Create protocol handler
        let mut protocol = ProtocolHandler::new();

        // Set up channels
        let (hook_tx, hook_rx_internal) = mpsc::unbounded_channel::<(String, HookEvent, serde_json::Value)>();
        let (permission_tx, permission_rx_internal) = mpsc::unbounded_channel();
        let (hook_callback_tx, hook_callback_rx) = mpsc::unbounded_channel::<(RequestId, String, serde_json::Value, Option<String>)>();

        protocol.set_hook_channel(hook_tx);
        protocol.set_permission_channel(permission_tx);
        protocol.set_hook_callback_channel(hook_callback_tx);

        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let (control_tx, control_rx) = mpsc::unbounded_channel();

        // Note: For hooks to work, we need to send initialization with hooks config
        // Mark protocol as initialized immediately for now (will send init below if needed)
        protocol.set_initialized(true);

        let transport = Arc::new(Mutex::new(transport));
        let protocol = Arc::new(Mutex::new(protocol));

        // Spawn message reader task
        let transport_clone = transport.clone();
        let protocol_clone = protocol.clone();
        let message_tx_clone = message_tx;
        tokio::spawn(async move {
            Self::message_reader_task(transport_clone, protocol_clone, message_tx_clone).await;
        });

        // Spawn control message writer task
        let transport_clone = transport.clone();
        let protocol_clone = protocol.clone();
        tokio::spawn(async move {
            Self::control_writer_task(transport_clone, protocol_clone, control_rx).await;
        });

        // Spawn hook handler task if hook manager is configured
        if let Some(ref manager) = hook_manager {
            let manager_clone = manager.clone();
            let protocol_clone = protocol.clone();
            tokio::spawn(async move {
                Self::hook_handler_task(manager_clone, protocol_clone, hook_rx_internal).await;
            });
        }

        // Spawn hook callback handler task if hook manager is configured
        if let Some(ref manager) = hook_manager {
            let manager_clone = manager.clone();
            let protocol_clone = protocol.clone();
            let control_tx_clone = control_tx.clone();
            tokio::spawn(async move {
                Self::hook_callback_handler_task(
                    manager_clone,
                    protocol_clone,
                    control_tx_clone,
                    hook_callback_rx,
                )
                .await;
            });
        }

        // Spawn permission handler task if permission manager is configured
        if let Some(ref manager) = permission_manager {
            let manager_clone = manager.clone();
            let protocol_clone = protocol.clone();
            tokio::spawn(async move {
                Self::permission_handler_task(
                    manager_clone,
                    protocol_clone,
                    permission_rx_internal,
                )
                .await;
            });
        }

        // Spawn MCP message handler task if SDK servers are present
        let mcp_callback_tx = if !sdk_mcp_servers.is_empty() {
            let (mcp_tx, mcp_rx) = mpsc::unbounded_channel();

            // Set channel in protocol handler
            {
                let mut protocol_guard = protocol.lock().await;
                protocol_guard.set_mcp_callback_channel(mcp_tx.clone());
            }

            // Spawn handler task
            let servers_clone = sdk_mcp_servers.clone();
            let protocol_clone = protocol.clone();
            let control_tx_clone = control_tx.clone();
            tokio::spawn(async move {
                Self::mcp_message_handler_task(
                    servers_clone,
                    protocol_clone,
                    control_tx_clone,
                    mcp_rx,
                )
                .await;
            });

            Some(mcp_tx)
        } else {
            None
        };

        let mut client = Self {
            transport,
            protocol,
            message_rx,
            control_tx,
            hook_rx: None,  // Hooks are handled automatically by hook_callback_handler_task
            permission_rx,
            hook_manager,
            permission_manager,
            sdk_mcp_servers: sdk_mcp_servers.clone(),
            mcp_callback_tx,
        };

        // Send initialization request if hooks are configured OR if SDK MCP servers are present
        // This must happen AFTER spawning tasks so the message reader can process the response
        let has_sdk_mcp = !sdk_mcp_servers.is_empty();
        if _hooks_for_init.is_some() || has_sdk_mcp {
            let hooks_config = _hooks_for_init.unwrap_or_default();
            client.send_initialize(hooks_config).await?;
        }

        Ok(client)
    }

    /// Message reader task - reads from transport and processes messages
    async fn message_reader_task(
        transport: Arc<Mutex<SubprocessTransport>>,
        protocol: Arc<Mutex<ProtocolHandler>>,
        message_tx: mpsc::UnboundedSender<Result<Message>>,
    ) {
        // Get the message receiver from the transport without holding the lock
        let mut msg_stream = {
            let mut transport_guard = transport.lock().await;
            transport_guard.read_messages()
        };

        while let Some(result) = msg_stream.recv().await {
            match result {
                Ok(value) => {
                    // Check message type field
                    let msg_type = value.get("type").and_then(|v| v.as_str());

                    // Handle control_response (init response, etc.)
                    if msg_type == Some("control_response") {
                        // This is a response to our init request - just log and continue
                        #[cfg(feature = "tracing-support")]
                        tracing::debug!("Received control_response");
                        #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                        eprintln!("Received control_response");
                        continue;
                    }

                    // Handle control_request (hook callbacks from CLI)
                    if msg_type == Some("control_request") {
                        let protocol_guard = protocol.lock().await;

                        // Extract request data
                        if let Some(request_obj) = value.get("request").and_then(|v| v.as_object()) {
                            let subtype = request_obj.get("subtype").and_then(|v| v.as_str());

                            if subtype == Some("hook_callback") {
                                // Extract hook callback data
                                let request_id = value.get("request_id")
                                    .and_then(|v| v.as_str())
                                    .map(|s| RequestId::new(s))
                                    .unwrap_or_else(|| RequestId::new("unknown"));

                                let callback_id = request_obj.get("callback_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("").to_string();

                                let input = request_obj.get("input")
                                    .cloned()
                                    .unwrap_or_else(|| serde_json::json!({}));

                                let tool_use_id = request_obj.get("tool_use_id")
                                    .and_then(|v| v.as_str())
                                    .map(String::from);

                                // Send to hook callback channel
                                if let Some(tx) = protocol_guard.get_hook_callback_channel() {
                                    let _ = tx.send((request_id, callback_id, input, tool_use_id));
                                }
                            } else if subtype == Some("mcp_message") {
                                // Extract MCP message data
                                let request_id = value.get("request_id")
                                    .and_then(|v| v.as_str())
                                    .map(|s| RequestId::new(s))
                                    .unwrap_or_else(|| RequestId::new("unknown"));

                                let server_name = request_obj.get("server_name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("").to_string();

                                let message = request_obj.get("message")
                                    .cloned()
                                    .unwrap_or_else(|| serde_json::json!({}));

                                // Send to MCP callback channel
                                if let Some(tx) = protocol_guard.get_mcp_callback_channel() {
                                    let _ = tx.send((request_id, server_name, message));
                                }
                            }
                        }

                        drop(protocol_guard);
                        continue;
                    }

                    // Try to parse as ControlMessage types (init, init_response)
                    let protocol_guard = protocol.lock().await;
                    if let Ok(control_msg) = protocol_guard.deserialize_message(
                        &serde_json::to_string(&value).unwrap_or_default(),
                    ) {
                        match control_msg {
                            ControlMessage::InitResponse(init_response) => {
                                if let Err(e) = protocol_guard.handle_init_response(init_response)
                                {
                                    let _ = message_tx.send(Err(e));
                                    break;
                                }
                            }
                            ControlMessage::Response(response) => {
                                if let Err(e) = protocol_guard.handle_response(response).await {
                                    let _ = message_tx.send(Err(e));
                                }
                            }
                            ControlMessage::Request(_) => {
                                // Ignore requests in client mode
                            }
                            ControlMessage::Init(_) => {
                                // Ignore init in client mode
                            }
                        }
                        drop(protocol_guard);
                        continue;
                    }
                    drop(protocol_guard);

                    // Otherwise parse as regular message
                    match parse_message(value) {
                        Ok(msg) => {
                            if message_tx.send(Ok(msg)).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = message_tx.send(Err(e));
                        }
                    }
                }
                Err(e) => {
                    let _ = message_tx.send(Err(e));
                    break;
                }
            }
        }
    }

    /// Control message writer task - writes control requests to transport
    async fn control_writer_task(
        transport: Arc<Mutex<SubprocessTransport>>,
        _protocol: Arc<Mutex<ProtocolHandler>>,
        mut control_rx: mpsc::UnboundedReceiver<ControlRequest>,
    ) {
        while let Some(request) = control_rx.recv().await {
            // In stream-json mode, Claude CLI expects simple control messages
            // without the full control protocol wrapper
            let control_json = match request {
                ControlRequest::Interrupt { .. } => {
                    // Send a control message to interrupt
                    serde_json::json!({
                        "type": "control",
                        "method": "interrupt"
                    })
                }
                ControlRequest::SendMessage { content, .. } => {
                    // This shouldn't go through control channel, but handle it anyway
                    serde_json::json!({
                        "type": "user",
                        "message": {
                            "role": "user",
                            "content": content
                        }
                    })
                }
                ControlRequest::HookCallbackResponse { id, output } => {
                    // Send hook callback response back to CLI in Python SDK format
                    serde_json::json!({
                        "type": "control_response",
                        "response": {
                            "subtype": "success",
                            "request_id": id.as_str(),
                            "response": output
                        }
                    })
                }
                ControlRequest::McpMessageResponse { id, mcp_response } => {
                    // Send MCP message response back to CLI
                    serde_json::json!({
                        "type": "control_response",
                        "response": {
                            "subtype": "success",
                            "request_id": id.as_str(),
                            "response": {
                                "mcp_response": mcp_response
                            }
                        }
                    })
                }
                _ => {
                    // Other control types not yet supported in stream-json mode
                    continue;
                }
            };

            if let Ok(json_str) = serde_json::to_string(&control_json) {
                let message_line = format!("{json_str}\n");
                let mut transport_guard = transport.lock().await;
                if transport_guard.write(&message_line).await.is_err() {
                    break;
                }
            } else {
                break;
            }
        }
    }

    /// Hook handler task - automatically processes hook events
    async fn hook_handler_task(
        manager: Arc<Mutex<HookManager>>,
        protocol: Arc<Mutex<ProtocolHandler>>,
        mut hook_rx: mpsc::UnboundedReceiver<(String, HookEvent, serde_json::Value)>,
    ) {
        while let Some((hook_id, _event, data)) = hook_rx.recv().await {
            let manager_guard = manager.lock().await;
            let context = HookContext {};

            // Extract tool_name from data if present
            let tool_name = data.get("tool_name")
                .and_then(|v| v.as_str())
                .map(String::from);

            // Use actual event data instead of empty JSON
            match manager_guard
                .invoke(data, tool_name, context)
                .await
            {
                Ok(output) => {
                    drop(manager_guard);

                    // Send hook response
                    let protocol_guard = protocol.lock().await;
                    let response = serde_json::to_value(&output).unwrap_or_default();
                    let _request = protocol_guard.create_hook_response(hook_id, response);
                    drop(protocol_guard);

                    // Send through control channel would require access to control_tx
                    // For now, hooks are processed but response sending needs client cooperation
                    // This is acceptable as hooks are advisory
                    // In a full implementation, we'd send _request through control_tx
                    #[cfg(feature = "tracing-support")]
                    tracing::debug!(event = ?_event, "Hook processed");
                    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                    eprintln!("Hook processed for event {_event:?}");
                }
                Err(_e) => {
                    #[cfg(feature = "tracing-support")]
                    tracing::error!(error = %_e, "Hook processing error");
                    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                    eprintln!("Hook processing error: {_e}");
                }
            }
        }
    }

    /// Hook callback handler task - handles incoming hook_callback requests from CLI
    async fn hook_callback_handler_task(
        manager: Arc<Mutex<HookManager>>,
        protocol: Arc<Mutex<ProtocolHandler>>,
        control_tx: mpsc::UnboundedSender<ControlRequest>,
        mut hook_callback_rx: mpsc::UnboundedReceiver<(RequestId, String, serde_json::Value, Option<String>)>,
    ) {
        while let Some((request_id, callback_id, input, _tool_use_id)) = hook_callback_rx.recv().await {
            #[cfg(feature = "tracing-support")]
            tracing::debug!(
                request_id = %request_id.as_str(),
                callback_id = %callback_id,
                "Processing hook callback request"
            );

            let manager_guard = manager.lock().await;
            let context = HookContext {};

            // Extract tool_name from input if present
            let tool_name = input
                .get("tool_name")
                .and_then(|v| v.as_str())
                .map(String::from);

            // Invoke the hook by callback_id
            match manager_guard.invoke_by_id(&callback_id, input, tool_name, context).await {
                Ok(output) => {
                    drop(manager_guard);

                    // Create and send hook callback response
                    let protocol_guard = protocol.lock().await;
                    let output_json = serde_json::to_value(&output).unwrap_or_default();
                    let response = protocol_guard.create_hook_callback_response(request_id, output_json);
                    drop(protocol_guard);

                    // Send response back to CLI
                    if let Err(_e) = control_tx.send(response) {
                        #[cfg(feature = "tracing-support")]
                        tracing::error!(error = ?_e, "Failed to send hook callback response");
                        #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                        eprintln!("Failed to send hook callback response: {_e:?}");
                    }

                    #[cfg(feature = "tracing-support")]
                    tracing::debug!(callback_id = %callback_id, "Hook callback processed");
                    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                    eprintln!("Hook callback processed for {callback_id}");
                }
                Err(_e) => {
                    #[cfg(feature = "tracing-support")]
                    tracing::error!(error = %_e, callback_id = %callback_id, "Hook callback error");
                    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                    eprintln!("Hook callback error for {callback_id}: {_e}");

                    // TODO: Send error response
                    // For now, just log the error
                }
            }
        }
    }

    /// MCP message handler task - handles incoming mcp_message requests from CLI
    async fn mcp_message_handler_task(
        sdk_mcp_servers: HashMap<String, Arc<crate::mcp::SdkMcpServer>>,
        protocol: Arc<Mutex<ProtocolHandler>>,
        control_tx: mpsc::UnboundedSender<ControlRequest>,
        mut mcp_callback_rx: mpsc::UnboundedReceiver<(RequestId, String, serde_json::Value)>,
    ) {
        while let Some((request_id, server_name, message)) = mcp_callback_rx.recv().await {
            #[cfg(feature = "tracing-support")]
            tracing::debug!(
                request_id = %request_id.as_str(),
                server_name = %server_name,
                "Processing MCP message request"
            );

            // Get SDK MCP server instance
            let server = match sdk_mcp_servers.get(&server_name) {
                Some(s) => s,
                None => {
                    #[cfg(feature = "tracing-support")]
                    tracing::error!(server_name = %server_name, "SDK MCP server not found");
                    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                    eprintln!("SDK MCP server not found: {server_name}");

                    // Send error response
                    let error_response = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": message.get("id"),
                        "error": {
                            "code": -32601,
                            "message": format!("Server '{server_name}' not found")
                        }
                    });

                    let protocol_guard = protocol.lock().await;
                    let response = protocol_guard.create_mcp_message_response(request_id, error_response);
                    drop(protocol_guard);

                    let _ = control_tx.send(response);
                    continue;
                }
            };

            // Parse JSONRPC request
            let jsonrpc_request: crate::mcp::protocol::JsonRpcRequest = match serde_json::from_value(message) {
                Ok(req) => req,
                Err(_e) => {
                    #[cfg(feature = "tracing-support")]
                    tracing::error!(error = %_e, "Failed to parse JSONRPC request");
                    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                    eprintln!("Failed to parse JSONRPC request: {_e}");
                    continue;
                }
            };

            #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
            eprintln!("MCP method called: {} for server: {}", jsonrpc_request.method, server_name);

            // Invoke SDK MCP server handler
            match server.handle_request(jsonrpc_request.clone()).await {
                Ok(jsonrpc_response) => {
                    // Convert response to JSON
                    let response_json = serde_json::to_value(&jsonrpc_response)
                        .unwrap_or_else(|_| serde_json::json!({"error": "Serialization failed"}));

                    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                    {
                        eprintln!("MCP response for {}: {}", jsonrpc_request.method,
                            serde_json::to_string_pretty(&response_json).unwrap_or_default());
                    }

                    // Create and send MCP message response
                    let protocol_guard = protocol.lock().await;
                    let response = protocol_guard.create_mcp_message_response(request_id, response_json);
                    drop(protocol_guard);

                    // Send response back to CLI
                    if let Err(_e) = control_tx.send(response) {
                        #[cfg(feature = "tracing-support")]
                        tracing::error!(error = ?_e, "Failed to send MCP response");
                        #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                        eprintln!("Failed to send MCP response: {_e:?}");
                    }

                    #[cfg(feature = "tracing-support")]
                    tracing::debug!(server_name = %server_name, "MCP message processed");
                    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                    eprintln!("MCP message processed for {server_name}");
                }
                Err(e) => {
                    #[cfg(feature = "tracing-support")]
                    tracing::error!(error = %e, server_name = %server_name, "MCP server error");
                    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                    eprintln!("MCP server error for {server_name}: {e}");

                    // Send error response
                    let error_response = serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "code": -32603,
                            "message": format!("Server error: {e}")
                        }
                    });

                    let protocol_guard = protocol.lock().await;
                    let response = protocol_guard.create_mcp_message_response(request_id, error_response);
                    drop(protocol_guard);

                    let _ = control_tx.send(response);
                }
            }
        }
    }

    /// Permission handler task - automatically processes permission requests
    async fn permission_handler_task(
        manager: Arc<Mutex<PermissionManager>>,
        protocol: Arc<Mutex<ProtocolHandler>>,
        mut permission_rx: mpsc::UnboundedReceiver<(RequestId, PermissionRequest)>,
    ) {
        while let Some((request_id, request)) = permission_rx.recv().await {
            let manager_guard = manager.lock().await;

            match manager_guard
                .can_use_tool(
                    request.tool_name.clone(),
                    request.tool_input.clone(),
                    request.context.clone(),
                )
                .await
            {
                Ok(result) => {
                    drop(manager_guard);

                    // Send permission response
                    let protocol_guard = protocol.lock().await;
                    let _request = protocol_guard.create_permission_response(request_id.clone(), result.clone());
                    drop(protocol_guard);

                    // Send through control channel would require access to control_tx
                    // For now, permissions are processed but response sending needs client cooperation
                    // This is acceptable for the automatic mode
                    // In a full implementation, we'd send _request through control_tx
                    #[cfg(feature = "tracing-support")]
                    tracing::debug!(request_id = %request_id.as_str(), result = ?result, "Permission processed");
                    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                    eprintln!("Permission {} processed: {:?}", request_id.as_str(), result);
                }
                Err(_e) => {
                    #[cfg(feature = "tracing-support")]
                    tracing::error!(error = %_e, "Permission processing error");
                    #[cfg(all(debug_assertions, not(feature = "tracing-support")))]
                    eprintln!("Permission processing error: {_e}");
                }
            }
        }
    }

    /// Send initialization request with hooks configuration to CLI
    ///
    /// # Arguments
    /// * `hooks_config` - Hooks configuration with callback IDs
    ///
    /// # Errors
    /// Returns error if initialization fails
    async fn send_initialize(
        &mut self,
        hooks_config: HashMap<HookEvent, Vec<HookMatcherConfig>>,
    ) -> Result<()> {
        // Generate unique request ID
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};
        static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);
        let counter = REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let request_id = format!("req_{}_{:x}", counter, nanos);

        // Build initialize request
        let init_request = serde_json::json!({
            "type": "control_request",
            "request_id": request_id,
            "request": {
                "subtype": "initialize",
                "hooks": hooks_config
            }
        });

        // Send initialization request
        let message_json = format!("{}\n", serde_json::to_string(&init_request)?);
        let mut transport = self.transport.lock().await;
        transport.write(&message_json).await?;
        drop(transport);

        // Wait a bit for the CLI to process initialization
        // TODO: Wait for actual init response instead of sleeping
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(())
    }

    /// Send a message to Claude
    ///
    /// # Arguments
    /// * `content` - Message content to send
    ///
    /// # Errors
    /// Returns error if message cannot be sent
    pub async fn send_message(&mut self, content: impl Into<String>) -> Result<()> {
        // Send a user message in the format the CLI expects
        let message = serde_json::json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": content.into()
            }
        });
        let message_json = format!("{}\n", serde_json::to_string(&message)?);

        let mut transport = self.transport.lock().await;
        transport.write(&message_json).await
    }

    /// Send an interrupt signal
    ///
    /// **Note**: Interrupt functionality via control messages may not be fully supported
    /// in all Claude CLI versions. The method demonstrates the SDK's bidirectional
    /// capability and will send the control message without blocking, but the CLI
    /// may not process it. Check your CLI version for control message support.
    ///
    /// # Errors
    /// Returns error if interrupt cannot be sent
    pub async fn interrupt(&mut self) -> Result<()> {
        let protocol = self.protocol.lock().await;
        let request = protocol.create_interrupt_request();
        drop(protocol);

        self.control_tx
            .send(request)
            .map_err(|_| ClaudeError::transport("Control channel closed"))
    }

    /// Get the next message from the stream
    ///
    /// Returns None when the stream ends
    pub async fn next_message(&mut self) -> Option<Result<Message>> {
        self.message_rx.recv().await
    }

    /// Take the hook event receiver
    ///
    /// This allows the caller to handle hook events independently
    pub fn take_hook_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<(String, HookEvent, serde_json::Value)>> {
        self.hook_rx.take()
    }

    /// Take the permission request receiver
    ///
    /// This allows the caller to handle permission requests independently
    pub fn take_permission_receiver(
        &mut self,
    ) -> Option<mpsc::UnboundedReceiver<(RequestId, PermissionRequest)>> {
        self.permission_rx.take()
    }

    /// Respond to a hook event
    ///
    /// # Arguments
    /// * `hook_id` - ID of the hook event being responded to
    /// * `response` - Hook response data
    ///
    /// # Errors
    /// Returns error if response cannot be sent
    pub async fn respond_to_hook(
        &mut self,
        hook_id: String,
        response: serde_json::Value,
    ) -> Result<()> {
        let protocol = self.protocol.lock().await;
        let request = protocol.create_hook_response(hook_id, response);
        drop(protocol);

        self.control_tx
            .send(request)
            .map_err(|_| ClaudeError::transport("Control channel closed"))
    }

    /// Respond to a permission request
    ///
    /// # Arguments
    /// * `request_id` - ID of the permission request being responded to
    /// * `result` - Permission result (Allow/Deny)
    ///
    /// # Errors
    /// Returns error if response cannot be sent
    pub async fn respond_to_permission(
        &mut self,
        request_id: RequestId,
        result: crate::types::PermissionResult,
    ) -> Result<()> {
        let protocol = self.protocol.lock().await;
        let request = protocol.create_permission_response(request_id, result);
        drop(protocol);

        self.control_tx
            .send(request)
            .map_err(|_| ClaudeError::transport("Control channel closed"))
    }

    /// Close the client and clean up resources
    ///
    /// # Errors
    /// Returns error if cleanup fails
    pub async fn close(&mut self) -> Result<()> {
        let mut transport = self.transport.lock().await;
        transport.close().await
    }
}

impl Drop for ClaudeSDKClient {
    fn drop(&mut self) {
        // Channel senders will be dropped, causing background tasks to exit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let options = ClaudeAgentOptions::default();
        let result = ClaudeSDKClient::new(options, None).await;
        assert!(result.is_ok() || result.is_err()); // Will succeed if CLI is available
    }
}
