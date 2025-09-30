# Claude Agent SDK for Rust

Rust SDK for Claude Agent. A Rust implementation mirroring the [Python Claude Agent SDK](https://github.com/anthropics/claude-agent-sdk-python) with idiomatic Rust patterns and best practices.

## Status

✅ **Feature Parity** - v0.1.0 - Full feature parity with Python SDK

### Completed Features
- ✅ Core error types with `thiserror`
- ✅ Comprehensive type system with newtypes for type safety
- ✅ Transport layer with subprocess CLI integration (lock-free architecture)
- ✅ Basic `query()` function for simple queries
- ✅ Message parsing with full message type support
- ✅ Builder pattern for options
- ✅ Control protocol handler (16 integration tests)
- ✅ ClaudeSDKClient for bidirectional communication (10 tests, working demo)
- ✅ **Hook system** - Fully integrated with automatic handling
- ✅ **Permission callbacks** - Fully integrated with automatic handling
- ✅ **SDK MCP server** - In-process custom tools with JSONRPC protocol
- ✅ Comprehensive test suite (70 tests: 42 unit + 28 doc, all passing)
- ✅ Full documentation with examples and API docs (6,200+ LOC)

### Examples
- `simple_query.rs` - Basic query usage
- `interactive_client.rs` - Interactive conversation
- `bidirectional_demo.rs` - Concurrent operations demonstration
- `hooks_demo.rs` - Hook system with 3 examples
- `permissions_demo.rs` - Permission system with 3 examples
- `mcp_demo.rs` - Custom tools with SDK MCP server

## Installation

**Prerequisites:**
- Rust 1.75.0 or later
- Node.js
- Claude Code: `npm install -g @anthropic-ai/claude-code`

Add to your `Cargo.toml`:

```toml
[dependencies]
claude-agent-sdk = "0.1.0"
tokio = { version = "1", features = ["full"] }
futures = "0.3"
```

## Quick Start

```rust
use claude_agent_sdk::query;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stream = query("What is 2 + 2?", None).await?;
    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        println!("{:?}", message?);
    }
    Ok(())
}
```

## Basic Usage: query()

`query()` is an async function for querying Claude Code. It returns a `Stream` of response messages.

```rust
use claude_agent_sdk::{query, ClaudeAgentOptions, Message, ContentBlock};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple query
    let stream = query("Hello Claude", None).await?;
    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        println!("Claude: {}", text);
                    }
                }
            }
            Message::Result { total_cost_usd, .. } => {
                if let Some(cost) = total_cost_usd {
                    println!("Cost: ${:.4}", cost);
                }
            }
            _ => {}
        }
    }

    Ok(())
}
```

### With Options

```rust
use claude_agent_sdk::{query, ClaudeAgentOptions, PermissionMode};

let options = ClaudeAgentOptions::builder()
    .system_prompt("You are a helpful coding assistant")
    .max_turns(5)
    .permission_mode(PermissionMode::AcceptEdits)
    .add_allowed_tool("Read")
    .add_allowed_tool("Write")
    .build();

let stream = query("Create a hello.py file", Some(options)).await?;
```

## Architecture

The SDK follows Rust best practices and idiomatic patterns:

### Type Safety
- **Newtypes** for IDs (`SessionId`, `ToolName`, `RequestId`)
- **Strong typing** prevents mixing incompatible values
- **Builder pattern** for complex configuration

### Error Handling
- Uses `thiserror` for ergonomic error types
- All errors are `Result<T, ClaudeError>`
- Rich error context with exit codes and stderr

### Async/Await
- Built on `tokio` runtime
- `async-trait` for trait methods
- Streams for message handling

### Modularity
```
src/
├── error.rs         # Error types
├── types.rs         # Type definitions
├── transport/       # Communication layer
│   ├── mod.rs       # Transport trait
│   └── subprocess.rs # CLI subprocess
├── message/         # Message parsing
├── query.rs         # Simple query function
└── lib.rs          # Public API
```

## Features Comparison with Python SDK

| Feature | Rust | Python |
|---------|------|--------|
| `query()` function | ✅ | ✅ |
| `ClaudeSDKClient` | ✅ | ✅ |
| Custom Tools (SDK MCP) | ✅ | ✅ |
| Hooks | ✅ | ✅ |
| Permission callbacks | ✅ | ✅ |
| Type safety | ✅✅ | ✅ |
| Error handling | ✅✅ | ✅ |
| Documentation | ✅✅ | ✅ |
| Performance | ✅✅ | ✅ |

**Legend:** ✅ = Supported, ✅✅ = Enhanced implementation

## Examples

Run examples with:

```bash
cargo run --example simple_query
```

See `examples/` directory for more:
- `simple_query.rs` - Basic usage with options
- `interactive_client.rs` - Interactive conversation
- `bidirectional_demo.rs` - Concurrent operations demonstration
- `hooks_demo.rs` - Hook system with 3 examples
- `permissions_demo.rs` - Permission system with 3 examples
- `mcp_demo.rs` - Custom tools with SDK MCP server

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Linting

```bash
cargo clippy
cargo fmt
```

### Documentation

```bash
cargo doc --open
```

## Design Principles

This SDK follows principles from:
- [Rust Security Handbook](https://github.com/yevh/rust-security-handbook)
- [Rust Error Handling Guide](https://www.howtocodeit.com/articles/the-definitive-guide-to-rust-error-handling)
- [Rust Ownership Best Practices](https://www.howtocodeit.com/articles/rust-ownership-explained-merging-linked-lists)
- [Rust Newtype Pattern](https://www.howtocodeit.com/articles/ultimate-guide-rust-newtypes)
- [Hexagonal Architecture in Rust](https://www.howtocodeit.com/articles/master-hexagonal-architecture-rust)

Key principles applied:
- **Type safety** with newtypes
- **Memory safety** without unsafe code
- **Clear error propagation** with Result types
- **Ownership patterns** for resource management
- **Builder pattern** for complex types
- **Trait abstraction** for extensibility

## Security

This SDK has undergone security hardening with multiple layers of protection:

- **Environment variable filtering** - Blocks dangerous variables like `LD_PRELOAD`, `PATH`, `NODE_OPTIONS`
- **Argument validation** - Allowlist for CLI flags prevents injection attacks
- **Timeout protection** - All I/O operations have configurable timeouts
- **Buffer limits** - Prevents memory exhaustion from unbounded growth
- **Secure defaults** - Conservative defaults for all security-sensitive options
- **No unsafe code** - 100% safe Rust with compiler guarantees

For details on security improvements, see:
- `fixes.md` - Complete security audit report
- `SECURITY_FIXES_APPLIED.md` - Implementation details

## Contributing

This is a reference implementation demonstrating Rust SDK development best practices. See `PLAN.md` for the complete implementation roadmap.

## License

MIT

## Related Projects

- [Python Claude Agent SDK](https://github.com/anthropics/claude-agent-sdk-python)
- [Claude Code](https://github.com/anthropics/claude-code)
- [MCP (Model Context Protocol)](https://github.com/anthropics/mcp)
