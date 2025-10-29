//! Type definitions for Claude Agent SDK
//!
//! This module contains all the type definitions used throughout the SDK,
//! including newtypes for type safety, message types, option types, and more.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use crate::error::Result;

// ============================================================================
// Newtype Wrappers for Type Safety
// ============================================================================

/// Session ID newtype for type safety
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(String);

impl SessionId {
    /// Create a new session ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the session ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self("default".to_string())
    }
}

impl From<String> for SessionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SessionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Tool name newtype
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolName(String);

impl ToolName {
    /// Create a new tool name
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the tool name as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ToolName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ToolName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Request ID newtype for control protocol
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(String);

impl RequestId {
    /// Create a new request ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the request ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for RequestId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for RequestId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// ============================================================================
// Permission Types
// ============================================================================

/// Permission modes for tool execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// Default mode - CLI prompts for dangerous tools
    Default,
    /// Auto-accept file edits
    AcceptEdits,
    /// Plan mode
    Plan,
    /// Allow all tools (use with caution)
    BypassPermissions,
}

/// Setting source types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingSource {
    /// User-level settings
    User,
    /// Project-level settings
    Project,
    /// Local settings
    Local,
}

/// Permission update destination
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionUpdateDestination {
    /// Save to user settings
    UserSettings,
    /// Save to project settings
    ProjectSettings,
    /// Save to local settings
    LocalSettings,
    /// Save to session only (temporary)
    Session,
}

/// Permission behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionBehavior {
    /// Allow the action
    Allow,
    /// Deny the action
    Deny,
    /// Ask the user
    Ask,
}

/// Permission rule value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRuleValue {
    /// Name of the tool
    pub tool_name: String,
    /// Optional rule content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_content: Option<String>,
}

/// Permission update configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PermissionUpdate {
    /// Add permission rules
    AddRules {
        /// Rules to add
        #[serde(skip_serializing_if = "Option::is_none")]
        rules: Option<Vec<PermissionRuleValue>>,
        /// Where to save the rules
        #[serde(skip_serializing_if = "Option::is_none")]
        destination: Option<PermissionUpdateDestination>,
    },
    /// Replace existing permission rules
    ReplaceRules {
        /// New rules
        #[serde(skip_serializing_if = "Option::is_none")]
        rules: Option<Vec<PermissionRuleValue>>,
        /// Where to save the rules
        #[serde(skip_serializing_if = "Option::is_none")]
        destination: Option<PermissionUpdateDestination>,
    },
    /// Remove permission rules
    RemoveRules {
        /// Rules to remove
        #[serde(skip_serializing_if = "Option::is_none")]
        rules: Option<Vec<PermissionRuleValue>>,
        /// Where to remove from
        #[serde(skip_serializing_if = "Option::is_none")]
        destination: Option<PermissionUpdateDestination>,
    },
    /// Set permission mode
    SetMode {
        /// New permission mode
        mode: PermissionMode,
        /// Where to save the mode
        #[serde(skip_serializing_if = "Option::is_none")]
        destination: Option<PermissionUpdateDestination>,
    },
    /// Add directories to allowed list
    AddDirectories {
        /// Directories to add
        #[serde(skip_serializing_if = "Option::is_none")]
        directories: Option<Vec<String>>,
        /// Where to save
        #[serde(skip_serializing_if = "Option::is_none")]
        destination: Option<PermissionUpdateDestination>,
    },
    /// Remove directories from allowed list
    RemoveDirectories {
        /// Directories to remove
        #[serde(skip_serializing_if = "Option::is_none")]
        directories: Option<Vec<String>>,
        /// Where to remove from
        #[serde(skip_serializing_if = "Option::is_none")]
        destination: Option<PermissionUpdateDestination>,
    },
}

/// Context for tool permission callbacks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPermissionContext {
    /// Permission suggestions from CLI
    pub suggestions: Vec<PermissionUpdate>,
}

/// Permission request from CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    /// Tool name being requested
    pub tool_name: ToolName,
    /// Tool input parameters
    pub tool_input: serde_json::Value,
    /// Permission context
    pub context: ToolPermissionContext,
}

