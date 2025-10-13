# SDK MCP Server Integration Implementation Plan

## Goal

Integrate SDK MCP servers with Claude CLI conversations, enabling in-process tool execution without subprocess overhead. This matches the Python SDK's implementation where SDK MCP servers are:
- Registered via `ClaudeAgentOptions`
- Stored as instances in client state
- Called via control protocol `mcp_message` requests
- Invoked through JSONRPC routing to server handlers

**Current State**: SDK MCP servers exist but only work standalone via `handle_request()`.
**Target State**: SDK MCP servers integrated with Claude CLI like hooks are.

## Files to Create/Modify

### Files to Modify
1. `src/types.rs` - Add SDK server storage to `McpServerConfig`
2. `src/client/mod.rs` - Extract, store, and route SDK MCP servers
3. `src/transport/subprocess.rs` - Strip server instances before CLI config
4. `src/control/protocol.rs` - Add `mcp_message` control protocol types
5. `src/mcp/server.rs` - Add JSON schema validation
6. `src/mcp/tool.rs` - Add pre-compiled schema support
7. `src/error.rs` - Add validation error variant
8. `Cargo.toml` - Add `jsonschema` dependency

### Files to Create
- `examples/mcp_integration_demo.rs` - Full SDK MCP + Claude conversation demo

## Types, Functions, and Structs to Create/Modify

### src/types.rs
**Modify:**
- `McpServerConfig::Sdk` - Add `Arc<SdkMcpServer>` field

### src/client/mod.rs
**Add Fields:**
- `ClaudeSDKClient::sdk_mcp_servers` - `HashMap<String, Arc<SdkMcpServer>>`
- `ClaudeSDKClient::mcp_callback_tx` - Channel sender for MCP requests

**Add Functions:**
- `extract_sdk_mcp_servers()` - Extract SDK servers from options
- `mcp_message_handler_task()` - Background task for MCP message routing
- `handle_mcp_message()` - Route JSONRPC to server instance

**Modify Functions:**
- `new()` - Extract and store SDK servers, spawn MCP handler task
- `message_reader_task()` - Add `mcp_message` routing

### src/transport/subprocess.rs
**Modify Functions:**
- `spawn()` - Strip `instance` field from SDK server configs

### src/control/protocol.rs
**Add Types:**
- `ControlRequest::McpMessage` - MCP request variant
- `ControlResponse::McpMessage` - MCP response variant
- `McpMessageRequest` - Server name + JSONRPC message
- `McpMessageResponse` - JSONRPC response wrapper

**Add Functions:**
- `ProtocolHandler::create_mcp_message_response()` - Format MCP responses
- `ProtocolHandler::get_mcp_callback_channel()` - Getter for MCP channel

**Modify:**
- `ProtocolHandler` - Add `mcp_callback_tx` field

### src/mcp/tool.rs
**Add Fields:**
- `SdkMcpTool::compiled_schema` - `Option<JSONSchema>`

**Add Functions:**
- `SdkMcpTool::validate_input()` - Validate arguments against schema

**Modify Functions:**
- `SdkMcpTool::new()` - Pre-compile JSON schema

### src/mcp/server.rs
**Modify Functions:**
- `handle_request()` - Add schema validation before tool invocation

### src/error.rs
**Add Variants:**
- `ClaudeError::ValidationError(String)` - Schema validation errors

## Sequential Task List

1. **Add Dependencies** - Add `jsonschema` crate to `Cargo.toml`
2. **Schema Validation** - Implement JSON schema validation in MCP tools
3. **Type Updates** - Modify `McpServerConfig::Sdk` to store server instances
4. **Extraction** - Extract SDK servers during client initialization
5. **CLI Config Stripping** - Remove server instances before passing to CLI
6. **Control Protocol Types** - Add `mcp_message` request/response types
7. **Message Routing** - Route `mcp_message` requests in message reader
8. **Handler Task** - Implement `mcp_message_handler_task()` for tool execution
9. **Response Formatting** - Wrap JSONRPC responses in control protocol format
10. **Testing** - Create integration demo and verify functionality

