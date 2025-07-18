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

### Project Configuration (sgconfig.yml)

The service supports ast-grep's `sgconfig.yml` configuration files. When you start the service, it will:

1. Search for `sgconfig.yml` in the current directory and parent directories
2. Load rule directories specified in the configuration
3. Make all rules from configured directories available for use

Example `sgconfig.yml`:
```yaml
ruleDirs:
  - ./rules
  - ./team-rules
  - ./node_modules/@company/ast-grep-rules
```

You can also specify a custom config path using the `--config` flag when starting the service.

**Note on Duplicate Rule IDs**: The ast-grep documentation states that rule IDs should be unique. When multiple rules have the same ID across different directories, this service:
- Uses only the first rule encountered
- Emits a warning to stderr showing both the duplicate and original file paths
- Ignores subsequent rules with the same ID

While testing shows the ast-grep CLI currently applies all rules with duplicate IDs, this behavior is undocumented and our approach ensures predictable rule application.

### ⚠️ Important Usage Notes

- **Manual Syntax Responsibility**: You are responsible for ensuring replacement patterns produce valid syntax
- **Test Before Apply**: Always use `dry_run: true` first to preview changes
- **Comma Placement**: Include commas explicitly in patterns - they are not automatically inserted
- **Struct Update Syntax**: Fields must come before `..Default::default()` in Rust struct literals

### Other MCP Clients

Configure your client to use `ast-grep-mcp` as a stdio-based MCP server.

## 🛠️ Available Tools

### 🎯 Automatic Language Injection Support

The service automatically detects and searches embedded languages, mimicking ast-grep CLI behavior:

- **JavaScript in HTML**: Search for JS patterns in `<script>` tags automatically
- **CSS in HTML**: Search for CSS patterns in `<style>` tags automatically
- **No configuration needed**: Just specify the pattern language, not the file language

Example:
```json
{
  "pattern": "console.log($MSG)",
  "language": "javascript",  // Pattern language
  "file": "index.html"       // Automatically detects JS in HTML
}
```

### `search`
Search for patterns in code strings (for quick checks).

### `file_search`
Search for patterns within files using glob patterns or direct file paths.
```json
{
  "path_pattern": "src/**/*.js",
  "pattern": "function $NAME($PARAMS) { $BODY }",
  "language": "javascript",
  "max_results": 20  // Optional, defaults to 20
}
```

**Supports both glob patterns and direct file paths:**
- Glob patterns: `"src/**/*.js"`, `"*.rs"`, `"**/*.py"`
- Direct file paths: `"/path/to/specific/file.js"`, `"src/main.rs"`

**Security**: Direct file paths must be under configured root directories.

**Pagination**: For large result sets, use the cursor for pagination:
```json
// First request
{
  "path_pattern": "**/*.js",
  "pattern": "console.log($VAR)",
  "language": "javascript"
}

// Response includes cursor for next page
// "next_cursor": { "cursor": "H4sIAAAAAAAAA...", "is_complete": false }

// Next request with cursor
{
  "path_pattern": "**/*.js",
  "pattern": "console.log($VAR)",
  "language": "javascript",
  "cursor": {
    "cursor": "H4sIAAAAAAAAA..."
  }
}
```

**Large Result Optimization**: When results exceed 10 files or 50 matches, the response automatically switches to a lightweight format with essential pagination data to avoid token limits.

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
  "dry_run": true,  // Optional, defaults to true for safety
  "max_results": 20  // Optional, defaults to 20
}
```

⚠️ **Important**: ast-grep performs **literal pattern matching and replacement**. It does not:
- Automatically insert commas between fields
- Infer proper placement of struct update syntax (`..Default::default()`)
- Handle syntax validation

You must ensure your replacement patterns produce valid syntax.

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

**Pagination**: Similar to `file_search`, supports cursor-based pagination for large refactoring operations. Uses the same opaque, compressed cursor format.

### `list_languages`
Get all supported programming languages.

### `generate_ast`
🔍 **Essential for LLM users**: Generate syntax trees and discover Tree-sitter node kinds
```json
{
  "code": "function test() { return 42; }",
  "language": "javascript"
}
```
Returns AST structure and available node kinds like `function_declaration`, `identifier`, `statement_block` for use in Kind rules.

### `documentation`
Comprehensive usage examples and best practices.

## 📖 Pattern Examples

### ⚠️ Important: Manual Comma Handling

**ast-grep does NOT automatically insert commas.** You must include commas explicitly in your patterns:

```rust
// ❌ WRONG - Missing comma in replacement
"Point { x: $X, y: $Y }" → "Point { x: $X, y: $Y z: 0 }"  // Invalid syntax