/// Permission result for allowing tool use
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResultAllow {
    /// Modified input for the tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<serde_json::Value>,
    /// Permission updates to apply
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_permissions: Option<Vec<PermissionUpdate>>,
}

/// Permission result for denying tool use
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResultDeny {
    /// Reason for denying
    pub message: String,
    /// Whether to interrupt the conversation
    #[serde(default)]
    pub interrupt: bool,
}

/// Permission result enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PermissionResult {
    /// Allow the tool use
    Allow(PermissionResultAllow),
    /// Deny the tool use
    Deny(PermissionResultDeny),
}

/// Callback type for tool permission checks
pub type CanUseToolCallback = Arc<
    dyn Fn(
            ToolName,
            serde_json::Value,
            ToolPermissionContext,
        ) -> Pin<Box<dyn Future<Output = Result<PermissionResult>> + Send>>
        + Send
        + Sync,
>;

// ============================================================================
// Hook Types
// ============================================================================

/// Hook event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    /// Before a tool is used
    PreToolUse,
    /// After a tool is used
    PostToolUse,
    /// When user submits a prompt
    UserPromptSubmit,
    /// When conversation stops
    Stop,
    /// When a subagent stops
    SubagentStop,
    /// Before compacting the conversation
    PreCompact,
}

/// Hook decision
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookDecision {
    /// Block the action
    Block,
}

/// Hook output
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookOutput {
    /// Decision to block or allow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<HookDecision>,
    /// System message to add
    #[serde(skip_serializing_if = "Option::is_none", rename = "systemMessage")]
    pub system_message: Option<String>,
    /// Hook-specific output data
    #[serde(skip_serializing_if = "Option::is_none", rename = "hookSpecificOutput")]
    pub hook_specific_output: Option<serde_json::Value>,
}

/// Context for hook callbacks
#[derive(Debug, Clone)]
pub struct HookContext {
    // Future: abort signal support
}

/// Hook callback type
pub type HookCallback = Arc<
    dyn Fn(
            serde_json::Value,
            Option<String>,
            HookContext,
        ) -> Pin<Box<dyn Future<Output = Result<HookOutput>> + Send>>
        + Send
        + Sync,
>;

/// Hook matcher configuration
#[derive(Clone)]
pub struct HookMatcher {
    /// Matcher pattern (e.g., tool name like "Bash" or pattern like "Write|Edit")
    pub matcher: Option<String>,
    /// List of hook callbacks
    pub hooks: Vec<HookCallback>,
}

impl std::fmt::Debug for HookMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookMatcher")
            .field("matcher", &self.matcher)
            .field("hooks", &format!("[{} callbacks]", self.hooks.len()))
            .finish()
    }
}

// ============================================================================
// MCP Server Types
// ============================================================================

/// MCP stdio server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStdioServerConfig {
    /// Server type (stdio)
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub server_type: Option<String>,
    /// Command to execute
    pub command: String,
    /// Command arguments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    /// Environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
}

/// MCP SSE server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSseServerConfig {
    /// Server type (sse)
    #[serde(rename = "type")]
    pub server_type: String,
    /// Server URL
    pub url: String,
    /// HTTP headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}

/// MCP HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpHttpServerConfig {
    /// Server type (http)
    #[serde(rename = "type")]
    pub server_type: String,
    /// Server URL
    pub url: String,
    /// HTTP headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}

/// SDK MCP server marker (not serialized directly)
#[derive(Debug, Clone)]
pub struct SdkMcpServerMarker {
    /// Server name
    #[allow(dead_code)]
    pub name: String,
    /// Server instance (stored for in-process execution)
    pub instance: std::sync::Arc<crate::mcp::SdkMcpServer>,
}

/// MCP server configuration enum
#[derive(Debug, Clone)]
pub enum McpServerConfig {
    /// Stdio-based MCP server
    Stdio(McpStdioServerConfig),
    /// SSE-based MCP server
    Sse(McpSseServerConfig),
    /// HTTP-based MCP server
    Http(McpHttpServerConfig),
    /// SDK-based in-process MCP server
    Sdk(SdkMcpServerMarker),
}

