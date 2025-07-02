use std::{borrow::Cow, collections::HashMap, fmt, io, path::PathBuf, str::FromStr, sync::Arc};

use ast_grep_core::{AstGrep, Pattern};
use ast_grep_language::SupportLang as Language;
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use futures::stream::{self, StreamExt};
use globset::{Glob, GlobSetBuilder};
use rmcp::{
    ServerHandler,
    model::{
        CallToolRequestParam, CallToolResult, Content, ErrorData, Implementation, InitializeResult,
        ListToolsResult, PaginatedRequestParam, ProtocolVersion, ServerCapabilities, Tool,
    },
    service::{RequestContext, RoleServer},
};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Maximum file size to process (in bytes)
    pub max_file_size: u64,
    /// Maximum number of concurrent file operations
    pub max_concurrency: usize,
    /// Maximum number of results to return per search
    pub max_results: usize,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            max_file_size: 50 * 1024 * 1024, // 50MB
            max_concurrency: 10,
            max_results: 1000,
        }
    }
}

#[derive(Clone)]
pub struct AstGrepService {
    config: ServiceConfig,
}

#[derive(Debug)]
pub enum ServiceError {
    Io(io::Error),
    SerdeJson(serde_json::Error),
    Glob(globset::Error),
    ParserError(String),
    ToolNotFound(String),
    Internal(String),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceError::Io(e) => write!(f, "IO error: {}", e),
            ServiceError::SerdeJson(e) => {
                write!(f, "JSON serialization/deserialization error: {}", e)
            }
            ServiceError::ParserError(e) => write!(f, "Parser error: {}", e),
            ServiceError::Glob(e) => write!(f, "Glob error: {}", e),
            ServiceError::ToolNotFound(tool) => write!(f, "Tool not found: {}", tool),
            ServiceError::Internal(msg) => write!(f, "Internal service error: {}", msg),
        }
    }
}

impl From<io::Error> for ServiceError {
    fn from(err: io::Error) -> Self {
        ServiceError::Io(err)
    }
}

impl From<globset::Error> for ServiceError {
    fn from(err: globset::Error) -> Self {
        ServiceError::Glob(err)
    }
}

impl From<serde_json::Error> for ServiceError {
    fn from(err: serde_json::Error) -> Self {
        ServiceError::SerdeJson(err)
    }
}

impl From<ServiceError> for ErrorData {
    fn from(err: ServiceError) -> Self {
        ErrorData::internal_error(Cow::Owned(err.to_string()), None)
    }
}

impl AstGrepService {
    pub fn new() -> Self {
        Self {
            config: ServiceConfig::default(),
        }
    }

    pub fn with_config(config: ServiceConfig) -> Self {
        Self { config }
    }

    pub async fn search(&self, param: SearchParam) -> Result<SearchResult, ServiceError> {
        let lang = Language::from_str(param.language.as_str())
            .map_err(|_| ServiceError::Internal("Failed to parse language".to_string()))?;

        let ast = AstGrep::new(param.code.as_str(), lang);
        let pattern = Pattern::new(param.pattern.as_str(), lang);

        let matches: Vec<MatchResult> = ast
            .root()
            .find_all(pattern)
            .map(|node| {
                let vars: HashMap<String, String> = node.get_env().clone().into();
                MatchResult {
                    text: node.text().to_string(),
                    vars,
                }
            })
            .collect();

        Ok(SearchResult { matches })
    }

