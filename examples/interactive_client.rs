//! Interactive client example
//!
//! This example demonstrates using ClaudeSDKClient for bidirectional
//! communication with Claude Code. The client supports sending messages
//! while receiving responses, enabling truly interactive conversations.

use claude_agent_sdk::{ClaudeAgentOptions, ClaudeSDKClient, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating Claude SDK client...\n");

    // Create client with limited turns for this demo
    let options = ClaudeAgentOptions::builder().max_turns(3).build();

    let mut client = ClaudeSDKClient::new(options, None).await?;

    println!("Client created successfully!");
    println!("Sending message: 'What is 2 + 2?'\n");

    // Send initial message
    client.send_message("What is 2 + 2?").await?;

    // Read and display responses
    while let Some(message) = client.next_message().await {
        match message {
            Ok(msg) => {
                match msg {
                    Message::Assistant { message, .. } => {
                        for block in &message.content {
                            if let claude_agent_sdk::ContentBlock::Text { text } = block {
                                println!("Claude: {text}\n");
                            }
                        }
                    }
                    Message::Result {
                        total_cost_usd: Some(cost),
                        ..
                    } => {
                        println!("Cost: ${cost:.4}\n");
                        // Result message indicates conversation is complete
                        break;
                    }
                    Message::Result { .. } => {
                        // Result without cost also indicates completion
                        break;
                    }
                    _ => {}
                }
            }
            Err(e) => {
                eprintln!("Error: {e}");
                break;
            }
        }
    }

    // Close client
    client.close().await?;

    println!("Client closed successfully.");
    println!("\nThis demonstrates true bidirectional communication!");
    println!("The client can send messages while receiving responses.");

    Ok(())
}
