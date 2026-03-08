# Examples

## Working Examples (No CLI Required)

### test_data_flow ✅ Recommended
Tests that hooks receive data without needing Claude CLI.

```bash
cargo run --example test_data_flow
```

**Output:**
```
📨 Protocol forwarded: ID: test-1, Event: PreToolUse
✅ Hook received:
   Tool: Some("Bash")
   Data: { "tool_name": "Bash", "tool_input": {...}, ... }
```

This **proves the fix works!**

---

## Examples That Need Claude CLI

### log_tool_use

Logs all tool usage when Claude runs commands.

**Requirements:**

1. **Claude CLI installed:**
   ```bash
   npm install -g @anthropic-ai/claude-code
   # or
   npm install -g @anthropic-ai/claude-cli
   ```

2. **Verify installation:**
   ```bash
   which claude
   claude --version
   ```

3. **Run the example:**
   ```bash
   cargo run --example log_tool_use
   ```

**Expected output:**
```
=== Tool Usage Logger ===

Connecting to Claude CLI...
Sending message...

🔧 Tool: Bash
   Input: {"command":"pwd"}

✅ Done!
```

**If it doesn't work:**
- Claude CLI might not be installed (install with npm above)
- Claude might not be using tools (normal for simple queries)
- API key might not be configured

**Tip:** Use `test_data_flow` to verify hooks work without CLI dependency.

---

## Other Examples

- `hooks_demo.rs` - Comprehensive hooks demonstration (needs CLI)
- `permissions_demo.rs` - Permission system demo (needs CLI)
- `interactive_client.rs` - Interactive chat (needs CLI)
- `simple_query.rs` - Basic query (needs CLI)
