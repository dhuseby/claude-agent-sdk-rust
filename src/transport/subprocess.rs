//! Subprocess transport implementation using Claude Code CLI

use async_trait::async_trait;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

use crate::error::{ClaudeError, Result};
use crate::types::{ClaudeAgentOptions, SystemPrompt};
use crate::{Transport, VERSION};

const DEFAULT_MAX_BUFFER_SIZE: usize = 1024 * 1024; // 1MB

// Dangerous environment variables that should not be passed to subprocess
const DANGEROUS_ENV_VARS: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "PATH",
    "NODE_OPTIONS",
    "PYTHONPATH",
    "PERL5LIB",
    "RUBYLIB",
];

// Allowed extra CLI flags (allowlist approach)
const ALLOWED_EXTRA_FLAGS: &[&str] = &[
    "timeout",
    "retries",
    "log-level",
    "cache-dir",
];

/// Prompt input type
#[derive(Debug)]
pub enum PromptInput {
    /// Single string prompt
    String(String),
    /// Stream of JSON messages
    Stream,
}

impl From<String> for PromptInput {
    fn from(s: String) -> Self {
        PromptInput::String(s)
    }
}

impl From<&str> for PromptInput {
    fn from(s: &str) -> Self {
        PromptInput::String(s.to_string())
    }
}

/// Subprocess transport for Claude Code CLI
pub struct SubprocessTransport {
    prompt: PromptInput,
    options: ClaudeAgentOptions,
    cli_path: PathBuf,
    cwd: Option<PathBuf>,
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    ready: Arc<AtomicBool>,
    max_buffer_size: usize,
    reader_task: Option<JoinHandle<()>>,
    stderr_task: Option<JoinHandle<()>>,
}

impl SubprocessTransport {
    /// Create a new subprocess transport
    ///
    /// # Arguments
    /// * `prompt` - The prompt input (string or stream)
    /// * `options` - Configuration options
    /// * `cli_path` - Optional path to Claude Code CLI (will search if None)
    ///
    /// # Errors
    /// Returns error if CLI cannot be found
    pub fn new(
        prompt: PromptInput,
        options: ClaudeAgentOptions,
        cli_path: Option<PathBuf>,
    ) -> Result<Self> {
        let cli_path = if let Some(path) = cli_path {
            path
        } else {
            Self::find_cli()?
        };

        let cwd = options.cwd.clone();
        let max_buffer_size = options.max_buffer_size.unwrap_or(DEFAULT_MAX_BUFFER_SIZE);

        Ok(Self {
            prompt,
            options,
            cli_path,
            cwd,
            process: None,
            stdin: None,
            stdout: None,
            ready: Arc::new(AtomicBool::new(false)),
            max_buffer_size,
            reader_task: None,
            stderr_task: None,
        })
    }

    /// Find Claude Code CLI binary
    fn find_cli() -> Result<PathBuf> {
        // Try using 'which' crate first
        if let Ok(path) = which::which("claude") {
            return Ok(path);
        }

        // Manual search in common locations
        let home = env::var("HOME").unwrap_or_else(|_| String::from("/root"));
        let locations = vec![
            PathBuf::from(home.clone()).join(".npm-global/bin/claude"),
            PathBuf::from("/usr/local/bin/claude"),
            PathBuf::from(home.clone()).join(".local/bin/claude"),
            PathBuf::from(home.clone()).join("node_modules/.bin/claude"),
            PathBuf::from(home).join(".yarn/bin/claude"),
        ];

        for path in locations {
            if path.exists() && path.is_file() {
                return Ok(path);
            }
        }

        Err(ClaudeError::cli_not_found())
    }

