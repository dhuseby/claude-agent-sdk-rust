//! TTS Notification Demo
//!
//! This example demonstrates:
//! - Creating a custom TTS MCP tool using ElevenLabs API
//! - Using hooks to trigger TTS on conversation Stop
//! - Real voice notification when task completes
//!
//! Requirements:
//! - Set ELEVENLABS_API_KEY environment variable
//! - Audio will be played directly (no file saved)
//!
//! Run with: ELEVENLABS_API_KEY=your_key cargo run --example tts_notify_demo

use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
use claude_agent_sdk::types::{
    ClaudeAgentOptions, HookContext, HookEvent, HookMatcher, HookOutput, McpServerConfig,
    McpServers, Message, SdkMcpServerMarker, ToolName,
};
use claude_agent_sdk::ClaudeSDKClient;
use elevenlabs_rs::{ElevenLabsClient, DefaultVoice, Model};
use elevenlabs_rs::endpoints::genai::tts::{TextToSpeech, TextToSpeechBody};
use elevenlabs_rs::utils::play;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== TTS Notification Demo ===\n");
    println!("This demo uses ElevenLabs API for real text-to-speech.\n");

    // Check for API key
    if std::env::var("ELEVENLABS_API_KEY").is_err() {
        eprintln!("❌ Error: ELEVENLABS_API_KEY environment variable not set");
        eprintln!("Please run: export ELEVENLABS_API_KEY=your_key");
        return Ok(());
    }

    // Create real TTS tool using ElevenLabs API
    let tts_tool = SdkMcpTool::new(
        "speak",
        "Convert text to speech using ElevenLabs API",
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to convert to speech"
                }
            },
            "required": ["text"]
        }),
        |input| {
            Box::pin(async move {
                let text = input["text"].as_str().unwrap_or("").to_string();

                println!("\n  🔊 [TTS] Speaking: \"{}\"", text);

                // Call ElevenLabs API
                match async {
                    let client = ElevenLabsClient::from_env()
                        .map_err(|e| format!("Auth error: {}", e))?;

                    let body = TextToSpeechBody::new(text.clone())
                        .with_model_id(Model::ElevenMultilingualV2);

                    let endpoint = TextToSpeech::new(
                        DefaultVoice::Brian,
                        body
                    );

                    let audio_bytes = client.hit(endpoint).await
                        .map_err(|e| format!("TTS error: {}", e))?;

                    // Play audio directly in blocking context
                    println!("  ♪ [TTS] Playing audio...");
                    tokio::task::spawn_blocking(move || {
                        play(audio_bytes)
                    })
                    .await
                    .map_err(|e| format!("Task join error: {}", e))?
                    .map_err(|e| format!("Playback error: {}", e))?;

                    Ok::<_, String>(text)
                }.await {
                    Ok(spoken_text) => {
                        println!("  ✓ [TTS] Playback complete");
                        println!("  ✓ [TTS] Notification delivered\n");
                        Ok(ToolResult::text(format!("Spoke: \"{}\"", spoken_text)))
                    }
                    Err(e) => {
                        eprintln!("  ❌ [TTS] Error: {}", e);
                        Err(claude_agent_sdk::error::ClaudeError::mcp(e))
                    }
                }
            })
        },
    );

    // Create TTS MCP server
    let tts_server = Arc::new(SdkMcpServer::new("tts").version("1.0.0").tool(tts_tool));

    // Hook: Notify when conversation stops
    let notify_on_stop = |input: serde_json::Value, _tool_name: Option<String>, _context: HookContext| {
        Box::pin(async move {
            println!("\n  📢 [HOOK] Conversation stopped - notification triggered");

            // Log hook data
            if let Some(session_id) = input.get("session_id") {
                println!("  📝 [HOOK] Session: {}", session_id);
            }

            // In a real implementation, you could automatically call the TTS tool here
            // For this demo, Claude will be instructed to call it

            Ok(HookOutput::default())
        })
            as std::pin::Pin<
                Box<
                    dyn std::future::Future<
                            Output = Result<HookOutput, claude_agent_sdk::error::ClaudeError>,
                        > + Send,
                >,
            >
    };

    // Configure options
    let mut mcp_servers = HashMap::new();
    mcp_servers.insert(
        "tts".to_string(),
        McpServerConfig::Sdk(SdkMcpServerMarker {
            name: "tts".to_string(),
            instance: tts_server,
        }),
    );

    let mut hooks = HashMap::new();
    hooks.insert(
        HookEvent::Stop,
        vec![HookMatcher {
            matcher: None,
            hooks: vec![Arc::new(notify_on_stop)],
        }],
    );

    let options = ClaudeAgentOptions {
        mcp_servers: McpServers::Dict(mcp_servers),
        hooks: Some(hooks),
        allowed_tools: vec![ToolName::new("mcp__tts__speak"), ToolName::new("Bash")],
        max_turns: Some(3),
        ..Default::default()
    };

    println!("Starting task: List files in current directory\n");

    let mut client = ClaudeSDKClient::new(options, None).await?;
    client
        .send_message(
            "List files in current directory, then use the speak tool to say 'Task complete'"
                .to_string(),
        )
        .await?;

    // Process messages
    while let Some(message) = client.next_message().await {
        match message {
            Ok(Message::Assistant { message, .. }) => {
                for block in &message.content {
                    match block {
                        claude_agent_sdk::types::ContentBlock::Text { text } => {
                            println!("Claude: {}\n", text);
                        }
                        claude_agent_sdk::types::ContentBlock::ToolUse { name, .. } => {
                            println!("  [Tool: {}]", name);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Message::Result { is_error, .. }) => {
                if is_error {
                    println!("\n❌ Task ended with error");
                } else {
                    println!("\n✓ Task completed successfully");
                }
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }

    println!("\n=== Demo Complete ===");
    println!("\nDemonstrated:");
    println!("✓ Custom TTS tool via SDK MCP server");
    println!("✓ Real ElevenLabs API integration");
    println!("✓ Hook on conversation Stop event");
    println!("✓ Voice notification when task completes");
    println!("✓ Audio played directly using rodio");

    Ok(())
}