/// MCP servers container
#[derive(Debug, Clone, Default)]
pub enum McpServers {
    /// No MCP servers
    #[default]
    None,
    /// Dictionary of MCP servers
    Dict(HashMap<String, McpServerConfig>),
    /// Path to MCP servers configuration file
    Path(PathBuf),
}

// ============================================================================
// Message Types
// ============================================================================

/// Content value for tool results
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentValue {
    /// String content
    String(String),
    /// Structured content blocks
    Blocks(Vec<serde_json::Value>),
}

/// Content block types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text content block
    Text {
        /// Text content
        text: String,
    },
    /// Thinking content block (extended thinking)
    Thinking {
        /// Thinking content
        thinking: String,
        /// Signature for verification
        signature: String,
    },
    /// Tool use request
    ToolUse {
        /// Tool use ID
        id: String,
        /// Tool name
        name: String,
        /// Tool input parameters
        input: serde_json::Value,
    },
    /// Tool execution result
    ToolResult {
        /// ID of the tool use this is a result for
        tool_use_id: String,
        /// Result content
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<ContentValue>,
        /// Whether this is an error result
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// User message content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessageContent {
    /// Message role (always "user")
    pub role: String,
    /// Message content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<UserContent>,
}

/// User content can be string or blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserContent {
    /// Plain string content
    String(String),
    /// Structured content blocks
    Blocks(Vec<ContentBlock>),
}

/// Assistant message content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessageContent {
    /// Model that generated the message
    pub model: String,
    /// Message content blocks
    pub content: Vec<ContentBlock>,
}

/// Message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// User message
    User {
        /// Parent tool use ID for nested conversations
        #[serde(skip_serializing_if = "Option::is_none")]
        parent_tool_use_id: Option<String>,
        /// Message content
        message: UserMessageContent,
        /// Session ID
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<SessionId>,
    },
    /// Assistant message
    Assistant {
        /// Parent tool use ID for nested conversations
        #[serde(skip_serializing_if = "Option::is_none")]
        parent_tool_use_id: Option<String>,
        /// Message content
        message: AssistantMessageContent,
        /// Session ID
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<SessionId>,
    },
    /// System message
    System {
        /// System message subtype
        subtype: String,
        /// Additional system message data
        #[serde(flatten)]
        data: serde_json::Value,
    },
    /// Result message with metrics
    Result {
        /// Result subtype
        subtype: String,
        /// Total duration in milliseconds
        duration_ms: u64,
        /// API call duration in milliseconds
        duration_api_ms: u64,
        /// Whether this is an error result
        is_error: bool,
        /// Number of conversation turns
        num_turns: u32,
        /// Session ID
        session_id: SessionId,
        /// Total cost in USD
        #[serde(skip_serializing_if = "Option::is_none")]
        total_cost_usd: Option<f64>,
        /// Token usage statistics
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<serde_json::Value>,
        /// Result message
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
    },
    /// Stream event for partial messages
    StreamEvent {
        /// Event UUID
        uuid: String,
        /// Session ID
        session_id: SessionId,
        /// Raw stream event data
        event: serde_json::Value,
        /// Parent tool use ID
        #[serde(skip_serializing_if = "Option::is_none")]
        parent_tool_use_id: Option<String>,
    },
}

// ============================================================================
// System Prompt Types
// ============================================================================

/// System prompt preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPromptPreset {
    /// Prompt type (always "preset")
    #[serde(rename = "type")]
    pub prompt_type: String,
    /// Preset name (e.g., "claude_code")
    pub preset: String,
    /// Additional text to append to the preset
    #[serde(skip_serializing_if = "Option::is_none")]
    pub append: Option<String>,
}

/// System prompt configuration
#[derive(Debug, Clone)]
pub enum SystemPrompt {
    /// Plain string system prompt
    String(String),
    /// Preset-based system prompt
    Preset(SystemPromptPreset),
}

// ============================================================================
// Agent Definition
// ============================================================================