    /// Build CLI command with all arguments
    fn build_command(&self) -> Command {
        let mut cmd = Command::new(&self.cli_path);

        // Use streaming mode when:
        // 1. Prompt is Stream (interactive client using send_message)
        // 2. Control protocol features are enabled (hooks/permissions/SDK MCP)
        let has_sdk_mcp_servers = match &self.options.mcp_servers {
            crate::types::McpServers::Dict(servers) => {
                servers.values().any(|config| matches!(config, crate::types::McpServerConfig::Sdk(_)))
            }
            _ => false,
        };

        let needs_control_protocol = self.options.hooks.is_some()
            || self.options.can_use_tool.is_some()
            || has_sdk_mcp_servers;

        // Stream input ALWAYS needs streaming mode (used by ClaudeSDKClient.send_message)
        let needs_streaming_mode = matches!(self.prompt, PromptInput::Stream) || needs_control_protocol;

        if needs_streaming_mode {
            // Streaming mode: required for interactive input and bidirectional control protocol
            cmd.arg("--input-format").arg("stream-json");
            cmd.arg("--output-format").arg("stream-json");
        } else {
            // Non-interactive print mode: simpler for one-shot queries
            cmd.arg("--print");
            cmd.arg("--output-format").arg("stream-json");
        }

        cmd.arg("--verbose");

        // System prompt
        if let Some(ref system_prompt) = self.options.system_prompt {
            match system_prompt {
                SystemPrompt::String(s) => {
                    cmd.arg("--system-prompt").arg(s);
                }
                SystemPrompt::Preset(preset) => {
                    if let Some(ref append) = preset.append {
                        cmd.arg("--append-system-prompt").arg(append);
                    }
                }
            }
        }

        // Allowed tools
        if !self.options.allowed_tools.is_empty() {
            let tools: Vec<String> = self
                .options
                .allowed_tools
                .iter()
                .map(|t| t.as_str().to_string())
                .collect();
            cmd.arg("--allowedTools").arg(tools.join(","));
        }

        // Max turns
        if let Some(max_turns) = self.options.max_turns {
            cmd.arg("--max-turns").arg(max_turns.to_string());
        }

        // Disallowed tools
        if !self.options.disallowed_tools.is_empty() {
            let tools: Vec<String> = self
                .options
                .disallowed_tools
                .iter()
                .map(|t| t.as_str().to_string())
                .collect();
            cmd.arg("--disallowedTools").arg(tools.join(","));
        }

        // Model
        if let Some(ref model) = self.options.model {
            cmd.arg("--model").arg(model);
        }

        // Permission prompt tool
        if let Some(ref tool) = self.options.permission_prompt_tool_name {
            cmd.arg("--permission-prompt-tool").arg(tool);
        }

        // Permission mode
        if let Some(ref mode) = self.options.permission_mode {
            let mode_str = match mode {
                crate::types::PermissionMode::Default => "default",
                crate::types::PermissionMode::AcceptEdits => "acceptEdits",
                crate::types::PermissionMode::Plan => "plan",
                crate::types::PermissionMode::BypassPermissions => "bypassPermissions",
            };
            cmd.arg("--permission-mode").arg(mode_str);
        }

        // Continue conversation
        if self.options.continue_conversation {
            cmd.arg("--continue");
        }

        // Resume session
        if let Some(ref session_id) = self.options.resume {
            cmd.arg("--resume").arg(session_id.as_str());
        }

        // Settings file
        if let Some(ref settings) = self.options.settings {
            cmd.arg("--settings").arg(settings);
        }

        // Add directories
        for dir in &self.options.add_dirs {
            cmd.arg("--add-dir").arg(dir);
        }

        // MCP servers
        match &self.options.mcp_servers {
            crate::types::McpServers::Dict(servers) => {
                if !servers.is_empty() {
                    let mut config_map = HashMap::new();
                    for (name, config) in servers {
                        config_map.insert(name.clone(), Self::serialize_mcp_config(config));
                    }
                    let config_json = serde_json::json!({
                        "mcpServers": config_map
                    });
                    cmd.arg("--mcp-config").arg(config_json.to_string());
                }
            }
            crate::types::McpServers::Path(path) => {
                cmd.arg("--mcp-config").arg(path);
            }
            crate::types::McpServers::None => {}
        }

        // Include partial messages
        if self.options.include_partial_messages {
            cmd.arg("--include-partial-messages");
        }

        // Fork session
        if self.options.fork_session {
            cmd.arg("--fork-session");
        }

        // Agents
        if let Some(ref agents) = self.options.agents {
            let agents_json = serde_json::to_string(agents).unwrap_or_default();
            cmd.arg("--agents").arg(agents_json);
        }

        // Setting sources
        if let Some(ref sources) = self.options.setting_sources {
            let sources_str: Vec<&str> = sources
                .iter()
                .map(|s| match s {
                    crate::types::SettingSource::User => "user",
                    crate::types::SettingSource::Project => "project",
                    crate::types::SettingSource::Local => "local",
                })
                .collect();
            cmd.arg("--setting-sources").arg(sources_str.join(","));
        } else {
            cmd.arg("--setting-sources").arg("");
        }

        // Extra args - only allow safe flags from allowlist
        for (flag, value) in &self.options.extra_args {
            if ALLOWED_EXTRA_FLAGS.contains(&flag.as_str()) {
                if let Some(v) = value {
                    cmd.arg(format!("--{flag}")).arg(v);
                } else {
                    cmd.arg(format!("--{flag}"));
                }
            }
        }

        // Prompt handling based on mode
        match &self.prompt {
            PromptInput::Stream => {
                // Stream mode is handled by streaming mode flags above
                // No additional args needed - messages sent via stdin
            }
            PromptInput::String(s) => {
                if !needs_streaming_mode {
                    // Print mode: pass the prompt as an argument after --
                    cmd.arg("--").arg(s);
                }
                // In streaming mode, prompt will be sent via stdin
            }
        }

        cmd
    }

