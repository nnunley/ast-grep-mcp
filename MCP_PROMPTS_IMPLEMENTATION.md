# MCP Prompts Implementation

## Overview

The AST-grep MCP service now implements the Model Context Protocol's native prompts API, providing intelligent, context-aware prompts for AST pattern matching assistance.

## Implementation Details

### 1. MCP Prompts API

The service implements two required MCP prompt methods in `ServerHandler`:

- **`list_prompts()`**: Returns available prompts with their metadata
- **`get_prompt()`**: Generates dynamic prompt content based on arguments

### 2. Available Prompts

#### `pattern_help`
- **Purpose**: Get help creating AST patterns for specific use cases
- **Arguments**:
  - `use_case` (required): What you want to achieve
  - `language` (required): Programming language
  - `complexity` (optional): beginner, intermediate, or advanced
- **Example**: "find all console.log statements" → suggests `console.log($MESSAGE)`

#### `pattern_debug`
- **Purpose**: Debug why a pattern isn't matching
- **Arguments**:
  - `pattern` (required): The AST pattern
  - `test_code` (required): Code to test against
  - `language` (required): Programming language
- **Uses**: Validation engine to provide real-time analysis

#### `pattern_optimize`
- **Purpose**: Get optimization suggestions for patterns
- **Arguments**:
  - `pattern` (required): The pattern to optimize
  - `goal` (optional): performance, readability, or flexibility
- **Provides**: Specific optimization strategies and improved patterns

### 3. Integration with Learning System

The prompts leverage the existing learning system:

- **ValidationEngine**: Provides pattern analysis and validation
- **DiscoveryService**: Offers curated pattern examples
- **PromptGenerator**: Creates educational content (tool-based approach preserved for internal use)

### 4. Key Features

- **Dynamic Content**: Prompts generate content based on user inputs
- **Language-Aware**: Provides language-specific examples and patterns
- **Educational Focus**: Includes learning tips and next steps
- **Real-Time Validation**: Uses the validation engine for live pattern testing

## Usage Examples

### In Claude Desktop or MCP-compatible clients:

1. **Getting Pattern Help**:
   ```
   Use prompt: pattern_help
   Arguments:
   - use_case: "find all function declarations"
   - language: "javascript"
   ```

2. **Debugging Patterns**:
   ```
   Use prompt: pattern_debug
   Arguments:
   - pattern: "function $NAME() { $$$ }"
   - test_code: "const fn = () => {}"
   - language: "javascript"
   ```

3. **Optimizing Patterns**:
   ```
   Use prompt: pattern_optimize
   Arguments:
   - pattern: "console.log($VAR)"
   - goal: "flexibility"
   ```

## Technical Notes

- Prompts are announced in server capabilities: `prompts: Some(PromptsCapability { list_changed: Some(true) })`
- Each prompt returns `GetPromptResult` with messages containing role and content
- Content is delivered as `PromptMessageContent::Text` with formatted markdown
- Validation runs synchronously using `tokio::task::block_in_place` for immediate feedback

## Benefits

1. **Native MCP Integration**: Works seamlessly with Claude Desktop and other MCP clients
2. **Interactive Learning**: Provides immediate, context-aware assistance
3. **No Tool Calls Needed**: Prompts are exposed as first-class MCP features
4. **Rich Educational Content**: Leverages the learning system's pattern knowledge

## Future Enhancements

- Add more prompt types (e.g., `pattern_convert` for cross-language patterns)
- Integrate with more learning system features
- Support prompt chaining for complex workflows
- Add user preference tracking for personalized suggestions