/// Agent definition configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Agent description
    pub description: String,
    /// Agent system prompt
    pub prompt: String,
    /// Tools available to the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    /// Model to use for the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

// ============================================================================
// Claude Agent Options
// ============================================================================

/// Main options for Claude Agent SDK
#[derive(Clone, Default)]
pub struct ClaudeAgentOptions {
    /// List of tools that Claude is allowed to use
    pub allowed_tools: Vec<ToolName>,
    /// System prompt configuration
    pub system_prompt: Option<SystemPrompt>,
    /// MCP server configurations
    pub mcp_servers: McpServers,
    /// Permission mode for tool execution
    pub permission_mode: Option<PermissionMode>,
    /// Whether to continue from the previous conversation
    pub continue_conversation: bool,
    /// Session ID to resume from
    pub resume: Option<SessionId>,
    /// Maximum number of turns before stopping
    pub max_turns: Option<u32>,
    /// List of tools that Claude is not allowed to use
    pub disallowed_tools: Vec<ToolName>,
    /// AI model to use
    pub model: Option<String>,
    /// Tool name to use for permission prompts
    pub permission_prompt_tool_name: Option<String>,
    /// Working directory for the CLI process
    pub cwd: Option<PathBuf>,
    /// Path to settings file
    pub settings: Option<PathBuf>,
    /// Additional directories to add to the context
    pub add_dirs: Vec<PathBuf>,
    /// Environment variables for the CLI process
    pub env: HashMap<String, String>,
    /// Extra CLI arguments to pass
    pub extra_args: HashMap<String, Option<String>>,
    /// Maximum buffer size for JSON messages (default: 1MB)
    pub max_buffer_size: Option<usize>,
    /// Callback for tool permission checks
    pub can_use_tool: Option<CanUseToolCallback>,
    /// Hook configurations
    pub hooks: Option<HashMap<HookEvent, Vec<HookMatcher>>>,
    /// User identifier
    pub user: Option<String>,
    /// Whether to include partial messages in stream
    pub include_partial_messages: bool,
    /// Whether to fork the session when resuming
    pub fork_session: bool,
    /// Custom agent definitions
    pub agents: Option<HashMap<String, AgentDefinition>>,
    /// Setting sources to load
    pub setting_sources: Option<Vec<SettingSource>>,
}

impl ClaudeAgentOptions {
    /// Create a new builder for ClaudeAgentOptions
    pub fn builder() -> ClaudeAgentOptionsBuilder {
        ClaudeAgentOptionsBuilder::default()
    }
}

impl std::fmt::Debug for ClaudeAgentOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeAgentOptions")
            .field("allowed_tools", &self.allowed_tools)
            .field("system_prompt", &self.system_prompt)
            .field("mcp_servers", &self.mcp_servers)
            .field("permission_mode", &self.permission_mode)
            .field("continue_conversation", &self.continue_conversation)
            .field("resume", &self.resume)
            .field("max_turns", &self.max_turns)
            .field("disallowed_tools", &self.disallowed_tools)
            .field("model", &self.model)
            .field(
                "permission_prompt_tool_name",
                &self.permission_prompt_tool_name,
            )
            .field("cwd", &self.cwd)
            .field("settings", &self.settings)
            .field("add_dirs", &self.add_dirs)
            .field("env", &self.env)
            .field("extra_args", &self.extra_args)
            .field("max_buffer_size", &self.max_buffer_size)
            .field(
                "can_use_tool",
                &self.can_use_tool.as_ref().map(|_| "<callback>"),
            )
            .field(
                "hooks",
                &self
                    .hooks
                    .as_ref()
                    .map(|h| format!("[{} hook types]", h.len())),
            )
            .field("user", &self.user)
            .field("include_partial_messages", &self.include_partial_messages)
            .field("fork_session", &self.fork_session)
            .field("agents", &self.agents)
            .field("setting_sources", &self.setting_sources)
            .finish()
    }
}

// ============================================================================
// Builder for ClaudeAgentOptions
// ============================================================================

/// Builder for ClaudeAgentOptions
#[derive(Debug, Default)]
pub struct ClaudeAgentOptionsBuilder {
    options: ClaudeAgentOptions,
}

