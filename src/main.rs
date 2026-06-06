use async_mcp::server::Server;
use async_mcp::transport::ServerStdioTransport;
use async_mcp::types::{CallToolRequest, CallToolResponse, Tool, ToolResponseContent};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

// Structures to parse the incoming MCP tool arguments
#[derive(Deserialize)]
struct ExecuteTaskArgs {
    prompt: String,
    target_file: String,
}

// Structures to parse Gemini's API response
#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Content,
}

#[derive(Deserialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Deserialize)]
struct Part {
    text: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Define the tool schema exposed to Claude Code
    let execute_tool = Tool {
        name: "execute_task".to_string(),
        description: Some("Offloads heavy code generation or file writing tasks to Gemini to save context tokens.".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The description of what code Gemini needs to generate."
                },
                "target_file": {
                    "type": "string",
                    "description": "The workspace file path where the generated code should be written."
                }
            },
            "required": ["prompt", "target_file"]
        }),
        output_schema: None,
    };

    // 2. Build and run the server using standard input/output (stdio)
    let mut builder = Server::builder(ServerStdioTransport)
        .name("gemini-executor")
        .version("1.0.0");

    builder.register_tool(execute_tool, |req| Box::pin(handle_execute_task(req)));

    let server = builder.build();
    server.listen().await?;

    Ok(())
}

// The core logic function that executes when Claude invokes the tool
async fn handle_execute_task(request: CallToolRequest) -> anyhow::Result<CallToolResponse> {
    // Deserialize arguments passed by Claude
    let arguments = request.arguments.unwrap_or_default();
    let args_json = serde_json::to_value(arguments)?;
    let args: ExecuteTaskArgs = serde_json::from_value(args_json)?;

    // Pull the Gemini API Key from the environment variables
    let api_key = env::var("GEMINI_API_KEY")
        .map_err(|_| anyhow::anyhow!("Environment variable 'GEMINI_API_KEY' is missing."))?;

    eprintln!("🤖 Forwarding task for {} to Gemini...", args.target_file);

    // Call Gemini API using reqwest
    let client = Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={api_key}"
    );

    let payload = json!({
        "contents": [{
            "parts": [{
                "text": format!(
                    "Task: {}\nTarget File: {}\nReturn ONLY raw code/text content for the file. Do not include markdown code block backticks (```).",
                    args.prompt, args.target_file
                )
            }]
        }]
    });

    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .await?;

    let gemini_data: GeminiResponse = response
        .json()
        .await?;

    // Extract generated text content
    let mut content = gemini_data
        .candidates
        .first()
        .and_then(|c| c.content.parts.first())
        .map(|p| p.text.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    // Clean up residual Markdown fences (```rust / ```) if Gemini ignored the prompt instruction
    if content.starts_with("```") && content.ends_with("```") {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() >= 2 {
            content = lines[1..lines.len() - 1].join("\n");
        }
    }

    // Ensure the target parent directory exists
    let path = Path::new(&args.target_file);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write the output directly to the workspace file
    let mut file = File::create(path)?;
    file.write_all(content.trim().as_bytes())?;

    // Return success response to Claude Code
    Ok(CallToolResponse {
        content: vec![ToolResponseContent::Text {
            text: format!(
                "Successfully offloaded to Gemini. Wrote contents to {}.",
                args.target_file
            ),
        }],
        is_error: None,
        meta: None,
    })
}