// ✅ CORRECT - Comma included in replacement
"Point { x: $X, y: $Y }" → "Point { x: $X, y: $Y, z: 0 }"  // Valid syntax
```

**Field ordering matters in struct updates:**
```rust
// ❌ WRONG - Fields after ..Default::default()
Config { field: value, ..Default::default(), new_field: None }  // Invalid

// ✅ CORRECT - Fields before ..Default::default()
Config { field: value, new_field: None, ..Default::default() }  // Valid
```

### AST Pattern Language
```javascript
// Metavariables capture single nodes
"console.log($VAR)"

// Multiple statements/expressions
"function $NAME($ARGS) { $$$BODY }"

// Exact text matching
"const API_KEY = 'secret'"
```

### Tree-sitter Node Kinds
Use `generate_ast` to discover available node kinds for any language:
```yaml
# Kind rules match specific AST node types
rule:
  kind: function_declaration  # JavaScript/TypeScript
  # OR
  kind: fn_item             # Rust
  # OR
  kind: function_definition  # Python
```

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

### 🎯 Best Practices for Reliable Patterns

1. **Test patterns with simple examples first**
2. **Use Context Lines** to understand insertion points:
   ```json
   {
     "pattern": "enabled: $VAL",
     "context_before": 2,
     "context_after": 2
   }
   ```
3. **Always include necessary commas** in replacement patterns
4. **Match specific locations** rather than broad patterns when adding struct fields
5. **Verify syntax** by ensuring `..Default::default()` remains last in struct literals

## 🏃 Usage

### Run as MCP Server
```bash
ast-grep-mcp
```

### With Custom Root Directories
```bash
# Search in specific directories
ast-grep-mcp --root-dir /path/to/project1 --root-dir /path/to/project2

# Short form
ast-grep-mcp -d /path/to/project1 -d /path/to/project2
```

### With Debug Logging
```bash
RUST_LOG=debug ast-grep-mcp
```

### Full Command Line Options
```bash
ast-grep-mcp --help
# Shows all available options:
# -d, --root-dir <ROOT_DIRECTORIES>  Root directories to search in
# --max-file-size <MAX_FILE_SIZE>    Maximum file size in bytes [default: 52428800]
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

## 🗺️ Practical Development Roadmap

Based on real-world usage patterns and user feedback, we're focusing on simple, high-value features that complement LLM capabilities rather than duplicate them.

### 🎯 Phase 1: Core Excellence (v0.1.1)

**Immediate Priority - Fix and Polish**

- [ ] **Fix language injection** - Complete the automatic language detection for HTML/JS/CSS
- [ ] **Performance optimization** - Improve handling of large codebases (>10k files)
- [ ] **Better error messages** - Clear, actionable error messages with suggestions
- [ ] **Edge case handling** - Robustness improvements for malformed code

### 🚀 Phase 2: High-Value Tools (v0.2.0)

**Simple features that fill real gaps**

- [ ] **`find_usage`** - Track where symbols are used across codebases
  ```json
  {
    "symbol": "functionName",
    "type": "imports|calls|definitions"
  }
  ```

- [ ] **`validate_pattern`** - Test patterns against sample code
  ```json
  {
    "pattern": "console.log($MSG)",
    "test_code": "console.log('hello');",
    "language": "javascript"
  }
  ```

- [ ] **`preview_changes`** - Simple list of files that would be affected
  ```json
  {
    "pattern": "var $X = $Y",
    "replacement": "const $X = $Y",
    "path_pattern": "**/*.js"
  }
  ```

### ✨ Phase 3: Polish (v0.3.0)

