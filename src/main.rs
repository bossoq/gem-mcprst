use async_mcp::server::Server;
use async_mcp::transport::ServerStdioTransport;
use async_mcp::types::{
    CallToolRequest, CallToolResponse, ServerCapabilities, Tool, ToolResponseContent,
};
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
    // 1. Check for the API Key immediately to provide better startup feedback
    let api_key_status = if env::var("GEMINI_API_KEY").is_ok() {
        "configured ✅"
    } else {
        "MISSING ❌ (Please set GEMINI_API_KEY environment variable)"
    };

    eprintln!("🚀 Starting gemini-executor MCP server...");
    eprintln!("🔑 Gemini API Key: {}", api_key_status);

    // 2. Define the tool schema exposed to Claude Code
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
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(json!({})),
            ..Default::default()
        });

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

    let response = client.post(&url).json(&payload).send().await?;

    let gemini_data: GeminiResponse = response.json().await?;

    // Extract generated text content
    let content_raw = gemini_data
        .candidates
        .first()
        .and_then(|c| c.content.parts.first())
        .map(|p| p.text.as_str())
        .unwrap_or("");

    let content = clean_gemini_response(content_raw);

    // Ensure the target parent directory exists
    let path = Path::new(&args.target_file);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write the output directly to the workspace file
    let mut file = File::create(path)?;
    file.write_all(content.as_bytes())?;

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

/// Cleans up residual Markdown fences (```rust / ```) if Gemini ignored the prompt instruction
fn clean_gemini_response(content: &str) -> String {
    let mut content = content.trim().to_string();
    if content.starts_with("```") && content.ends_with("```") {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() >= 2 {
            content = lines[1..lines.len() - 1].join("\n");
        }
    }
    content.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_gemini_response_with_fences() {
        let input = "```rust\nfn main() {}\n```";
        let expected = "fn main() {}";
        assert_eq!(clean_gemini_response(input), expected);

        let input_no_lang = "```\nhello world\n```";
        let expected_no_lang = "hello world";
        assert_eq!(clean_gemini_response(input_no_lang), expected_no_lang);
    }

    #[test]
    fn test_clean_gemini_response_without_fences() {
        let input = "plain text content";
        let expected = "plain text content";
        assert_eq!(clean_gemini_response(input), expected);
    }

    #[test]
    fn test_clean_gemini_response_empty() {
        assert_eq!(clean_gemini_response(""), "");
        assert_eq!(clean_gemini_response("   "), "");
    }

    #[test]
    fn test_execute_task_args_deserialization() {
        let json = json!({
            "prompt": "write a function",
            "target_file": "src/lib.rs"
        });
        let args: ExecuteTaskArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.prompt, "write a function");
        assert_eq!(args.target_file, "src/lib.rs");
    }
}