    /// Serialize MCP config for CLI
    fn serialize_mcp_config(config: &crate::types::McpServerConfig) -> serde_json::Value {
        match config {
            crate::types::McpServerConfig::Stdio(stdio) => {
                let mut obj = serde_json::json!({
                    "command": stdio.command,
                });
                if let Some(ref args) = stdio.args {
                    obj["args"] = serde_json::json!(args);
                }
                if let Some(ref env) = stdio.env {
                    obj["env"] = serde_json::json!(env);
                }
                if let Some(ref server_type) = stdio.server_type {
                    obj["type"] = serde_json::json!(server_type);
                }
                obj
            }
            crate::types::McpServerConfig::Sse(sse) => {
                serde_json::json!({
                    "type": sse.server_type,
                    "url": sse.url,
                    "headers": sse.headers,
                })
            }
            crate::types::McpServerConfig::Http(http) => {
                serde_json::json!({
                    "type": http.server_type,
                    "url": http.url,
                    "headers": http.headers,
                })
            }
            crate::types::McpServerConfig::Sdk(sdk) => {
                serde_json::json!({
                    "type": "sdk",
                    "name": sdk.name,
                })
            }
        }
    }
}

#[async_trait]
impl Transport for SubprocessTransport {
    async fn connect(&mut self) -> Result<()> {
        if self.process.is_some() {
            return Ok(());
        }

        let mut cmd = self.build_command();

        // Set up environment - filter dangerous variables
        let mut process_env = env::vars().collect::<HashMap<_, _>>();

        // Only add user-provided env vars that are not in the dangerous list
        for (key, value) in &self.options.env {
            if !DANGEROUS_ENV_VARS.contains(&key.as_str()) {
                process_env.insert(key.clone(), value.clone());
            }
        }

        process_env.insert("CLAUDE_CODE_ENTRYPOINT".to_string(), "sdk-rust".to_string());
        process_env.insert("CLAUDE_AGENT_SDK_VERSION".to_string(), VERSION.to_string());

        if let Some(ref cwd) = self.cwd {
            process_env.insert("PWD".to_string(), cwd.to_string_lossy().to_string());
            cmd.current_dir(cwd);
        }

        cmd.envs(process_env);

        // Set up stdio
        // IMPORTANT: We pipe stderr instead of inheriting to prevent the child process
        // from manipulating the parent terminal state. Inheriting stderr gives the child
        // access to the terminal, which can leave it in a corrupted state.
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()); // Pipe stderr to prevent terminal manipulation