---

## Detailed Phase Specifications

### Phase 1: Add Dependencies

**Goal**: Add JSON schema validation library.

**Files**: `Cargo.toml`

**Changes**:
```toml
[dependencies]
jsonschema = "0.18"
```

**Tests**: Run `cargo check` to verify dependency resolution.

---

### Phase 2: Schema Validation - Tool Updates

**Goal**: Add pre-compiled JSON schema validation to tools.

**Files**:
- `src/mcp/tool.rs`
- `src/error.rs`

**Changes**:

`src/error.rs`:
```rust
#[derive(Debug, thiserror::Error)]
pub enum ClaudeError {
    // ... existing variants

    #[error("Schema validation error: {0}")]
    ValidationError(String),
}
```

`src/mcp/tool.rs`:
- Add import: `use jsonschema::JSONSchema;`
- Add field to `SdkMcpTool`:
  ```rust
  pub(crate) compiled_schema: Option<JSONSchema>
  ```
- Modify `SdkMcpTool::new()`:
  - Compile schema: `JSONSchema::compile(&input_schema).ok()`
  - Store in `compiled_schema` field
- Add method `validate_input()`:
  ```rust
  pub fn validate_input(&self, input: &serde_json::Value) -> Result<()>
  ```
  - If schema exists, validate input
  - Collect errors and return `ClaudeError::ValidationError` if invalid

**Tests**:
```rust
#[test]
fn test_schema_validation_success() {
    let tool = SdkMcpTool::new(
        "add",
        "Add numbers",
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"}
            },
            "required": ["a", "b"]
        }),
        |_| async { Ok(ToolResult::text("ok")) }
    );

    // Valid input
    assert!(tool.validate_input(&json!({"a": 5, "b": 3})).is_ok());
}

#[test]
fn test_schema_validation_failure() {
    let tool = SdkMcpTool::new(
        "add",
        "Add numbers",
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number"}
            },
            "required": ["a"]
        }),
        |_| async { Ok(ToolResult::text("ok")) }
    );

    // Missing required field
    assert!(tool.validate_input(&json!({})).is_err());

    // Wrong type
    assert!(tool.validate_input(&json!({"a": "text"})).is_err());
}
```

---

### Phase 3: Schema Validation - Server Integration

**Goal**: Use validation in server's `handle_request()`.

**Files**: `src/mcp/server.rs`

**Changes**:

In `handle_tools_call()` method, after extracting arguments (line 231), before invoking tool (line 244):

```rust
let arguments = params["arguments"].clone();

// Validate using pre-compiled schema
if let Err(e) = tool.validate_input(&arguments) {
    return Ok(JsonRpcResponse::error(
        request_id,
        McpError::invalid_params(format!("Validation failed: {e}")),
    ));
}

// Invoke the tool (validation passed)
match tool.invoke(arguments).await {
    // ... existing code
```

**Tests**:
Add to existing tests in `src/mcp/server.rs`:
```rust
#[tokio::test]
async fn test_validation_error() {
    let server = create_test_server();

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "test_tool",
            "arguments": {}  // Missing required fields
        })),
    };

    let response = server.handle_request(request).await.unwrap();
    assert!(response.error.is_some());
    assert!(response.error.unwrap().message.contains("Validation failed"));
}
```

---

### Phase 4: Type Updates - McpServerConfig

**Goal**: Store SDK server instances in `McpServerConfig`.

**Files**: `src/types.rs`

**Changes**:

Modify `McpServerConfig` enum:
```rust
/// SDK-based in-process MCP server
Sdk(SdkMcpServerMarker, Arc<crate::mcp::SdkMcpServer>),
```

Where `SdkMcpServerMarker` keeps metadata and `Arc<SdkMcpServer>` is the actual server instance.