impl ClaudeAgentOptionsBuilder {
    /// Set allowed tools
    pub fn allowed_tools(mut self, tools: Vec<impl Into<ToolName>>) -> Self {
        self.options.allowed_tools = tools.into_iter().map(|t| t.into()).collect();
        self
    }

    /// Add an allowed tool
    pub fn add_allowed_tool(mut self, tool: impl Into<ToolName>) -> Self {
        self.options.allowed_tools.push(tool.into());
        self
    }

    /// Set system prompt
    pub fn system_prompt(mut self, prompt: impl Into<SystemPrompt>) -> Self {
        self.options.system_prompt = Some(prompt.into());
        self
    }

    /// Set MCP servers
    pub fn mcp_servers(mut self, servers: HashMap<String, McpServerConfig>) -> Self {
        self.options.mcp_servers = McpServers::Dict(servers);
        self
    }

    /// Set permission mode
    pub fn permission_mode(mut self, mode: PermissionMode) -> Self {
        self.options.permission_mode = Some(mode);
        self
    }

    /// Set max turns
    pub fn max_turns(mut self, turns: u32) -> Self {
        const MAX_ALLOWED_TURNS: u32 = 1000;
        if turns > MAX_ALLOWED_TURNS {
            panic!(
                "max_turns {turns} exceeds maximum allowed: {MAX_ALLOWED_TURNS}"
            );
        }
        self.options.max_turns = Some(turns);
        self
    }

    /// Set working directory
    pub fn cwd(mut self, path: impl Into<PathBuf>) -> Self {
        self.options.cwd = Some(path.into());
        self
    }

    /// Set can_use_tool callback
    pub fn can_use_tool(mut self, callback: CanUseToolCallback) -> Self {
        self.options.can_use_tool = Some(callback);
        self
    }

    /// Set hooks
    pub fn hooks(mut self, hooks: HashMap<HookEvent, Vec<HookMatcher>>) -> Self {
        self.options.hooks = Some(hooks);
        self
    }

    /// Set custom agent definitions
    ///
    /// This method replaces any previously configured agents. To add agents
    /// incrementally, use `add_agent()` instead.
    ///
    /// # Arguments
    ///
    /// * `agents` - HashMap of agent name to AgentDefinition
    ///
    /// # Example
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use claude_agent_sdk::{ClaudeAgentOptions, AgentDefinition};
    ///
    /// let mut agents = HashMap::new();
    /// agents.insert("reviewer".to_string(), AgentDefinition {
    ///     description: "Code reviewer".to_string(),
    ///     prompt: "Review code thoroughly".to_string(),
    ///     tools: Some(vec!["Read".to_string()]),
    ///     model: Some("sonnet".to_string()),
    /// });
    ///
    /// let options = ClaudeAgentOptions::builder()
    ///     .agents(agents)
    ///     .build();
    /// ```
    pub fn agents(mut self, agents: HashMap<String, AgentDefinition>) -> Self {
        self.options.agents = Some(agents);
        self
    }

    /// Add a custom agent definition
    ///
    /// This method adds a single agent to the configuration. Multiple calls
    /// can be chained to add multiple agents. If an agent with the same name
    /// already exists, it will be replaced.
    ///
    /// # Arguments
    ///
    /// * `name` - The agent name/identifier
    /// * `agent` - The agent definition
    ///
    /// # Example
    ///
    /// ```
    /// use claude_agent_sdk::{ClaudeAgentOptions, AgentDefinition};
    ///
    /// let options = ClaudeAgentOptions::builder()
    ///     .add_agent("code-reviewer", AgentDefinition {
    ///         description: "Reviews code for best practices".to_string(),
    ///         prompt: "You are a code reviewer...".to_string(),
    ///         tools: Some(vec!["Read".to_string(), "Grep".to_string()]),
    ///         model: Some("sonnet".to_string()),
    ///     })
    ///     .add_agent("tester", AgentDefinition {
    ///         description: "Runs tests".to_string(),
    ///         prompt: "You are a testing expert...".to_string(),
    ///         tools: Some(vec!["Bash".to_string()]),
    ///         model: None,
    ///     })
    ///     .build();
    /// ```
    pub fn add_agent(mut self, name: impl Into<String>, agent: AgentDefinition) -> Self {
        self.options
            .agents
            .get_or_insert_with(HashMap::new)
            .insert(name.into(), agent);
        self
    }