    pub async fn file_search(
        &self,
        param: FileSearchParam,
    ) -> Result<FileSearchResult, ServiceError> {
        let lang = Language::from_str(param.language.as_str())
            .map_err(|_| ServiceError::Internal("Failed to parse language".to_string()))?;

        let mut builder = GlobSetBuilder::new();
        builder.add(Glob::new(&param.path_pattern)?);
        let globset = builder.build()?;

        let max_file_size = param.max_file_size.unwrap_or(self.config.max_file_size);
        let max_results = param.max_results.unwrap_or(self.config.max_results);

        // Determine cursor position for pagination
        let cursor_path = if let Some(cursor) = &param.cursor {
            if cursor.is_complete {
                // Previous search was complete, no more results
                return Ok(FileSearchResult {
                    file_results: vec![],
                    next_cursor: Some(SearchCursor::complete()),
                    total_files_found: 0,
                });
            }
            Some(cursor.decode_path()?)
        } else {
            None
        };

        // Collect all matching file paths, sorted for consistent pagination
        let mut all_matching_files: Vec<_> = WalkDir::new(".")
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|entry| {
                let path = entry.path();
                path.is_file() && globset.is_match(path)
            })
            .filter(|entry| {
                // Check file size
                if let Ok(metadata) = entry.metadata() {
                    if metadata.len() > max_file_size {
                        tracing::warn!(
                            "Skipping large file: {:?} ({}MB)",
                            entry.path(),
                            metadata.len() / (1024 * 1024)
                        );
                        return false;
                    }
                }
                true
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();

        // Sort for consistent ordering across pagination requests
        all_matching_files.sort();
        let total_files_found = all_matching_files.len();

        // Apply cursor-based filtering
        let files_to_process: Vec<_> = if let Some(cursor_path) = cursor_path {
            all_matching_files
                .into_iter()
                .skip_while(|path| path.to_string_lossy().as_ref() <= cursor_path.as_str())
                .take(max_results * 2) // Take more files since not all will have matches
                .collect()
        } else {
            all_matching_files
                .into_iter()
                .take(max_results * 2)
                .collect()
        };

        // Process files in parallel
        let pattern_str = param.pattern.clone();
        let file_results_raw: Vec<(PathBuf, FileMatchResult)> =
            stream::iter(files_to_process.iter().cloned())
                .map(|path| {
                    let pattern_str = pattern_str.clone();
                    async move {
                        let result = self
                            .search_single_file(path.clone(), pattern_str, lang)
                            .await;
                        (path, result)
                    }
                })
                .buffer_unordered(self.config.max_concurrency)
                .filter_map(|(path, result)| async move {
                    match result {
                        Ok(Some(file_result)) => Some((path, file_result)),
                        Ok(None) => None,
                        Err(e) => {
                            tracing::warn!("Error processing file {:?}: {}", path, e);
                            None
                        }
                    }
                })
                .take(max_results)
                .collect::<Vec<_>>()
                .await;

        // Determine next cursor
        let next_cursor = if file_results_raw.len() < max_results {
            // We got fewer results than requested, so we're done
            Some(SearchCursor::complete())
        } else if let Some((last_path, _)) = file_results_raw.last() {
            // More results may be available
            Some(SearchCursor::new(&last_path.to_string_lossy()))
        } else {
            Some(SearchCursor::complete())
        };

        // Extract just the file results
        let file_results: Vec<FileMatchResult> = file_results_raw
            .into_iter()
            .map(|(_, result)| result)
            .collect();

        Ok(FileSearchResult {
            file_results,
            next_cursor,
            total_files_found,
        })
    }

    async fn search_single_file(
        &self,
        path: PathBuf,
        pattern_str: String,
        lang: Language,
    ) -> Result<Option<FileMatchResult>, ServiceError> {
        let file_content = tokio::fs::read_to_string(&path).await?;

        let ast = AstGrep::new(file_content.as_str(), lang);
        let pattern = Pattern::new(pattern_str.as_str(), lang);

        let matches: Vec<MatchResult> = ast
            .root()
            .find_all(pattern)
            .map(|node| {
                let vars: HashMap<String, String> = node.get_env().clone().into();
                MatchResult {
                    text: node.text().to_string(),
                    vars,
                }
            })
            .collect();

        if !matches.is_empty() {
            Ok(Some(FileMatchResult {
                file_path: path,
                matches,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn replace(&self, param: ReplaceParam) -> Result<ReplaceResult, ServiceError> {
        let lang = Language::from_str(param.language.as_str())
            .map_err(|_| ServiceError::Internal("Failed to parse language".to_string()))?;

        let mut ast = AstGrep::new(param.code.as_str(), lang);
        let pattern = Pattern::new(param.pattern.as_str(), lang);
        let replacement = param.replacement.as_str();

        ast.replace(pattern, replacement)
            .map_err(|_| ServiceError::Internal("Failed to apply replacement".to_string()))?;
        let rewritten_code = ast.root().text().to_string();

        Ok(ReplaceResult { rewritten_code })
    }

    pub async fn file_replace(
        &self,
        param: FileReplaceParam,
    ) -> Result<FileReplaceResult, ServiceError> {
        let lang = Language::from_str(param.language.as_str())
            .map_err(|_| ServiceError::Internal("Failed to parse language".to_string()))?;

        let mut builder = GlobSetBuilder::new();
        builder.add(Glob::new(&param.path_pattern)?);
        let globset = builder.build()?;

        let max_file_size = param.max_file_size.unwrap_or(self.config.max_file_size);
        let max_results = param.max_results.unwrap_or(self.config.max_results);

        // Determine cursor position for pagination
        let cursor_path = if let Some(cursor) = &param.cursor {
            if cursor.is_complete {
                return Ok(FileReplaceResult {
                    file_results: vec![],
                    next_cursor: Some(SearchCursor::complete()),
                    total_files_found: 0,
                });
            }
            Some(cursor.decode_path()?)
        } else {
            None
        };

        // Collect all matching file paths, sorted for consistent pagination
        let mut all_matching_files: Vec<_> = WalkDir::new(".")
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|entry| {
                let path = entry.path();
                path.is_file() && globset.is_match(path)
            })
            .filter(|entry| {
                // Check file size
                if let Ok(metadata) = entry.metadata() {
                    if metadata.len() > max_file_size {
                        tracing::warn!(
                            "Skipping large file: {:?} ({}MB)",
                            entry.path(),
                            metadata.len() / (1024 * 1024)
                        );
                        return false;
                    }
                }
                true
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();

        all_matching_files.sort();
        let total_files_found = all_matching_files.len();

        // Apply cursor-based filtering
        let files_to_process: Vec<_> = if let Some(cursor_path) = cursor_path {
            all_matching_files
                .into_iter()
                .skip_while(|path| path.to_string_lossy().as_ref() <= cursor_path.as_str())
                .take(max_results)
                .collect()
        } else {
            all_matching_files.into_iter().take(max_results).collect()
        };

        // Process files in parallel
        let pattern_str = param.pattern.clone();
        let replacement_str = param.replacement.clone();
        let file_results_raw: Vec<(PathBuf, FileReplaceResultItem)> =
            stream::iter(files_to_process.iter().cloned())
                .map(|path| {
                    let pattern_str = pattern_str.clone();
                    let replacement_str = replacement_str.clone();
                    async move {
                        let result = self
                            .replace_single_file(path.clone(), pattern_str, replacement_str, lang)
                            .await;
                        (path, result)
                    }
                })
                .buffer_unordered(self.config.max_concurrency)
                .filter_map(|(path, result)| async move {
                    match result {
                        Ok(Some(file_result)) => Some((path, file_result)),
                        Ok(None) => None,
                        Err(e) => {
                            tracing::warn!("Error processing file {:?}: {}", path, e);
                            None
                        }
                    }
                })
                .collect::<Vec<_>>()
                .await;

        // Determine next cursor
        let next_cursor = if files_to_process.len() < max_results {
            Some(SearchCursor::complete())
        } else if let Some((last_path, _)) = file_results_raw.last() {
            Some(SearchCursor::new(&last_path.to_string_lossy()))
        } else if let Some(last_processed) = files_to_process.last() {
            Some(SearchCursor::new(&last_processed.to_string_lossy()))
        } else {
            Some(SearchCursor::complete())
        };

        // Extract just the file results
        let file_results: Vec<FileReplaceResultItem> = file_results_raw
            .into_iter()
            .map(|(_, result)| result)
            .collect();

        Ok(FileReplaceResult {
            file_results,
            next_cursor,
            total_files_found,
        })
    }

    async fn replace_single_file(
        &self,
        path: PathBuf,
        pattern_str: String,
        replacement_str: String,
        lang: Language,
    ) -> Result<Option<FileReplaceResultItem>, ServiceError> {
        let file_content = tokio::fs::read_to_string(&path).await?;

        let mut ast = AstGrep::new(file_content.as_str(), lang);
        let pattern = Pattern::new(pattern_str.as_str(), lang);
        let replacement = replacement_str.as_str();

        match ast.replace(pattern, replacement) {
            Ok(_) => {
                let rewritten_file_content = ast.root().text().to_string();
                // Only return if there were actual changes
                if rewritten_file_content != file_content {
                    Ok(Some(FileReplaceResultItem {
                        file_path: path,
                        rewritten_content: rewritten_file_content,
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Err(ServiceError::Internal(
                "Failed to apply replacement".to_string(),
            )),
        }
    }

    pub async fn list_languages(
        &self,
        _param: ListLanguagesParam,
    ) -> Result<ListLanguagesResult, ServiceError> {
        // List all supported languages manually since all_languages() may not exist
        let languages = vec![
            "bash",
            "c",
            "cpp",
            "csharp",
            "css",
            "dart",
            "elixir",
            "go",
            "haskell",
            "html",
            "java",
            "javascript",
            "json",
            "kotlin",
            "lua",
            "php",
            "python",
            "ruby",
            "rust",
            "scala",
            "swift",
            "typescript",
            "tsx",
            "yaml",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect();

        Ok(ListLanguagesResult { languages })
    }

    pub async fn documentation(
        &self,
        _param: DocumentationParam,
    ) -> Result<DocumentationResult, ServiceError> {
        let docs = r##"
## search

Searches for patterns in code provided as a string. Useful for quick checks or when code snippets are generated dynamically.

**Parameters:**
- `code`: The source code string to search within.
- `pattern`: The ast-grep pattern to search for (e.g., "console.log($VAR)").
- `language`: The programming language of the code (e.g., "javascript", "typescript", "rust").

**Example Usage:**
```json
{
  "tool_code": "search",
  "tool_params": {
    "code": "function greet() { console.log(\"Hello\"); }",
    "pattern": "console.log($VAR)",
    "language": "javascript"
  }
}
```

## file_search

Searches for patterns within a specified file. Ideal for analyzing existing code files on the system.

**Parameters:**
- `path_pattern`: A glob pattern for files to search within (e.g., "src/**/*.js").
- `pattern`: The ast-grep pattern to search for.
- `language`: The programming language of the file.
- `max_results` (optional): Maximum number of results to return (default: 1000).
- `max_file_size` (optional): Maximum file size to process in bytes (default: 50MB).
- `cursor` (optional): Continuation token from previous search for pagination.

**Example Usage:**
```json
{
  "tool_code": "file_search",
  "tool_params": {
    "path_pattern": "src/**/*.rs",
    "pattern": "fn $FN_NAME()",
    "language": "rust"
  }
}
```

## replace

Replaces patterns in code provided as a string. Useful for in-memory code transformations.

**Parameters:**
- `code`: The source code string to modify.
- `pattern`: The ast-grep pattern to search for.
- `replacement`: The ast-grep replacement pattern.
- `language`: The programming language of the code.

**Example Usage:**
```json
{
  "tool_code": "replace",
  "tool_params": {
    "code": "function oldName() { console.log(\"Hello\"); }",
    "pattern": "function oldName()",
    "replacement": "function newName()",
    "language": "javascript"
  }
}
```

## file_replace

Replaces patterns within a specified file. The rewritten content is returned, the file is NOT modified in place.

**Parameters:**
- `path_pattern`: A glob pattern for files to modify (e.g., "src/**/*.js").
- `pattern`: The ast-grep pattern to search for.
- `replacement`: The ast-grep replacement pattern.
- `language`: The programming language of the file.
- `max_results` (optional): Maximum number of results to return (default: 1000).
- `max_file_size` (optional): Maximum file size to process in bytes (default: 50MB).
- `cursor` (optional): Continuation token from previous search for pagination.

**Example Usage:**
```json
{
  "tool_code": "file_replace",
  "tool_params": {
    "path_pattern": "src/**/*.js",
    "pattern": "const $VAR = $VAL",
    "replacement": "let $VAR = $VAL",
    "language": "javascript"
  }
}
```

**Output Format for all tools:**

`search` and `file_search` return a list of matches. Each match includes:
- `text`: The full text of the matched code snippet.
- `vars`: A dictionary (key-value pairs) of captured variables (e.g., `$VAR`, `$FN_NAME`) and their corresponding matched text.

`replace` and `file_replace` return the `rewritten_code` or `rewritten_file_content` as a string.

```json
{
  "matches": [
    {
      "text": "console.log(\"Hello\")",
      "vars": {
        "VAR": "\"Hello\""
      }
    }
  ]
}
```

## Pagination

`file_search` and `file_replace` support pagination for large result sets. When results are paginated:

- The response includes a `next_cursor` field with a continuation token
- Use this cursor in the `cursor` parameter of the next request
- The `total_files_found` field shows how many files matched the glob pattern
- When `next_cursor.is_complete` is true, no more results are available

**Pagination Example:**
```json
{
  "tool_code": "file_search",
  "tool_params": {
    "path_pattern": "src/**/*.js",
    "pattern": "function $NAME()",
    "language": "javascript",
    "max_results": 10,
    "cursor": {
      "last_file_path": "c3JjL2NvbXBvbmVudHMvQnV0dG9uLmpz",
      "is_complete": false
    }
  }
}
```
        "##;
        Ok(DocumentationResult {
            documentation: docs.to_string(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchResult {
    pub text: String,
    pub vars: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchParam {
    pub code: String,
    pub pattern: String,
    pub language: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub matches: Vec<MatchResult>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchCursor {
    /// Base64-encoded continuation token
    pub last_file_path: String,
    /// Whether this cursor represents the end of results
    pub is_complete: bool,
}

impl SearchCursor {
    pub fn new(path: &str) -> Self {
        Self {
            last_file_path: general_purpose::STANDARD.encode(path.as_bytes()),
            is_complete: false,
        }
    }

    pub fn complete() -> Self {
        Self {
            last_file_path: String::new(),
            is_complete: true,
        }
    }

    pub fn decode_path(&self) -> Result<String, ServiceError> {
        if self.is_complete {
            return Ok(String::new());
        }
        general_purpose::STANDARD
            .decode(&self.last_file_path)
            .map_err(|e| ServiceError::Internal(format!("Invalid cursor: {}", e)))
            .and_then(|bytes| {
                String::from_utf8(bytes)
                    .map_err(|e| ServiceError::Internal(format!("Invalid cursor encoding: {}", e)))
            })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileSearchParam {
    pub path_pattern: String,
    pub pattern: String,
    pub language: String,
    #[serde(default)]
    pub max_results: Option<usize>,
    #[serde(default)]
    pub max_file_size: Option<u64>,
    /// Continuation token from previous search
    #[serde(default)]
    pub cursor: Option<SearchCursor>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileSearchResult {
    pub file_results: Vec<FileMatchResult>,
    /// Continuation token for next page (None if no more results)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<SearchCursor>,
    /// Total number of files that matched the glob pattern (for progress indication)
    pub total_files_found: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMatchResult {
    pub file_path: PathBuf,
    pub matches: Vec<MatchResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplaceParam {
    pub code: String,
    pub pattern: String,
    pub replacement: String,
    pub language: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplaceResult {
    pub rewritten_code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileReplaceParam {
    pub path_pattern: String,
    pub pattern: String,
    pub replacement: String,
    pub language: String,
    #[serde(default)]
    pub max_results: Option<usize>,
    #[serde(default)]
    pub max_file_size: Option<u64>,
    /// Continuation token from previous replace operation
    #[serde(default)]
    pub cursor: Option<SearchCursor>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileReplaceResult {
    pub file_results: Vec<FileReplaceResultItem>,
    /// Continuation token for next page (None if no more results)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<SearchCursor>,
    /// Total number of files that matched the glob pattern (for progress indication)
    pub total_files_found: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileReplaceResultItem {
    pub file_path: PathBuf,
    pub rewritten_content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListLanguagesParam {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListLanguagesResult {
    pub languages: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentationParam {}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentationResult {
    pub documentation: String,
}

#[async_trait]
impl ServerHandler for AstGrepService {
    fn get_info(&self) -> InitializeResult {
        InitializeResult {
            protocol_version: ProtocolVersion::LATEST,
            server_info: Implementation {
                name: "ast-grep-mcp".to_string(),
                version: "0.1.0".to_string(),
            },
            capabilities: ServerCapabilities {
                tools: Some(rmcp::model::ToolsCapability { list_changed: Some(true) }),
                ..Default::default()
            },
            instructions: Some("This MCP server provides tools for structural code search and analysis using ast-grep. You can search for code patterns within strings or files, and extract metavariables. Use the `documentation` tool for detailed usage examples.".to_string()),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: vec![
                Tool {
                    name: "search".to_string().into(),
                    description: "Search for patterns in code using ast-grep.".to_string().into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": { "code": { "type": "string" }, "pattern": { "type": "string" }, "language": { "type": "string" } } })).unwrap()),
                },
                Tool {
                    name: "file_search".to_string().into(),
                    description: "Search for patterns in a file using ast-grep.".to_string().into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path_pattern": { "type": "string" },
                            "pattern": { "type": "string" },
                            "language": { "type": "string" },
                            "max_results": { "type": "integer", "minimum": 1, "maximum": 10000 },
                            "max_file_size": { "type": "integer", "minimum": 1024, "maximum": 1073741824 },
                            "cursor": {
                                "type": "object",
                                "properties": {
                                    "last_file_path": { "type": "string" },
                                    "is_complete": { "type": "boolean" }
                                },
                                "required": ["last_file_path", "is_complete"]
                            }
                        },
                        "required": ["path_pattern", "pattern", "language"]
                    })).unwrap()),
                },
                Tool {
                    name: "replace".to_string().into(),
                    description: "Replace patterns in code.".to_string().into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": { "code": { "type": "string" }, "pattern": { "type": "string" }, "replacement": { "type": "string" }, "language": { "type": "string" } } })).unwrap()),
                },
                Tool {
                    name: "file_replace".to_string().into(),
                    description: "Replace patterns in a file.".to_string().into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path_pattern": { "type": "string" },
                            "pattern": { "type": "string" },
                            "replacement": { "type": "string" },
                            "language": { "type": "string" },
                            "max_results": { "type": "integer", "minimum": 1, "maximum": 10000 },
                            "max_file_size": { "type": "integer", "minimum": 1024, "maximum": 1073741824 },
                            "cursor": {
                                "type": "object",
                                "properties": {
                                    "last_file_path": { "type": "string" },
                                    "is_complete": { "type": "boolean" }
                                },
                                "required": ["last_file_path", "is_complete"]
                            }
                        },
                        "required": ["path_pattern", "pattern", "replacement", "language"]
                    })).unwrap()),
                },
                Tool {
                    name: "list_languages".to_string().into(),
                    description: "List all supported programming languages.".to_string().into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": {} })).unwrap()),
                },
                Tool {
                    name: "documentation".to_string().into(),
                    description: "Provides detailed usage examples for all tools.".to_string().into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": {} })).unwrap()),
                },
                ],
                ..Default::default()
            })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        match request.name.as_ref() {
            "search" => {
                let param: SearchParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.search(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "file_search" => {
                let param: FileSearchParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.file_search(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "replace" => {
                let param: ReplaceParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.replace(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "file_replace" => {
                let param: FileReplaceParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.file_replace(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "list_languages" => {
                let param: ListLanguagesParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.list_languages(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "documentation" => {
                let param: DocumentationParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.documentation(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            _ => Err(ErrorData::method_not_found::<
                rmcp::model::CallToolRequestMethod,
            >()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_basic() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "function greet() { console.log(\"Hello\"); }".to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "javascript".to_string(),
        };

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].text, "console.log(\"Hello\")");
        assert_eq!(
            result.matches[0].vars.get("VAR"),
            Some(&"\"Hello\"".to_string())
        );
    }

    #[tokio::test]
    async fn test_search_no_matches() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "function greet() { alert(\"Hello\"); }".to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "javascript".to_string(),
        };

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 0);
    }

    #[tokio::test]
    async fn test_search_invalid_language() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "function greet() { console.log(\"Hello\"); }".to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "invalid_language".to_string(),
        };

        let result = service.search(param).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ServiceError::Internal(_)
        ));
    }

    #[tokio::test]
    async fn test_replace_basic() {
        let service = AstGrepService::new();
        let param = ReplaceParam {
            code: "function oldName() { console.log(\"Hello\"); }".to_string(),
            pattern: "function oldName()".to_string(),
            replacement: "function newName()".to_string(),
            language: "javascript".to_string(),
        };

        let result = service.replace(param).await.unwrap();
        assert!(result.rewritten_code.contains("function newName()"));
        assert!(!result.rewritten_code.contains("function oldName()"));
    }

    #[tokio::test]
    async fn test_replace_with_vars() {
        let service = AstGrepService::new();
        let param = ReplaceParam {
            code: "const x = 5; const y = 10;".to_string(),
            pattern: "const $VAR = $VAL".to_string(),
            replacement: "let $VAR = $VAL".to_string(),
            language: "javascript".to_string(),
        };

        let result = service.replace(param).await.unwrap();
        assert!(result.rewritten_code.contains("let x = 5"));
        assert!(result.rewritten_code.contains("let y = 10"));
        assert!(!result.rewritten_code.contains("const"));
    }

    #[tokio::test]
    async fn test_rust_pattern_matching() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "fn main() { println!(\"Hello, world!\"); }".to_string(),
            pattern: "println!($VAR)".to_string(),
            language: "rust".to_string(),
        };

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].text, "println!(\"Hello, world!\")");
        assert_eq!(
            result.matches[0].vars.get("VAR"),
            Some(&"\"Hello, world!\"".to_string())
        );
    }

    #[tokio::test]
    async fn test_list_languages() {
        let service = AstGrepService::new();
        let param = ListLanguagesParam {};

        let result = service.list_languages(param).await.unwrap();
        assert!(!result.languages.is_empty());
        assert!(result.languages.contains(&"rust".to_string()));
        assert!(result.languages.contains(&"javascript".to_string()));
        assert!(result.languages.contains(&"python".to_string()));
    }

    #[tokio::test]
    async fn test_search_cursor() {
        // Test cursor creation and decoding
        let cursor = SearchCursor::new("src/main.rs");
        assert!(!cursor.is_complete);

        let decoded = cursor.decode_path().unwrap();
        assert_eq!(decoded, "src/main.rs");

        // Test complete cursor
        let complete_cursor = SearchCursor::complete();
        assert!(complete_cursor.is_complete);
        assert_eq!(complete_cursor.decode_path().unwrap(), "");
    }

    #[tokio::test]
    async fn test_pagination_configuration() {
        let custom_config = ServiceConfig {
            max_file_size: 1024 * 1024, // 1MB
            max_concurrency: 5,
            max_results: 10,
        };

        let service = AstGrepService::with_config(custom_config);
        assert_eq!(service.config.max_file_size, 1024 * 1024);
        assert_eq!(service.config.max_concurrency, 5);
        assert_eq!(service.config.max_results, 10);
    }

    #[tokio::test]
    async fn test_documentation() {
        let service = AstGrepService::new();
        let param = DocumentationParam {};

        let result = service.documentation(param).await.unwrap();
        assert!(result.documentation.contains("search"));
        assert!(result.documentation.contains("file_search"));
        assert!(result.documentation.contains("replace"));
        assert!(result.documentation.contains("file_replace"));
    }

    #[tokio::test]
    async fn test_multiple_matches() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "console.log(\"Hello\"); console.log(\"World\"); alert(\"test\");".to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "javascript".to_string(),
        };

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 2);
        assert_eq!(
            result.matches[0].vars.get("VAR"),
            Some(&"\"Hello\"".to_string())
        );
        assert_eq!(
            result.matches[1].vars.get("VAR"),
            Some(&"\"World\"".to_string())
        );
    }

    #[tokio::test]
    async fn test_complex_pattern() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "function test(a, b) { return a + b; } function add(x, y) { return x + y; }"
                .to_string(),
            pattern: "function $NAME($PARAM1, $PARAM2) { return $PARAM1 + $PARAM2; }".to_string(),
            language: "javascript".to_string(),
        };

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 2);

        // Check first match
        assert_eq!(
            result.matches[0].vars.get("NAME"),
            Some(&"test".to_string())
        );
        assert_eq!(result.matches[0].vars.get("PARAM1"), Some(&"a".to_string()));
        assert_eq!(result.matches[0].vars.get("PARAM2"), Some(&"b".to_string()));

        // Check second match
        assert_eq!(result.matches[1].vars.get("NAME"), Some(&"add".to_string()));
        assert_eq!(result.matches[1].vars.get("PARAM1"), Some(&"x".to_string()));
        assert_eq!(result.matches[1].vars.get("PARAM2"), Some(&"y".to_string()));
    }
}