Or alternatively, modify `SdkMcpServerMarker`:
```rust
#[derive(Clone)]
pub struct SdkMcpServerMarker {
    pub name: String,
    pub instance: Arc<crate::mcp::SdkMcpServer>,
}
```

**Tests**: Ensure `ClaudeAgentOptions` can be created with SDK servers:
```rust
#[test]
fn test_sdk_server_in_options() {
    let server = SdkMcpServer::new("test").version("1.0.0");
    let server_arc = Arc::new(server);

    let mut mcp_servers = HashMap::new();
    mcp_servers.insert(
        "test".to_string(),
        McpServerConfig::Sdk(
            SdkMcpServerMarker { name: "test".to_string() },
            server_arc.clone()
        )
    );

    let options = ClaudeAgentOptions {
        mcp_servers: McpServers::Dict(mcp_servers),
        ..Default::default()
    };

    // Verify instance is stored
    assert!(matches!(
        options.mcp_servers,
        McpServers::Dict(_)
    ));
}
```

---

### Phase 5: Extraction - Client Initialization

**Goal**: Extract SDK MCP servers from options during client creation.

**Files**: `src/client/mod.rs`

**Changes**:

Add field to `ClaudeSDKClient`:
```rust
sdk_mcp_servers: HashMap<String, Arc<SdkMcpServer>>,
mcp_callback_tx: Option<mpsc::UnboundedSender<(RequestId, String, serde_json::Value)>>,
```

Add extraction logic in `ClaudeSDKClient::new()` (similar to hooks extraction around line 219):
```rust
// Extract SDK MCP servers
let sdk_mcp_servers = if let McpServers::Dict(ref servers) = options.mcp_servers {
    servers
        .iter()
        .filter_map(|(name, config)| {
            if let McpServerConfig::Sdk(_, instance) = config {
                Some((name.clone(), instance.clone()))
            } else {
                None
            }
        })
        .collect::<HashMap<_, _>>()
} else {
    HashMap::new()
};
```

Store in client:
```rust
let mut client = Self {
    // ... existing fields
    sdk_mcp_servers,
    mcp_callback_tx: None,  // Set later after spawning tasks
};
```

**Tests**:
```rust
#[tokio::test]
async fn test_extract_sdk_servers() {
    let server = Arc::new(SdkMcpServer::new("test").version("1.0.0"));
    let mut mcp_servers = HashMap::new();
    mcp_servers.insert(
        "test".to_string(),
        McpServerConfig::Sdk(
            SdkMcpServerMarker { name: "test".to_string() },
            server.clone()
        )
    );

    let options = ClaudeAgentOptions {
        mcp_servers: McpServers::Dict(mcp_servers),
        ..Default::default()
    };

    let client = ClaudeSDKClient::new("test prompt".to_string(), Some(options))
        .await
        .unwrap();

    assert_eq!(client.sdk_mcp_servers.len(), 1);
    assert!(client.sdk_mcp_servers.contains_key("test"));
}
```

---

### Phase 6: CLI Config Stripping

**Goal**: Remove server instances before passing MCP config to CLI.

**Files**: `src/transport/subprocess.rs`

**Changes**:

In `SubprocessTransport::spawn()`, when building MCP config (around line 200), strip SDK instances:

```rust
if let McpServers::Dict(ref servers) = self.options.mcp_servers {
    let mut servers_for_cli = serde_json::Map::new();

    for (name, config) in servers {
        match config {
            McpServerConfig::Sdk(marker, _instance) => {
                // Strip instance field - CLI only needs metadata
                servers_for_cli.insert(
                    name.clone(),
                    json!({
                        "type": "sdk",
                        "name": marker.name
                    })
                );
            }
            McpServerConfig::Stdio(cfg) => {
                servers_for_cli.insert(
                    name.clone(),
                    serde_json::to_value(cfg).unwrap_or_default()
                );
            }
            // ... other variants
        }
    }

    cmd.arg("--mcp-config");
    cmd.arg(serde_json::to_string(&json!({
        "mcpServers": servers_for_cli
    }))?);
}
```