    /// Build the options
    pub fn build(self) -> ClaudeAgentOptions {
        self.options
    }
}

// Implement conversions for SystemPrompt
impl From<String> for SystemPrompt {
    fn from(s: String) -> Self {
        SystemPrompt::String(s)
    }
}

impl From<&str> for SystemPrompt {
    fn from(s: &str) -> Self {
        SystemPrompt::String(s.to_string())
    }
}

impl From<SystemPromptPreset> for SystemPrompt {
    fn from(preset: SystemPromptPreset) -> Self {
        SystemPrompt::Preset(preset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_agents() {
        let mut agents = HashMap::new();
        agents.insert(
            "reviewer".to_string(),
            AgentDefinition {
                description: "Code reviewer".to_string(),
                prompt: "Review code".to_string(),
                tools: Some(vec!["Read".to_string()]),
                model: Some("sonnet".to_string()),
            },
        );

        let options = ClaudeAgentOptions::builder()
            .agents(agents.clone())
            .build();

        assert_eq!(options.agents, Some(agents));
    }

    #[test]
    fn test_builder_add_agent() {
        let agent = AgentDefinition {
            description: "Code reviewer".to_string(),
            prompt: "Review code".to_string(),
            tools: Some(vec!["Read".to_string()]),
            model: Some("sonnet".to_string()),
        };

        let options = ClaudeAgentOptions::builder()
            .add_agent("reviewer", agent.clone())
            .build();

        assert!(options.agents.is_some());
        assert_eq!(options.agents.unwrap().get("reviewer"), Some(&agent));
    }

    #[test]
    fn test_builder_add_multiple_agents() {
        let reviewer = AgentDefinition {
            description: "Code reviewer".to_string(),
            prompt: "Review code".to_string(),
            tools: Some(vec!["Read".to_string()]),
            model: Some("sonnet".to_string()),
        };

        let tester = AgentDefinition {
            description: "Test runner".to_string(),
            prompt: "Run tests".to_string(),
            tools: Some(vec!["Bash".to_string()]),
            model: None,
        };

        let options = ClaudeAgentOptions::builder()
            .add_agent("reviewer", reviewer.clone())
            .add_agent("tester", tester.clone())
            .build();

        let agents = options.agents.unwrap();
        assert_eq!(agents.len(), 2);
        assert_eq!(agents.get("reviewer"), Some(&reviewer));
        assert_eq!(agents.get("tester"), Some(&tester));
    }

    #[test]
    fn test_builder_agents_mixed() {
        let mut initial_agents = HashMap::new();
        initial_agents.insert(
            "reviewer".to_string(),
            AgentDefinition {
                description: "Code reviewer".to_string(),
                prompt: "Review code".to_string(),
                tools: None,
                model: None,
            },
        );

        let tester = AgentDefinition {
            description: "Test runner".to_string(),
            prompt: "Run tests".to_string(),
            tools: None,
            model: None,
        };

        let options = ClaudeAgentOptions::builder()
            .agents(initial_agents)
            .add_agent("tester", tester)
            .build();

        let agents = options.agents.unwrap();
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn test_builder_agent_replacement() {
        let agent_v1 = AgentDefinition {
            description: "Version 1".to_string(),
            prompt: "Old prompt".to_string(),
            tools: None,
            model: None,
        };

        let agent_v2 = AgentDefinition {
            description: "Version 2".to_string(),
            prompt: "New prompt".to_string(),
            tools: None,
            model: None,
        };

        let options = ClaudeAgentOptions::builder()
            .add_agent("test", agent_v1)
            .add_agent("test", agent_v2.clone())
            .build();

        let agents = options.agents.unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents.get("test"), Some(&agent_v2));
    }
}