**Nice-to-have improvements**

- [ ] **Enhanced language injection** - Support for more embedded languages
- [ ] **Performance profiling** - Help users optimize their patterns
- [ ] **Batch operation improvements** - Better progress reporting for large operations

### 🔍 Implementation Status

**Current Version: v0.1.0**
- ✅ Core ast-grep pattern matching
- ✅ Rule-based search and replace
- ✅ `suggest_patterns` - Already implemented!
- ✅ Tree-sitter node kind discovery
- ✅ Token-efficient diff output
- ✅ MCP service integration
- ⚠️ Partial language injection (2/5 tests passing)

**Focus**: Making the core tool excellent rather than adding complex "smart" features

### 💡 Vision: Simple Tools, Maximum Value

Keep ast-grep MCP focused on what it does best:
- **Fast pattern matching** at scale
- **Reliable transformations** with safety features
- **Simple tools** that complement LLM capabilities
- **Predictable behavior** that LLMs can rely on
- **Efficient operations** on large codebases

### 🤝 Contributing to the Roadmap

We welcome contributions that align with our focused approach:

1. **High Priority Items**: Performance optimization, dependency tracking, pattern validation
2. **Good First Issues**: Fix language injection tests, documentation improvements, test coverage
3. **Core Improvements**: Error handling, edge cases, batch operation efficiency

**Note**: See [ROADMAP_ANALYSIS.md](ROADMAP_ANALYSIS.md) for detailed analysis of which features provide real value versus complexity.

## 📋 Technical Architecture & Implementation Notes

### Duplicate Rule ID Handling

The ast-grep documentation states that rule IDs should be unique, but the CLI behavior with duplicates is undocumented. Our implementation follows a "first wins" strategy:

- Only the first rule with a given ID is kept
- Subsequent rules with the same ID are ignored and a warning is emitted
- This ensures predictable rule application, unlike the undocumented CLI behavior

### Known Issues & Future Improvements

#### Language Injection Enhancement (TODO)
The current automatic language injection (JavaScript in HTML, CSS in HTML) has limitations:

**Current Status:**
- ✅ Basic HTML/JS/CSS injection working
- ✅ Detection based on file extension + pattern language
- ✅ Integration with search methods

**Remaining Work:**
- Fix extraction patterns for script tags with attributes
- Improve CSS extraction for styled-components and Vue scoped styles
- Add more language combinations (Python/SQL, JS/GraphQL, Markdown code blocks)
- Handle edge cases and malformed HTML
- Performance optimization with caching
- Configuration support via sgconfig.yml
- Better error handling and fallback mechanisms

#### Performance Optimization Opportunities
- Cache extraction results for repeated searches
- Improve rule evaluation performance for large files
- Optimize AST parsing for frequently-used patterns
- Implement parallel processing for bulk operations

#### Error Handling Improvements
- More comprehensive error recovery mechanisms
- Better error messages for pattern matching failures
- Graceful degradation when Tree-sitter parsing fails
- Validation of user-provided patterns before execution

## 📝 Changelog

### [Unreleased]

**Added:**
- Initial release of ast-grep MCP service
- Support for structural code search and replacement
- Five main tools: `search`, `file_search`, `replace`, `file_replace`, `list_languages`
- Parallelized file operations for improved performance
- Cursor-based pagination for handling large result sets
- Configurable file size limits and concurrency settings
- Support for 20+ programming languages
- Comprehensive documentation and examples
- Full test suite with unit and integration tests
- Rule management system with YAML configuration
- Automatic language injection for embedded code
- Debug tools for pattern testing and AST exploration

**Technical Details:**
- Built on rmcp (Rust MCP SDK)
- Uses ast-grep-core for pattern matching
- Supports MCP (Model Context Protocol)
- Production-ready error handling
- Base64-encoded pagination cursors
- Configurable concurrency (default: 10 concurrent operations)
- File size limits (default: 50MB per file)
- Result limits (default: 1000 results per request)

### [0.1.0] - 2024-07-02

**Added:**
- Initial project setup
- Basic MCP service structure
- MIT License
- README with setup instructions

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
