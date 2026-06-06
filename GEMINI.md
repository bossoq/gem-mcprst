# Project Overview: gem-mcprst

`gem-mcprst` is a Model Context Protocol (MCP) server implemented in Rust. Its primary purpose is to provide a bridge between an MCP client (like Claude Code) and the Google Gemini API, specifically for offloading heavy code generation or file-writing tasks to save context tokens in the main client session.

### Core Technologies
- **Rust**: Language of choice, using the 2024 edition.
- **async-mcp**: Standard Rust SDK (v0.1.3) for building MCP servers.
- **Anyhow**: For flexible and idiomatic error handling.
- **Tokio**: Async runtime for handling I/O and server lifecycle.
- **Reqwest**: For making HTTP requests to the Gemini API.
- **Serde**: For JSON serialization and deserialization.

## Architecture & Logic

The server operates over standard input/output (stdio), following the MCP specification. It exposes a single tool:

### Tool: `execute_task`
- **Description**: Offloads tasks to Gemini.
- **Arguments**:
  - `prompt`: Detailed description of the code or content to be generated.
  - `target_file`: The workspace path where the generated output should be written.
- **Workflow**:
  1. Receives the task from the MCP client.
  2. Forwards the prompt to Gemini (using the `GEMINI_API_KEY` environment variable).
  3. Cleans up the response (removes Markdown code blocks).
  4. Automatically creates any missing parent directories for the `target_file`.
  5. Writes the generated content directly to the disk.
  6. Returns a success confirmation to the client.

## Building and Running

### Prerequisites
- Rust toolchain (Edition 2024 support required).
- A valid Google Gemini API Key.

### Environment Setup
The server requires the following environment variable to be set:
```bash
export GEMINI_API_KEY="your_api_key_here"
```

### Commands
- **Build**: `cargo build`
- **Check**: `cargo check`
- **Lint**: `cargo clippy -- -D warnings`
- **Run**: `cargo run` (The server will start and wait for MCP messages on stdin).
- **Test**: `cargo test`

## Development Conventions

- **API Usage**: Use `Server::builder(ServerStdioTransport)` for initialization.
- **Handlers**: Tool handlers must return `anyhow::Result<CallToolResponse>` and be registered using `Box::pin` if using the low-level `register_tool` API.
- **Surgical Updates**: When modifying `src/main.rs`, maintain the existing structure of tool definitions and handlers.
- **Error Handling**: Leverages `anyhow` for robust error propagation back to the MCP client.
- **Formatting**: Adhere to standard Rust formatting. Run `cargo fmt` before committing.
- **API Version**: Currently uses `gemini-2.5-flash` model. Update the URL in `src/main.rs` if a different model or version is required.
