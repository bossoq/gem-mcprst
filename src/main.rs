use async_mcp::server::Server;
use async_mcp::transport::ServerStdioTransport;
use async_mcp::types::{
    CallToolRequest, CallToolResponse, ServerCapabilities, Tool, ToolResponseContent,
};
use serde::Deserialize;
use serde_json::json;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;

// Structures to parse the incoming MCP tool arguments
#[derive(Deserialize)]
struct ExecuteTaskArgs {
    prompt: String,
    target_file: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Check if agy CLI is available in the path
    let cli_check = Command::new("agy").arg("--version").output();
    let cli_status = if cli_check.is_ok() {
        "detected ✅"
    } else {
        "NOT FOUND ❌ (Please ensure 'agy' CLI is installed and in your PATH)"
    };

    eprintln!("🚀 Starting gemini-executor MCP server...");
    eprintln!("💻 Local agy CLI: {cli_status}");

    // 2. Define the tool schema exposed to Claude Code
    let execute_tool = Tool {
        name: "execute_task".to_string(),
        description: Some("Offloads heavy code generation or file writing tasks to agy CLI to save context tokens.".to_string()),
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

    // 3. Build and run the server using standard input/output (stdio)
    let execute_tool_for_list = execute_tool.clone();
    let server = Server::builder(ServerStdioTransport)
        .name("gemini-executor")
        .version("1.0.1")
        .capabilities(ServerCapabilities {
            tools: Some(json!({
                "listChanged": false
            })),
            ..Default::default()
        })
        // Manual handler for tools/list to be tolerant of null/empty params
        .request_handler("tools/list", move |_req: serde_json::Value| {
            let tool = execute_tool_for_list.clone();
            Box::pin(async move {
                Ok(json!({
                    "tools": [tool]
                }))
            })
        })
        // Manual handler for tools/call
        .request_handler("tools/call", |req: CallToolRequest| {
            Box::pin(handle_execute_task(req))
        })
        .build();

    server.listen().await?;

    Ok(())
}

// The core logic function that executes when Claude invokes the tool
async fn handle_execute_task(request: CallToolRequest) -> anyhow::Result<CallToolResponse> {
    // Deserialize arguments passed by Claude
    let arguments = request.arguments.unwrap_or_default();
    let args_json = serde_json::to_value(arguments)?;
    let args: ExecuteTaskArgs = serde_json::from_value(args_json)?;

    eprintln!(
        "🤖 Forwarding task for {} to local agy CLI...",
        args.target_file
    );

    // Call local agy CLI
    let prompt = format!(
        "Task: {}\nTarget File: {}\nReturn ONLY raw code/text content for the file. Do not include markdown code block backticks (```).",
        args.prompt, args.target_file
    );

    let output = Command::new("agy")
        .args(["--model", "flash", "-p", &prompt])
        .output()?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("agy CLI failed: {error_msg}"));
    }

    let content_raw = String::from_utf8_lossy(&output.stdout);
    let content = clean_gemini_response(&content_raw);

    if content.is_empty() {
        return Err(anyhow::anyhow!("agy CLI returned empty content."));
    }

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
                "Successfully offloaded to local agy CLI. Wrote contents to {}.",
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
