/// Simple query example
use claude_agent_sdk::{query, ClaudeAgentOptions, Message};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Simple Query Example");
    println!("===================\n");

    // Simple query with default options
    let stream = query("What is 2 + 2?", None).await?;
    let mut stream = Box::pin(stream);

    println!("Sending query: 'What is 2 + 2?'\n");
    println!("Response:\n");

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let claude_agent_sdk::ContentBlock::Text { text } = block {
                        println!("Claude: {text}");
                    }
                }
            }
            Message::Result {
                total_cost_usd,
                num_turns,
                ..
            } => {
                println!("\n---");
                println!("Completed in {num_turns} turns");
                if let Some(cost) = total_cost_usd {
                    println!("Cost: ${cost:.4}");
                }
                break; // Exit after result
            }
            _ => {}
        }
    }

    println!("\n\nWith custom options:");
    println!("====================\n");

    // Query with custom options
    let options = ClaudeAgentOptions::builder()
        .system_prompt("You are a helpful math tutor. Be concise.")
        .max_turns(1)
        .build();

    let stream2 = query("Explain prime numbers in one sentence", Some(options)).await?;
    let mut stream2 = Box::pin(stream2);

    println!("Sending query: 'Explain prime numbers in one sentence'\n");
    println!("Response:\n");

    while let Some(message) = stream2.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let claude_agent_sdk::ContentBlock::Text { text } = block {
                        println!("Claude: {text}");
                    }
                }
            }
            Message::Result {
                total_cost_usd: Some(cost),
                ..
            } => {
                println!("\nCost: ${cost:.4}");
                break; // Exit after result
            }
            Message::Result { .. } => {
                break; // Exit after result even without cost
            }
            _ => {}
        }
    }

    Ok(())
}