        // Spawn process
        let mut child = cmd.spawn().map_err(|e| {
            if let Some(ref cwd) = self.cwd {
                if !cwd.exists() {
                    #[cfg(debug_assertions)]
                    return ClaudeError::connection(format!(
                        "Working directory does not exist: {}",
                        cwd.display()
                    ));
                    #[cfg(not(debug_assertions))]
                    return ClaudeError::connection("Working directory does not exist".to_string());
                }
            }
            ClaudeError::connection(format!("Failed to start Claude Code: {e}"))
        })?;

        // Get stdin, stdout, and stderr
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| ClaudeError::connection("Failed to get stdin handle"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ClaudeError::connection("Failed to get stdout handle"))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ClaudeError::connection("Failed to get stderr handle"))?;

        // Spawn task to consume stderr to prevent blocking
        // We forward it to parent stderr for visibility
        let stderr_task = tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut stderr = stderr;
            let mut buffer = vec![0u8; 4096];

            loop {
                match stderr.read(&mut buffer).await {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        // Forward stderr to parent's stderr
                        let _ = std::io::Write::write_all(&mut std::io::stderr(), &buffer[..n]);
                    }
                    Err(_) => break,
                }
            }
        });

        // Store handles
        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));
        self.process = Some(child);
        self.stderr_task = Some(stderr_task);
        self.ready.store(true, Ordering::SeqCst);

        // For string mode, close stdin immediately
        if matches!(self.prompt, PromptInput::String(_)) {
            if let Some(mut stdin) = self.stdin.take() {
                let _ = stdin.shutdown().await;
            }
        }

        Ok(())
    }

    async fn write(&mut self, data: &str) -> Result<()> {
        if !self.is_ready() {
            return Err(ClaudeError::transport("Transport is not ready for writing"));
        }

        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| ClaudeError::transport("stdin not available"))?;

        stdin
            .write_all(data.as_bytes())
            .await
            .map_err(|e| ClaudeError::transport(format!("Failed to write to stdin: {e}")))?;

        stdin
            .flush()
            .await
            .map_err(|e| ClaudeError::transport(format!("Failed to flush stdin: {e}")))?;

        Ok(())
    }

    async fn end_input(&mut self) -> Result<()> {
        if let Some(mut stdin) = self.stdin.take() {
            stdin
                .shutdown()
                .await
                .map_err(|e| ClaudeError::transport(format!("Failed to close stdin: {e}")))?;
        }
        Ok(())
    }

    fn read_messages(&mut self) -> mpsc::UnboundedReceiver<Result<serde_json::Value>> {
        let (tx, rx) = mpsc::unbounded_channel();

        // Take ownership of stdout and process
        let stdout = self.stdout.take();
        let process = Arc::new(Mutex::new(self.process.take()));
        let max_buffer_size = self.max_buffer_size;

        // Spawn background task to read messages
        let task = tokio::spawn(async move {
            if stdout.is_none() {
                let _ = tx.send(Err(ClaudeError::connection(
                    "Not connected - stdout not available",
                )));
                return;
            }

            let mut stdout = stdout.unwrap();
            let mut json_buffer = String::new();

            loop {
                let mut line = String::new();

                // Add timeout to read_line to prevent hanging
                // Increased to 5 minutes for complex suborchestrator operations
                match tokio::time::timeout(
                    std::time::Duration::from_secs(300),
                    stdout.read_line(&mut line)
                ).await {
                    Ok(Ok(0)) => break, // EOF
                    Ok(Ok(_)) => {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }

                        // Accumulate partial JSON until we can parse it
                        json_buffer.push_str(line);

                        if json_buffer.len() > max_buffer_size {
                            let _ = tx.send(Err(ClaudeError::JsonDecode(
                                serde_json::Error::io(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    format!(
                                        "JSON message exceeded maximum buffer size of {max_buffer_size} bytes"
                                    ),
                                )),
                            )));
                            json_buffer.clear();
                            continue;
                        }

                        // Try to parse JSON
                        match serde_json::from_str::<serde_json::Value>(&json_buffer) {
                            Ok(data) => {
                                json_buffer.clear();
                                if tx.send(Ok(data)).is_err() {
                                    // Receiver dropped, stop reading
                                    break;
                                }
                            }
                            Err(_) => {
                                // Not complete yet, continue accumulating
                                // The timeout on read_line will handle incomplete JSON timeouts
                                continue;
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        let _ = tx.send(Err(ClaudeError::Io(e)));
                        break;
                    }
                    Err(_) => {
                        let _ = tx.send(Err(ClaudeError::timeout("Read operation timed out")));
                        break;
                    }
                }
            }

            // Check process exit code
            if let Ok(mut process_guard) = process.try_lock() {
                if let Some(mut child) = process_guard.take() {
                    match child.wait().await {
                        Ok(status) => {
                            if !status.success() {
                                if let Some(code) = status.code() {
                                    let _ = tx.send(Err(ClaudeError::process(
                                        "Command failed",
                                        code,
                                        Some("Check stderr output for details".to_string()),
                                    )));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Err(ClaudeError::Io(e)));
                        }
                    }
                }
            }
        });

        // Store task handle for cleanup
        self.reader_task = Some(task);

        rx
    }

    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    async fn close(&mut self) -> Result<()> {
        self.ready.store(false, Ordering::SeqCst);

        // Close stdin to signal the process to exit gracefully
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.shutdown().await;
        }

        // Abort reader and stderr tasks first to prevent race conditions
        if let Some(task) = self.reader_task.take() {
            task.abort();
            // Give the task a moment to clean up
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        if let Some(task) = self.stderr_task.take() {
            task.abort();
        }

        self.stdout = None;

        // Try to wait for the process to exit gracefully first
        if let Some(mut child) = self.process.take() {
            // Give the process a configurable timeout to exit gracefully
            let timeout_duration = std::time::Duration::from_secs(5);

            match tokio::time::timeout(timeout_duration, child.wait()).await {
                Ok(Ok(_status)) => {
                    // Process exited gracefully
                }
                Ok(Err(e)) => {
                    return Err(ClaudeError::Io(e));
                }
                Err(_) => {
                    // Timeout - kill the process
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                }
            }
        }

        Ok(())
    }
}

impl Drop for SubprocessTransport {
    fn drop(&mut self) {
        // Close stdin if still open to signal graceful shutdown
        if let Some(stdin) = self.stdin.take() {
            // Drop will close it
            drop(stdin);
        }

        // Abort reader task if running
        if let Some(task) = self.reader_task.take() {
            task.abort();
        }

        // Abort stderr task if running
        if let Some(task) = self.stderr_task.take() {
            task.abort();
        }

        // Try graceful shutdown first, then kill if needed
        if let Some(mut child) = self.process.take() {
            // Try to kill gracefully (SIGTERM on Unix)
            let _ = child.start_kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_cli() {
        // This will succeed if claude is installed
        let result = SubprocessTransport::find_cli();
        // We can't assert success because it depends on environment
        println!("CLI search result: {result:?}");
    }

    #[test]
    fn test_prompt_input_conversions() {
        let _prompt1: PromptInput = "hello".into();
        let _prompt2: PromptInput = String::from("world").into();
    }
}
