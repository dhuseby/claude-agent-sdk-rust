//! Hook system for intercepting agent events
//!
//! This module provides the hook system that allows users to intercept
//! and respond to various events in the agent lifecycle.

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{ClaudeError, Result};
use crate::types::{HookCallback, HookContext, HookDecision, HookMatcher, HookOutput};

/// Hook manager for registering and invoking hooks
pub struct HookManager {
    /// Registered hook matchers
    matchers: Vec<HookMatcher>,
    /// Map callback_id -> (matcher_index, callback_index)
    /// Used to look up callbacks when CLI sends hook_callback requests
    callback_id_map: HashMap<String, (usize, usize)>,
    /// Counter for generating unique callback IDs
    next_callback_id: u32,
}

impl HookManager {
    /// Create a new hook manager
    pub fn new() -> Self {
        Self {
            matchers: Vec::new(),
            callback_id_map: HashMap::new(),
            next_callback_id: 0,
        }
    }

    /// Register a hook with a matcher
    ///
    /// # Arguments
    /// * `matcher` - Hook matcher configuration
    pub fn register(&mut self, matcher: HookMatcher) {
        self.matchers.push(matcher);
    }

    /// Register a matcher and generate callback IDs for all its hooks
    ///
    /// # Arguments
    /// * `matcher` - Hook matcher configuration
    ///
    /// # Returns
    /// Vector of generated callback IDs for the hooks in this matcher
    pub fn register_with_ids(&mut self, matcher: HookMatcher) -> Vec<String> {
        let matcher_index = self.matchers.len();
        let mut callback_ids = Vec::new();

        // Generate callback IDs for each hook in this matcher
        for callback_index in 0..matcher.hooks.len() {
            let callback_id = format!("hook_{}", self.next_callback_id);
            self.next_callback_id += 1;

            self.callback_id_map.insert(
                callback_id.clone(),
                (matcher_index, callback_index)
            );
            callback_ids.push(callback_id);
        }

        self.matchers.push(matcher);
        callback_ids
    }

    /// Invoke a callback by its callback_id (used when CLI sends hook_callback request)
    ///
    /// # Arguments
    /// * `callback_id` - The callback ID to invoke
    /// * `event_data` - Event data (JSON value)
    /// * `tool_name` - Optional tool name
    /// * `context` - Hook context
    ///
    /// # Returns
    /// Hook output from the specific callback
    pub async fn invoke_by_id(
        &self,
        callback_id: &str,
        event_data: serde_json::Value,
        tool_name: Option<String>,
        context: HookContext,
    ) -> Result<HookOutput> {
        let (matcher_index, callback_index) = self
            .callback_id_map
            .get(callback_id)
            .ok_or_else(|| ClaudeError::hook(format!("Unknown callback_id: {}", callback_id)))?;

        let matcher = &self.matchers[*matcher_index];
        let callback = &matcher.hooks[*callback_index];

        callback(event_data, tool_name, context).await
    }

    /// Invoke hooks for a given event
    ///
    /// # Arguments
    /// * `event_data` - Event data (JSON value)
    /// * `tool_name` - Optional tool name
    /// * `context` - Hook context
    ///
    /// # Returns
    /// Hook output with optional decision and modifications
    pub async fn invoke(
        &self,
        event_data: serde_json::Value,
        tool_name: Option<String>,
        context: HookContext,
    ) -> Result<HookOutput> {
        let mut output = HookOutput::default();

        // Find matching hooks
        for matcher in &self.matchers {
            if Self::matches(&matcher.matcher, &tool_name) {
                // Invoke each hook callback
                for hook in &matcher.hooks {
                    let result = hook(event_data.clone(), tool_name.clone(), context.clone()).await?;

                    // Merge hook results
                    if result.decision.is_some() {
                        output.decision = result.decision;
                    }
                    if result.system_message.is_some() {
                        output.system_message = result.system_message;
                    }
                    if result.hook_specific_output.is_some() {
                        output.hook_specific_output = result.hook_specific_output;
                    }

                    // If decision is Block, stop processing
                    if matches!(output.decision, Some(HookDecision::Block)) {
                        return Ok(output);
                    }
                }
            }
        }

        Ok(output)
    }

    /// Check if a matcher matches a tool name
    ///
    /// # Security Note
    /// This uses simple pattern matching with pipe-separated alternatives.
    /// For production use with untrusted patterns, consider using a proper
    /// glob or regex library with safety guarantees (e.g., `globset` crate).
    fn matches(matcher: &Option<String>, tool_name: &Option<String>) -> bool {
        match (matcher, tool_name) {
            (None, _) => true, // No matcher = match all
            (Some(pattern), Some(name)) => {
                // Simple wildcard matching
                if pattern == "*" {
                    return true;
                }
                // Exact match or simple pipe-separated pattern
                // Note: This doesn't handle edge cases like pipe characters in tool names
                pattern == name || pattern.split('|').any(|p| p == name)
            }
            (Some(_), None) => false,
        }
    }

    /// Create a hook callback from a closure
    pub fn callback<F, Fut>(f: F) -> HookCallback
    where
        F: Fn(serde_json::Value, Option<String>, HookContext) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<HookOutput>> + Send + 'static,
    {
        Arc::new(move |event_data, tool_name, context| {
            Box::pin(f(event_data, tool_name, context))
        })
    }
}

impl Default for HookManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating hook matchers
pub struct HookMatcherBuilder {
    matcher: Option<String>,
    hooks: Vec<HookCallback>,
}

impl HookMatcherBuilder {
    /// Create a new hook matcher builder
    ///
    /// # Arguments
    /// * `pattern` - Matcher pattern (None for all, or specific tool name/pattern)
    pub fn new(pattern: Option<impl Into<String>>) -> Self {
        Self {
            matcher: pattern.map(|p| p.into()),
            hooks: Vec::new(),
        }
    }

    /// Add a hook callback
    pub fn add_hook(mut self, hook: HookCallback) -> Self {
        self.hooks.push(hook);
        self
    }

    /// Build the hook matcher
    pub fn build(self) -> HookMatcher {
        HookMatcher {
            matcher: self.matcher,
            hooks: self.hooks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hook_manager() {
        let mut manager = HookManager::new();

        // Register a hook
        let hook = HookManager::callback(|_event_data, _tool_name, _context| async {
            Ok(HookOutput::default())
        });

        let matcher = HookMatcherBuilder::new(Some("*")).add_hook(hook).build();
        manager.register(matcher);

        // Invoke hook
        let context = HookContext {};
        let result = manager
            .invoke(serde_json::json!({}), Some("test".to_string()), context)
            .await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_matcher_wildcard() {
        assert!(HookManager::matches(
            &Some("*".to_string()),
            &Some("any_tool".to_string())
        ));
        assert!(HookManager::matches(&None, &Some("any_tool".to_string())));
    }

    #[test]
    fn test_matcher_specific() {
        assert!(HookManager::matches(
            &Some("Bash".to_string()),
            &Some("Bash".to_string())
        ));
        assert!(!HookManager::matches(
            &Some("Bash".to_string()),
            &Some("Write".to_string())
        ));
    }

    #[test]
    fn test_matcher_pattern() {
        assert!(HookManager::matches(
            &Some("Write|Edit".to_string()),
            &Some("Write".to_string())
        ));
        assert!(HookManager::matches(
            &Some("Write|Edit".to_string()),
            &Some("Edit".to_string())
        ));
        assert!(!HookManager::matches(
            &Some("Write|Edit".to_string()),
            &Some("Bash".to_string())
        ));
    }
}
