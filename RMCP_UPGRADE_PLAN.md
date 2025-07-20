# RMCP 0.3.0 Upgrade Plan

## Overview
Upgrading from RMCP 0.1.5 to 0.3.0 involves several breaking changes. This document outlines the necessary changes and implementation plan.

## Breaking Changes Identified

### 1. ServerHandler Trait Changes

#### `list_tools` Method Signature
- **Old**: `list_tools(&self, request: PaginatedRequestParam, context: RequestContext<RoleServer>)`
- **New**: `list_tools(&self, request: Option<PaginatedRequestParam>, context: RequestContext<RoleServer>)`
- **Change**: Parameter is now `Option<PaginatedRequestParam>`

#### New Methods Added
- `get_prompt()` - For retrieving specific prompts
- `list_prompts()` - For listing available prompts
- `complete()` - For completion support
- `set_level()` - For logging level control
- `list_resources()` - For resource listing
- `list_resource_templates()` - For template listing
- `read_resource()` - For reading resources
- `subscribe()` / `unsubscribe()` - For subscriptions
- Various notification handlers

### 2. Tool Struct Changes

#### New Required Field
- **Field**: `annotations: Option<ToolAnnotations>`
- **Purpose**: Additional metadata for tools
- **Default**: Can be `None` for basic tools

### 3. Import Changes
- Some types may have moved between modules
- Need to verify all imports still work

## Implementation Plan

### Phase 1: Fix Compilation Errors (Immediate)

1. **Update `list_tools` signature**:
   ```rust
   fn list_tools(
       &self,
       request: Option<PaginatedRequestParam>,  // Changed from PaginatedRequestParam
       context: RequestContext<RoleServer>,
   ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_
   ```

2. **Add `annotations` field to all Tool creations**:
   ```rust
   Tool {
       name: Cow::Borrowed("search"),
       description: Some(Cow::Borrowed("...")),
       input_schema: Arc::new(...),
       annotations: None,  // Add this line
   }
   ```

3. **Handle Optional pagination**:
   ```rust
   // In list_tools implementation
   let _request = request.unwrap_or_default();
   ```

### Phase 2: Implement New Optional Methods (Optional)

For now, we can use default implementations for:
- `get_prompt()` / `list_prompts()` - Return empty results
- `complete()` - Return not implemented error
- `list_resources()` / `read_resource()` - Return empty results
- Other methods - Use defaults

### Phase 3: Add Prompts Support (Future)

1. **Convert documentation to prompts**:
   ```rust
   fn list_prompts(
       &self,
       request: Option<PaginatedRequestParam>,
       context: RequestContext<RoleServer>,
   ) -> impl Future<Output = Result<ListPromptsResult, ErrorData>> + Send + '_ {
       async move {
           Ok(ListPromptsResult {
               prompts: vec![
                   Prompt {
                       name: "ast-grep-patterns".into(),
                       description: Some("Common ast-grep pattern examples".into()),
                       arguments: vec![],
                   }
               ],
               next_cursor: None,
           })
       }
   }
   ```

## Pagination Handling

RMCP's pagination is cursor-based:

1. **Request**: Client sends optional cursor
   ```json
   {
     "cursor": "eyJvZmZzZXQiOjIwfQ=="  // Optional, base64-encoded state
   }
   ```

2. **Response**: Server returns results + optional next cursor
   ```json
   {
     "items": [...],
     "next_cursor": "eyJvZmZzZXQiOjQwfQ=="  // If more results available
   }
   ```

3. **Implementation Pattern**:
   ```rust
   fn list_tools(&self, request: Option<PaginatedRequestParam>, ...) {
       let cursor = request.and_then(|r| r.cursor);
       let offset = decode_cursor(cursor).unwrap_or(0);

       let all_tools = self.get_all_tools();
       let page = &all_tools[offset..offset + PAGE_SIZE];

       let next_cursor = if offset + PAGE_SIZE < all_tools.len() {
           Some(encode_cursor(offset + PAGE_SIZE))
       } else {
           None
       };

       Ok(ListToolsResult {
           tools: page.to_vec(),
           next_cursor,
       })
   }
   ```

## Testing Strategy

1. **Compile and fix errors** - Get basic compilation working
2. **Test existing functionality** - Ensure all current features still work
3. **Test pagination** - Verify cursor-based pagination works correctly
4. **Add prompt tests** - Once prompts are implemented

## Migration Checklist

- [ ] Update Cargo.toml dependencies
- [ ] Fix `list_tools` method signature
- [ ] Add `annotations: None` to all Tool structs
- [ ] Handle optional pagination parameter
- [ ] Run all tests
- [ ] Update documentation
- [ ] Consider implementing prompts
- [ ] Test with Claude Desktop or other MCP clients

## Benefits of Upgrade

1. **Prompts Support** - Better way to provide context to LLMs
2. **Resource Management** - New resource listing/reading capabilities
3. **Better Pagination** - Standardized cursor-based pagination
4. **Completion Support** - Auto-completion for better UX
5. **Future Compatibility** - Stay current with MCP spec

## Notes on Large Result Pagination

For the ast-grep-mcp service, pagination is especially important for:
- `file_search` - Can return thousands of matches
- `file_replace` - Can affect many files
- `list_rules` - Projects may have hundreds of rules

The cursor-based approach allows stateless pagination where the cursor encodes the position/state, making it suitable for large result sets without server-side state management.