**Tests**:
```rust
#[test]
fn test_sdk_server_config_stripped() {
    let server = Arc::new(SdkMcpServer::new("calc").version("1.0.0"));
    let mut mcp_servers = HashMap::new();
    mcp_servers.insert(
        "calc".to_string(),
        McpServerConfig::Sdk(
            SdkMcpServerMarker { name: "calc".to_string() },
            server
        )
    );

    let options = ClaudeAgentOptions {
        mcp_servers: McpServers::Dict(mcp_servers),
        ..Default::default()
    };

    let transport = SubprocessTransport::new(options);
    let cmd_args = transport.build_command_args();

    // Find --mcp-config argument
    let config_idx = cmd_args.iter().position(|s| s == "--mcp-config").unwrap();
    let config_json: serde_json::Value =
        serde_json::from_str(&cmd_args[config_idx + 1]).unwrap();

    // Verify instance field is not present
    let calc_config = &config_json["mcpServers"]["calc"];
    assert_eq!(calc_config["type"], "sdk");
    assert_eq!(calc_config["name"], "calc");
    assert!(calc_config.get("instance").is_none());
}
```

---

### Phase 7: Control Protocol Types

**Goal**: Add `mcp_message` request/response types to control protocol.

**Files**: `src/control/protocol.rs`

**Changes**:

Add request/response structs:
```rust
/// MCP message request from CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMessageRequest {
    pub server_name: String,
    pub message: serde_json::Value,  // JSONRPC message
}

/// MCP message response to CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMessageResponse {
    pub mcp_response: serde_json::Value,  // JSONRPC response
}
```

Add variants to `ControlRequest` and `ControlResponse`:
```rust
pub enum ControlRequest {
    // ... existing variants
    McpMessageResponse {
        id: RequestId,
        response: McpMessageResponse,
    },
}

pub enum ControlResponse {
    // ... existing variants
    McpMessage {
        id: RequestId,
        server_name: String,
        message: serde_json::Value,
    },
}
```

Add field to `ProtocolHandler`:
```rust
mcp_callback_tx: Option<mpsc::UnboundedSender<(RequestId, String, serde_json::Value)>>,
```

Add methods:
```rust
impl ProtocolHandler {
    pub fn set_mcp_callback_channel(
        &mut self,
        tx: mpsc::UnboundedSender<(RequestId, String, serde_json::Value)>
    ) {
        self.mcp_callback_tx = Some(tx);
    }

    pub fn get_mcp_callback_channel(
        &self
    ) -> Option<&mpsc::UnboundedSender<(RequestId, String, serde_json::Value)>> {
        self.mcp_callback_tx.as_ref()
    }

    pub fn create_mcp_message_response(
        &self,
        request_id: RequestId,
        response: serde_json::Value
    ) -> ControlRequest {
        ControlRequest::McpMessageResponse {
            id: request_id,
            response: McpMessageResponse {
                mcp_response: response,
            },
        }
    }
}
```

**Tests**:
```rust
#[test]
fn test_mcp_message_serialization() {
    let request = McpMessageRequest {
        server_name: "calc".to_string(),
        message: json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": "add", "arguments": {"a": 5, "b": 3}}
        }),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["server_name"], "calc");
    assert_eq!(json["message"]["method"], "tools/call");
}
```

---

### Phase 8: Message Routing - Reader Task

**Goal**: Route `mcp_message` control requests to MCP handler.

**Files**: `src/client/mod.rs`

**Changes**:

In `message_reader_task()`, add handling for `mcp_message` subtype (similar to `hook_callback` handling around line 392):

