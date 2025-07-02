# ast-grep MCP Service

[![Crates.io](https://img.shields.io/crates/v/ast-grep-mcp.svg)](https://crates.io/crates/ast-grep-mcp)
[![Documentation](https://docs.rs/ast-grep-mcp/badge.svg)](https://docs.rs/ast-grep-mcp)
[![Build Status](https://github.com/nnunley/ast-grep-mcp/workflows/CI/badge.svg)](https://github.com/nnunley/ast-grep-mcp/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![MCP Compatible](https://img.shields.io/badge/MCP-Compatible-blue.svg)](https://modelcontextprotocol.io)
[![Rust](https://img.shields.io/badge/rust-2024%2B-brightgreen.svg)](https://www.rust-lang.org)

A Model Context Protocol (MCP) service that provides ast-grep functionality for structural code search and transformation.

## Overview

This MCP service exposes ast-grep's powerful structural search and replace capabilities through the Model Context Protocol, allowing AI assistants to perform sophisticated code analysis and transformations.

## Features

- **Structural Search**: Find code patterns using abstract syntax tree matching
- **Language Support**: Works with multiple programming languages supported by ast-grep
- **File Operations**: Search across files with glob pattern support
- **Pattern Matching**: Use ast-grep's powerful pattern matching syntax
- **Code Replacement**: Perform structural code replacements

## Installation

### Prerequisites

- Rust (latest stable)
- Cargo

### Building from Source

```bash
git clone <repository-url>
cd ast-grep-mcp
cargo build --release
```

## Usage

### Adding to Claude Desktop

Add the following configuration to your Claude Desktop config file:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`  
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "ast-grep": {
      "command": "/path/to/ast-grep-mcp/target/release/ast-grep-mcp"
    }
  }
}
```

### Adding to Other MCP Clients

For other MCP clients, configure them to use the `ast-grep-mcp` executable as a stdio-based MCP server.

## Tools

The service provides the following tools:

### `search_pattern`
Search for code patterns in files using ast-grep syntax.

**Parameters:**
- `pattern` (required): The ast-grep pattern to search for
- `language` (required): Programming language (e.g., "rust", "javascript", "python")
- `paths` (optional): Array of file paths or glob patterns to search
- `max_results` (optional): Maximum number of results to return (default: 100)

### `replace_pattern`
Replace code patterns using ast-grep's structural replacement.

**Parameters:**
- `pattern` (required): The ast-grep pattern to search for
- `replacement` (required): The replacement pattern
- `language` (required): Programming language
- `paths` (optional): Array of file paths or glob patterns
- `max_results` (optional): Maximum number of results to return (default: 100)

## Examples

### Search for Function Calls

```
Use the search_pattern tool to find all calls to the "println!" macro in Rust files:
- pattern: "println!($$$)"
- language: "rust"
- paths: ["**/*.rs"]
```

### Replace Variable Names

```
Use the replace_pattern tool to rename a variable:
- pattern: "let $VAR = $VALUE;"
- replacement: "let new_name = $VALUE;"
- language: "rust"
- paths: ["src/main.rs"]
```

## Development

### Running from Source

```bash
cargo run
```

### Logging

The service uses tracing for logging. Set the `RUST_LOG` environment variable to control log levels:

```bash
RUST_LOG=debug cargo run
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Related Projects

- [ast-grep](https://github.com/ast-grep/ast-grep) - The core ast-grep library
- [Model Context Protocol](https://modelcontextprotocol.io) - The protocol specification