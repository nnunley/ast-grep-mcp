# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial release of ast-grep MCP service
- Support for structural code search and replacement using ast-grep
- Five main tools: `search`, `file_search`, `replace`, `file_replace`, `list_languages`
- Parallelized file operations for improved performance
- Cursor-based pagination for handling large result sets
- Configurable file size limits and concurrency settings
- Support for 20+ programming languages
- Comprehensive documentation and examples
- Full test suite with unit and integration tests

### Features
- **search**: Search for patterns in code strings
- **file_search**: Search for patterns in files using glob patterns
- **replace**: Replace patterns in code strings
- **file_replace**: Replace patterns in files using glob patterns
- **list_languages**: List all supported programming languages
- **documentation**: Get detailed usage examples

### Technical Details
- Built on rmcp (Rust MCP SDK)
- Uses ast-grep-core for pattern matching
- Supports MCP (Model Context Protocol)
- Production-ready error handling
- Base64-encoded pagination cursors
- Configurable concurrency (default: 10 concurrent operations)
- File size limits (default: 50MB per file)
- Result limits (default: 1000 results per request)

## [0.1.0] - 2024-07-02

### Added
- Initial project setup
- Basic MCP service structure
- MIT License
- README with setup instructions