```rust
// Handle control_request (MCP messages from CLI)
if msg_type == Some("control_request") {
    let protocol_guard = protocol.lock().await;

    if let Some(request_obj) = value.get("request").and_then(|v| v.as_object()) {
        let subtype = request_obj.get("subtype").and_then(|v| v.as_str());

        if subtype == Some("mcp_message") {
            let request_id = value.get("request_id")
                .and_then(|v| v.as_str())
                .map(|s| RequestId::new(s))
                .unwrap_or_else(|| RequestId::new("unknown"));

            let server_name = request_obj.get("server_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let message = request_obj.get("message")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));

            if let Some(tx) = protocol_guard.get_mcp_callback_channel() {
                let _ = tx.send((request_id, server_name, message));
            }
        }
    }

    drop(protocol_guard);
    continue;
}
```

**Tests**: Integration test (will be in Phase 10)

---

### Phase 9: Handler Task - MCP Message Processing

**Goal**: Implement background task to handle MCP JSONRPC routing.

**Files**: `src/client/mod.rs`

**Changes**:

Add function:
```rust
async fn mcp_message_handler_task(
    sdk_mcp_servers: HashMap<String, Arc<SdkMcpServer>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    control_tx: mpsc::UnboundedSender<ControlRequest>,
    mut mcp_callback_rx: mpsc::UnboundedReceiver<(RequestId, String, serde_json::Value)>,
) {
    while let Some((request_id, server_name, message)) = mcp_callback_rx.recv().await {
        // Get server instance
        let server = match sdk_mcp_servers.get(&server_name) {
            Some(s) => s,
            None => {
                eprintln!("SDK MCP server not found: {server_name}");

                // Send error response
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": message.get("id"),
                    "error": {
                        "code": -32601,
                        "message": format!("Server '{server_name}' not found")
                    }
                });

                let protocol_guard = protocol.lock().await;
                let response = protocol_guard.create_mcp_message_response(
                    request_id,
                    error_response
                );
                drop(protocol_guard);

                let _ = control_tx.send(response);
                continue;
            }
        };

        // Parse JSONRPC request
        let jsonrpc_request: crate::mcp::protocol::JsonRpcRequest =
            match serde_json::from_value(message) {
                Ok(req) => req,
                Err(e) => {
                    eprintln!("Failed to parse JSONRPC request: {e}");
                    continue;
                }
            };

        // Invoke server handler
        match server.handle_request(jsonrpc_request).await {
            Ok(jsonrpc_response) => {
                // Convert to JSON and send response
                let response_json = serde_json::to_value(&jsonrpc_response)
                    .unwrap_or_else(|_| json!({"error": "Serialization failed"}));

                let protocol_guard = protocol.lock().await;
                let response = protocol_guard.create_mcp_message_response(
                    request_id,
                    response_json
                );
                drop(protocol_guard);

                if let Err(e) = control_tx.send(response) {
                    eprintln!("Failed to send MCP response: {e:?}");
                }
            }
            Err(e) => {
                eprintln!("MCP server error: {e}");

                // Send error response
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": format!("Server error: {e}")
                    }
                });

                let protocol_guard = protocol.lock().await;
                let response = protocol_guard.create_mcp_message_response(
                    request_id,
                    error_response
                );
                drop(protocol_guard);

                let _ = control_tx.send(response);
            }
        }
    }
}
```

Spawn task in `ClaudeSDKClient::new()` (around line 300, after spawning other tasks):
```rust
// Spawn MCP message handler task if SDK servers are present
let mcp_rx = if !sdk_mcp_servers.is_empty() {
    let (mcp_callback_tx, mcp_callback_rx) = mpsc::unbounded_channel();

    // Set channel in protocol handler
    {
        let mut protocol_guard = protocol.lock().await;
        protocol_guard.set_mcp_callback_channel(mcp_callback_tx.clone());
    }

    // Spawn handler task
    tokio::spawn(Self::mcp_message_handler_task(
        sdk_mcp_servers.clone(),
        protocol.clone(),
        control_tx.clone(),
        mcp_callback_rx,
    ));

    Some(mcp_callback_tx)
} else {
    None
};

// Store in client
let mut client = Self {
    // ... existing fields
    sdk_mcp_servers,
    mcp_callback_tx: mcp_rx,
};
```

**Tests**: Integration test (will be in Phase 10)

---

