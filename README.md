# ast-grep MCP Service

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![MCP Compatible](https://img.shields.io/badge/MCP-Compatible-blue.svg)](https://modelcontextprotocol.io)
[![Rust](https://img.shields.io/badge/rust-2024%2B-brightgreen.svg)](https://www.rust-lang.org)

A Model Context Protocol (MCP) service that provides ast-grep functionality for structural code search and transformation. This service enables AI assistants to perform sophisticated code analysis and refactoring with token-efficient diff-based responses.

## ✨ Key Features

- **🔍 Structural Search & Replace** - Use ast-grep's powerful AST-based pattern matching
- **📁 Multi-Root Directory Support** - Search across multiple directory trees
- **⚡ Token-Efficient Diffs** - Returns line-by-line changes instead of full file content
- **🛡️ Safe by Default** - Dry-run mode with optional in-place file modification
- **🌍 Multi-Language Support** - JavaScript, TypeScript, Rust, Python, Java, Go, and more
- **📊 Comprehensive Documentation** - Built-in examples and best practices via `/documentation` tool

## 🚀 Installation

### Option 1: Install from GitHub (Recommended)
```bash
cargo install --git https://github.com/nnunley/ast-grep-mcp
```

### Option 2: Build from Source
```bash
git clone https://github.com/nnunley/ast-grep-mcp
cd ast-grep-mcp
cargo install --path .
```

## 🔧 Configuration

### Claude Desktop

Add to your Claude Desktop configuration file:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "ast-grep": {
      "command": "ast-grep-mcp"
    }
  }
}
```

### Other MCP Clients

Configure your client to use `ast-grep-mcp` as a stdio-based MCP server.

## 🛠️ Available Tools

### `search`
Search for patterns in code strings (for quick checks).

### `file_search`
Search for patterns within files using glob patterns.
```json
{
  "path_pattern": "src/**/*.js",
  "pattern": "function $NAME($PARAMS) { $BODY }",
  "language": "javascript"
}
```

### `replace`
Replace patterns in code strings (for in-memory transformations).

### `file_replace`
🌟 **Token-efficient file replacement with diff output**
```json
{
  "path_pattern": "src/**/*.js",
  "pattern": "const $VAR = $VAL",
  "replacement": "let $VAR = $VAL",
  "language": "javascript",
  "dry_run": true
}
```

**Returns compact diffs:**
```json
{
  "file_results": [{
    "file_path": "src/main.js",
    "changes": [
      {
        "line": 15,
        "old_text": "const x = 5;",
        "new_text": "let x = 5;"
      }
    ],
    "total_changes": 1
  }],
  "dry_run": true
}
```

### `list_languages`
Get all supported programming languages.

### `documentation`
Comprehensive usage examples and best practices.

## 📖 Pattern Examples

### JavaScript/TypeScript
```javascript
// Find function declarations
"function $NAME($PARAMS) { $BODY }"

// Find console.log calls
"console.log($VAR)"

// Find variable assignments
"const $VAR = $VALUE"
```

### Rust
```rust
// Find function definitions
"fn $NAME($PARAMS) -> $RETURN_TYPE { $BODY }"

// Find println! macros
"println!($VAR)"

// Find match expressions
"match $EXPR { $ARMS }"
```

### Python
```python
// Find class definitions
"class $NAME($BASE): $BODY"

// Find function definitions
"def $NAME($PARAMS): $BODY"
```

## 🔄 Workflow: Preview → Apply

1. **Preview changes** (safe, default):
```json
{
  "tool_code": "file_replace",
  "tool_params": {
    "pattern": "var $VAR = $VAL",
    "replacement": "const $VAR = $VAL",
    "dry_run": true
  }
}
```

2. **Apply changes** (when ready):
```json
{
  "dry_run": false
}
```

## 🏃 Usage

### Run as MCP Server
```bash
ast-grep-mcp
```

### With Debug Logging
```bash
RUST_LOG=debug ast-grep-mcp
```

### Test Installation
```bash
# Should show help and available tools
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ast-grep-mcp
```

## 🧪 Development

### Run Tests
```bash
cargo test
```

### Run with Logging
```bash
RUST_LOG=debug cargo run
```

### Lint & Format
```bash
cargo clippy
cargo fmt
```

## 🌟 Supported Languages

**Web**: JavaScript, TypeScript, TSX, HTML, CSS
**Systems**: Rust, C, C++, Go
**Enterprise**: Java, C#, Kotlin, Scala
**Scripting**: Python, Ruby, Lua, Bash
**Others**: Swift, Dart, Elixir, Haskell, PHP, YAML, JSON

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass: `cargo test`
5. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🔗 Related Projects

- [ast-grep](https://github.com/ast-grep/ast-grep) - The core ast-grep library
- [Model Context Protocol](https://modelcontextprotocol.io) - The protocol specification
- [Claude Code](https://claude.ai/code) - AI assistant with MCP support

---

**Perfect for**: Code refactoring, pattern analysis, bulk transformations, and AI-assisted development workflows.
