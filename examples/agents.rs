//! Example of using custom agents with Claude Agent SDK
//!
//! This example demonstrates how to define and use custom agents with specific
//! tools, prompts, and models.
//!
//! Usage:
//! cargo run --example agents

use claude_agent_sdk::{query, ClaudeAgentOptions, AgentDefinition, Message, ContentBlock};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    code_reviewer_example().await?;
    multiple_agents_example().await?;
    Ok(())
}

async fn code_reviewer_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Code Reviewer Agent Example ===\n");

    let options = ClaudeAgentOptions::builder()
        .add_agent("code-reviewer", AgentDefinition {
            description: "Reviews code for best practices and potential issues".to_string(),
            prompt: "You are a code reviewer. Analyze code for bugs, performance issues, \
                     security vulnerabilities, and adherence to best practices. \
                     Provide constructive feedback.".to_string(),
            tools: Some(vec!["Read".to_string(), "Grep".to_string()]),
            model: Some("sonnet".to_string()),
        })
        .build();

    let stream = query(
        "Use the code-reviewer agent to review the code in src/types.rs",
        Some(options)
    ).await?;

    print_messages(stream).await?;
    Ok(())
}

async fn multiple_agents_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Multiple Agents Example ===\n");

    let options = ClaudeAgentOptions::builder()
        .add_agent("analyzer", AgentDefinition {
            description: "Analyzes code structure and patterns".to_string(),
            prompt: "You are a code analyzer. Examine code structure, patterns, and architecture.".to_string(),
            tools: Some(vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()]),
            model: None,
        })
        .add_agent("tester", AgentDefinition {
            description: "Creates and runs tests".to_string(),
            prompt: "You are a testing expert. Write comprehensive tests and ensure code quality.".to_string(),
            tools: Some(vec!["Read".to_string(), "Write".to_string(), "Bash".to_string()]),
            model: Some("sonnet".to_string()),
        })
        .build();

    let stream = query(
        "Use the analyzer agent to find all Rust files in the examples/ directory",
        Some(options)
    ).await?;

    print_messages(stream).await?;
    Ok(())
}

/// Helper function to print all messages from the stream with detailed information
async fn print_messages(
    stream: impl futures::Stream<Item = Result<Message, claude_agent_sdk::ClaudeError>>
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, parent_tool_use_id, .. } => {
                println!("\n[Assistant Response - Model: {}]", message.model);
                if let Some(parent) = parent_tool_use_id {
                    println!("  (Parent tool use: {})", parent);
                }
                for block in &message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            println!("\n{}", text);
                        }
                        ContentBlock::ToolUse { id, name, input } => {
                            println!("\n[Using tool: {} (id: {})]", name, id);
                            if let Ok(pretty) = serde_json::to_string_pretty(input) {
                                println!("{}", pretty);
                            }
                        }
                        ContentBlock::Thinking { thinking, .. } => {
                            println!("\n[Thinking]\n{}", thinking);
                        }
                        _ => {}
                    }
                }
            }
            Message::User { message, .. } => {
                println!("\n[User Message]");
                if let Some(content) = &message.content {
                    println!("{:?}", content);
                }
            }
            Message::System { subtype, data } => {
                println!("\n[System Event: {}]", subtype);
                if let Some(obj) = data.as_object() {
                    for (key, value) in obj {
                        println!("  {}: {}", key, value);
                    }
                }
            }
            Message::Result {
                subtype,
                total_cost_usd,
                num_turns,
                duration_ms,
                is_error,
                result,
                ..
            } => {
                println!("\n[Conversation Result - {}]", subtype);
                println!("  Turns: {}", num_turns);
                println!("  Duration: {}ms", duration_ms);
                println!("  Error: {}", is_error);
                if let Some(cost) = total_cost_usd {
                    println!("  Cost: ${:.4}", cost);
                }
                if let Some(msg) = result {
                    println!("  Message: {}", msg);
                }
            }
            Message::StreamEvent { event, .. } => {
                // Optionally print stream events for debugging
                if std::env::var("VERBOSE").is_ok() {
                    println!("\n[Stream Event]");
                    if let Ok(pretty) = serde_json::to_string_pretty(&event) {
                        println!("{}", pretty);
                    }
                }
            }
        }
    }

    println!();
    Ok(())
}