### Phase 10: Response Formatting - Control Writer

**Goal**: Format MCP responses for CLI in control writer task.

**Files**: `src/client/mod.rs`

**Changes**:

In `control_writer_task()`, add handling for `McpMessageResponse` (around line 445):

```rust
ControlRequest::McpMessageResponse { id, response } => {
    // Send MCP response back to CLI
    serde_json::json!({
        "type": "control_response",
        "response": {
            "subtype": "success",
            "request_id": id.as_str(),
            "response": {
                "mcp_response": response.mcp_response
            }
        }
    })
}
```

**Tests**: Integration test (next phase)

---

### Phase 11: Integration Testing

**Goal**: Create comprehensive example demonstrating SDK MCP + Claude conversation.

**Files**: `examples/mcp_integration_demo.rs`

**Content**:
```rust
//! SDK MCP Server Integration Demo
//!
//! Demonstrates SDK MCP servers working in Claude conversations.
//! Tools are called by Claude and executed in-process.

use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolContent, ToolResult};
use claude_agent_sdk::{ClaudeAgentOptions, ClaudeSDKClient};
use serde_json::json;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SDK MCP Integration Demo ===\n");

    // Create calculator tools
    let add_tool = SdkMcpTool::new(
        "add",
        "Add two numbers together",
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number", "description": "First number"},
                "b": {"type": "number", "description": "Second number"}
            },
            "required": ["a", "b"]
        }),
        |input| {
            Box::pin(async move {
                let a = input["a"].as_f64().unwrap();
                let b = input["b"].as_f64().unwrap();
                let result = a + b;

                println!("  [SDK TOOL] add({a}, {b}) = {result}");

                Ok(ToolResult {
                    content: vec![ToolContent::Text {
                        text: format!("{a} + {b} = {result}"),
                    }],
                    is_error: None,
                })
            })
        },
    );

    let multiply_tool = SdkMcpTool::new(
        "multiply",
        "Multiply two numbers",
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"}
            },
            "required": ["a", "b"]
        }),
        |input| {
            Box::pin(async move {
                let a = input["a"].as_f64().unwrap();
                let b = input["b"].as_f64().unwrap();
                let result = a * b;

                println!("  [SDK TOOL] multiply({a}, {b}) = {result}");

                Ok(ToolResult {
                    content: vec![ToolContent::Text {
                        text: format!("{a} × {b} = {result}"),
                    }],
                    is_error: None,
                })
            })
        },
    );

    // Create SDK MCP server
    let calculator = SdkMcpServer::new("calculator")
        .version("1.0.0")
        .tool(add_tool)
        .tool(multiply_tool);

    let calculator_arc = Arc::new(calculator);

    // Configure options
    let mut mcp_servers = std::collections::HashMap::new();
    mcp_servers.insert(
        "calc".to_string(),
        claude_agent_sdk::types::McpServerConfig::Sdk(
            claude_agent_sdk::types::SdkMcpServerMarker {
                name: "calc".to_string(),
            },
            calculator_arc,
        ),
    );

    let options = ClaudeAgentOptions {
        mcp_servers: claude_agent_sdk::types::McpServers::Dict(mcp_servers),
        allowed_tools: vec![
            "mcp__calc__add".into(),
            "mcp__calc__multiply".into(),
        ],
        max_turns: Some(5),
        ..Default::default()
    };

    // Test with Claude
    println!("Testing: Calculate 15 + 27\n");
    let mut client = ClaudeSDKClient::new(
        "Calculate 15 + 27 using the add tool".to_string(),
        Some(options.clone()),
    )
    .await?;

    while let Some(message) = client.next_message().await {
        match message {
            claude_agent_sdk::Message::Text(text) => {
                println!("Claude: {text}");
            }
            claude_agent_sdk::Message::ToolUse(tool_use) => {
                println!("Claude calling tool: {}", tool_use.name);
            }
            claude_agent_sdk::Message::ToolResult(result) => {
                println!("Tool result: {:?}", result);
            }
            _ => {}
        }
    }

    println!("\n✓ Integration test complete!");
    println!("SDK MCP servers work with Claude CLI!");

    Ok(())
}
```

