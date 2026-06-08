# gem-mcprst 🤖

`gem-mcprst` is a high-performance, asynchronous **Model Context Protocol (MCP)** server implemented in Rust. It serves as a bridge to the Google Gemini API, allowing MCP clients (like Claude Code) to offload heavy code generation and file-writing tasks.

## 🚀 Purpose

Modern AI clients often consume significant context tokens when generating large blocks of code. `gem-mcprst` solves this by:
1.  **Offloading Generation**: Sending the heavy lifting to Gemini.
2.  **Direct File Writing**: Automatically writing the generated content to your workspace.
3.  **Context Efficiency**: Saving your primary AI's context for higher-level reasoning.

## ✨ Features

- **Asynchronous Design**: Built on `tokio` and `async-mcp` for non-blocking I/O.
- **Direct Workspace Integration**: Writes generated code directly to target files, creating parent directories automatically.
- **Clean Output**: Automatically strips Markdown code fences from Gemini's responses.
- **Robust Error Handling**: Powered by `anyhow` for clear diagnostic messages.
- **CI/CD Ready**: Automated testing, linting, and multi-platform binary releases via GitHub Actions.

## 🛠️ Setup

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (Edition 2024 support).
- A Google Gemini API Key. Get one at [Google AI Studio](https://aistudio.google.com/).

### Installation

```bash
# Clone the repository
git clone https://github.com/bossoq/gem-mcprst.git
cd gem-mcprst

# Build the project
cargo build --release
```

### Configuration

The server invokes the local `agy` CLI. Ensure it is installed and available in your `PATH`.

#### Claude Desktop Integration
Add this to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "gemini-executor": {
      "command": "cargo",
      "args": ["run", "--quiet", "--manifest-path", "/path/to/gem-mcprst/Cargo.toml"]
    }
  }
}
```

*Note: Replace `/path/to/gem-mcprst/` with the absolute path to this repository.*

## 🔌 MCP Tooling

The server exposes the following tool to your MCP client:

### `execute_task`
- **Arguments**:
    - `prompt` (string): The description of the code or content to generate.
    - `target_file` (string): The path in your workspace where the output should be saved.
- **Example Usage**:
    > "Use `execute_task` to write a comprehensive Rust unit test for my parser in `src/parser_tests.rs`."

## 🏗️ Development

### Commands
- **Test**: `cargo test`
- **Lint**: `cargo clippy -- -D warnings`
- **Format**: `cargo fmt`

### CI/CD
This project uses GitHub Actions for:
- **CI**: Automated testing on every PR.
- **CD**: Automated version bumping and binary releases for Linux and macOS.

## 📜 License

[MIT](LICENSE) (or specify your license)
