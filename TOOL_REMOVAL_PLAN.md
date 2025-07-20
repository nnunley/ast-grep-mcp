# Tool Removal Plan: Focus on Core Mission

## Core Mission
The ast-grep MCP service should focus on **structural code search and transformation** using ast-grep patterns. This includes basic pattern operations and rule management.

## Current Tools Analysis

### 🟢 **KEEP** - Core Tools (6 tools)

1. **`search`** - Search patterns in code strings ✅
   - Core functionality
   - Essential for pattern testing

2. **`file_search`** - Search patterns in files ✅
   - Core functionality for real-world usage
   - Includes pagination for large results

3. **`replace`** - Replace patterns in code strings ✅
   - Core transformation functionality
   - Essential for testing replacements

4. **`file_replace`** - Replace patterns in files ✅
   - Core transformation for real-world usage
   - Includes dry-run safety feature

5. **`generate_ast`** - Generate AST for code ✅
   - Essential for understanding Tree-sitter nodes
   - Helps users write better patterns

6. **`list_languages`** - List supported languages ✅
   - Basic utility function
   - Helps users know what's available

### 🟡 **KEEP** - Rule Management (7 tools)

7. **`rule_search`** - Search using rules
8. **`rule_replace`** - Replace using rules
9. **`create_rule`** - Create new rule
10. **`list_rules`** - List available rules
11. **`get_rule`** - Get specific rule
12. **`delete_rule`** - Delete rule
13. **`validate_rule`** - Validate rule syntax

### 🔴 **REMOVE** - Beyond Core Mission (7 tools)

14. **`search_embedded`** ❌
    - Overly complex for marginal benefit
    - Language injection is partially broken
    - Adds significant complexity

15. **`suggest_patterns`** ❌
    - AI/ML-like feature that overreaches
    - LLMs can already suggest patterns
    - Adds complexity without clear value

16. **`debug_pattern`** ❌
    - Debugging tool that's nice but not essential
    - Users can test patterns with search

17. **`debug_ast`** ❌
    - Similar to generate_ast but more complex
    - generate_ast already provides AST info

18. **`documentation`** ❌
    - Meta-tool that returns usage docs
    - Better handled by README and tool descriptions

19. **`list_catalog_rules`** ❌
    - External rule catalog is beyond core scope
    - Adds external dependencies

20. **`import_catalog_rule`** ❌
    - Related to catalog, beyond core scope
    - Users can manually add rules

## Implementation Plan

### Phase 1: Update Tool Lists

1. Remove tools from `list_tools()` in `ast_grep_service.rs`
2. Remove tool handling from `tool_router.rs`
3. Remove tool definitions from `tools.rs`

### Phase 2: Remove Implementations

1. **Remove modules**:
   - `src/embedded.rs` - Entire embedded search functionality
   - `src/debug.rs` - Debug functionality
   - Parts of `src/ast_grep_service.rs` - suggest_patterns, documentation

2. **Remove dependencies**:
   - Pattern suggestion logic
   - Debug formatting logic
   - Embedded language detection

3. **Update tests**:
   - Remove tests for deleted functionality
   - Update integration tests

### Phase 3: Simplify Codebase

1. **Remove from main.rs**:
   - CLI commands for removed tools
   - Associated command-line arguments

2. **Update documentation**:
   - Remove references to deleted tools
   - Update README examples
   - Simplify getting started guide

## Benefits of Removal

1. **Smaller Codebase** - Easier to maintain and understand
2. **Focused Purpose** - Clear mission: ast-grep patterns and rules
3. **Fewer Dependencies** - Reduced complexity
4. **Better Reliability** - Fewer edge cases and bugs
5. **Easier Upgrade** - Less code to migrate to RMCP 0.3.0

## Final Tool Count

- **Before**: 20 tools
- **After**: 13 tools (6 core + 7 rules)
- **Removed**: 7 tools (35% reduction)

## Migration for Users

For removed functionality:
- `suggest_patterns` → Use LLMs or learn patterns from docs
- `debug_pattern/debug_ast` → Use `generate_ast` + `search` to test
- `documentation` → Read README or tool descriptions
- `search_embedded` → Search files directly with proper language
- `catalog_rules` → Manually download and add rules

## Summary

This plan removes 35% of tools that go beyond the core mission of structural search and replace. The remaining tools form a focused, maintainable toolkit that does one thing well: **ast-grep pattern matching and transformation with rule support**.