**Manual Test Steps**:
1. Run `cargo run --example mcp_integration_demo`
2. Verify output shows:
   - `[SDK TOOL] add(15, 27) = 42`
   - Claude receives and displays the result
   - No errors in control protocol
3. Test with invalid input to verify validation:
   ```rust
   "Calculate the square root of -1 using the add tool"
   ```
   Should show validation error for wrong tool usage

**Automated Tests**:

Add to `tests/client_tests.rs`:
```rust
#[tokio::test]
async fn test_sdk_mcp_integration() {
    let add_tool = SdkMcpTool::new(
        "add",
        "Add numbers",
        json!({"type": "object", "properties": {"a": {"type": "number"}, "b": {"type": "number"}}, "required": ["a", "b"]}),
        |input| Box::pin(async move {
            let a = input["a"].as_f64().unwrap();
            let b = input["b"].as_f64().unwrap();
            Ok(ToolResult::text(format!("{}", a + b)))
        }),
    );

    let server = Arc::new(
        SdkMcpServer::new("calc")
            .version("1.0.0")
            .tool(add_tool)
    );

    let mut mcp_servers = HashMap::new();
    mcp_servers.insert(
        "calc".to_string(),
        McpServerConfig::Sdk(
            SdkMcpServerMarker { name: "calc".to_string() },
            server,
        ),
    );

    let options = ClaudeAgentOptions {
        mcp_servers: McpServers::Dict(mcp_servers),
        allowed_tools: vec!["mcp__calc__add".into()],
        max_turns: Some(2),
        ..Default::default()
    };

    let mut client = ClaudeSDKClient::new(
        "Use the add tool to calculate 5 + 3".to_string(),
        Some(options),
    )
    .await
    .expect("Failed to create client");

    let mut tool_called = false;
    while let Some(message) = client.next_message().await {
        if let Message::ToolUse(tool_use) = message {
            if tool_use.name == "mcp__calc__add" {
                tool_called = true;
            }
        }
    }

    assert!(tool_called, "SDK MCP tool should have been called");
}

#[tokio::test]
async fn test_sdk_mcp_validation_error() {
    // Create tool with strict schema
    let tool = SdkMcpTool::new(
        "strict",
        "Strict tool",
        json!({"type": "object", "properties": {"x": {"type": "number"}}, "required": ["x"]}),
        |_| Box::pin(async move { Ok(ToolResult::text("ok")) }),
    );

    let server = Arc::new(SdkMcpServer::new("test").tool(tool));

    // Test direct JSONRPC call with invalid input
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "strict",
            "arguments": {}  // Missing required "x"
        })),
    };

    let response = server.handle_request(request).await.unwrap();

    // Should get validation error
    assert!(response.error.is_some());
    let error = response.error.unwrap();
    assert!(error.message.contains("Validation failed"));
}
```

---

## Success Criteria

- ✅ SDK MCP servers can be registered via `ClaudeAgentOptions`
- ✅ Tools are invoked during Claude conversations
- ✅ Tool results are returned to Claude
- ✅ JSON schema validation prevents invalid inputs
- ✅ Control protocol routes `mcp_message` requests correctly
- ✅ CLI receives SDK server metadata without instance objects
- ✅ Integration demo runs without errors
- ✅ All tests pass

## Testing Strategy

1. **Unit Tests**: Each phase includes tests for new functions/types
2. **Integration Tests**: Full end-to-end test in Phase 11
3. **Manual Testing**: Run example and verify output
4. **Validation Tests**: Ensure schema validation catches errors

## Notes

- Similar complexity to hooks implementation (~7 phases vs 11 for hooks)
- Reuses control protocol infrastructure from hooks
- Pre-compiled schemas improve performance
- Tool naming matches Python SDK: `mcp__<server>__<tool>`
