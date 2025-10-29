//! Standalone TTS Demo
//!
//! This example demonstrates ElevenLabs TTS integration without Claude CLI.
//! Shows that the TTS MCP tool works correctly.
//!
//! Requirements:
//! - Set ELEVENLABS_API_KEY environment variable
//! - Audio file will be saved to tts_output.mp3
//!
//! Run with: ELEVENLABS_API_KEY=your_key cargo run --example tts_standalone_demo

use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
use elevenlabs_rs::{ElevenLabsClient, DefaultVoice, Model};
use elevenlabs_rs::endpoints::genai::tts::{TextToSpeech, TextToSpeechBody};
use serde_json::json;
use std::fs::File;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Standalone TTS Demo ===\n");
    println!("Testing ElevenLabs API integration...\n");

    // Check for API key
    if std::env::var("ELEVENLABS_API_KEY").is_err() {
        eprintln!("❌ Error: ELEVENLABS_API_KEY environment variable not set");
        eprintln!("Please run: export ELEVENLABS_API_KEY=your_key");
        return Ok(());
    }

    // Create TTS tool
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

                println!("🔊 Speaking: \"{}\"", text);

                // Call ElevenLabs API (async - elevenlabs_rs is async-first)
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

                    // Save audio
                    let mut file = File::create("tts_output.mp3")
                        .map_err(|e| format!("File create error: {}", e))?;
                    file.write_all(&audio_bytes)
                        .map_err(|e| format!("File write error: {}", e))?;

                    Ok::<_, String>(text)
                }.await {
                    Ok(spoken_text) => {
                        println!("✓ Audio saved to tts_output.mp3");
                        Ok(ToolResult::text(format!("Spoke: \"{}\" (saved to tts_output.mp3)", spoken_text)))
                    }
                    Err(e) => {
                        eprintln!("❌ Error: {}", e);
                        Err(claude_agent_sdk::error::ClaudeError::mcp(e))
                    }
                }
            })
        },
    );

    // Create server
    let server = SdkMcpServer::new("tts").version("1.0.0").tool(tts_tool);

    // Test the tool directly
    println!("Testing tool invocation...\n");

    let request = claude_agent_sdk::mcp::protocol::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "speak",
            "arguments": {
                "text": "Task complete. All systems operational."
            }
        })),
    };

    match server.handle_request(request).await {
        Ok(response) => {
            if let Some(result) = response.result {
                println!("\n✓ Tool executed successfully!");
                println!("Response: {}", result);
            } else if let Some(error) = response.error {
                eprintln!("\n❌ Tool returned error: {}", error.message);
            }
        }
        Err(e) => {
            eprintln!("\n❌ Failed to execute tool: {}", e);
        }
    }

    println!("\n=== Demo Complete ===");
    println!("\nYou can play the audio with:");
    println!("  mpv tts_output.mp3    # or");
    println!("  ffplay tts_output.mp3 # or");
    println!("  open tts_output.mp3   # on macOS");

    Ok(())
}
