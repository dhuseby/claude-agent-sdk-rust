//! Transport layer for communicating with Claude Code CLI
//!
//! This module provides the transport abstraction and implementations for
//! communicating with the Claude Code CLI process.

pub mod subprocess;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::Result;

/// Transport trait for communicating with Claude Code
///
/// This trait defines the interface for sending and receiving messages
/// to/from the Claude Code CLI process.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Connect to the transport
    ///
    /// # Errors
    /// Returns error if connection fails
    async fn connect(&mut self) -> Result<()>;

    /// Write data to the transport
    ///
    /// # Arguments
    /// * `data` - String data to write (typically JSON)
    ///
    /// # Errors
    /// Returns error if write fails or transport is not ready
    async fn write(&mut self, data: &str) -> Result<()>;

    /// End the input stream (close stdin)
    ///
    /// # Errors
    /// Returns error if closing fails
    async fn end_input(&mut self) -> Result<()>;

    /// Read messages from the transport
    ///
    /// Returns a receiver that yields JSON values representing messages from Claude Code.
    /// This method spawns a background task to read messages, allowing concurrent writes.
    /// The receiver will be closed when the transport ends or encounters an error.
    fn read_messages(&mut self) -> mpsc::UnboundedReceiver<Result<serde_json::Value>>;

    /// Check if transport is ready for communication
    fn is_ready(&self) -> bool;

    /// Close the transport and clean up resources
    ///
    /// # Errors
    /// Returns error if cleanup fails
    async fn close(&mut self) -> Result<()>;
}

pub use subprocess::{PromptInput, SubprocessTransport};
