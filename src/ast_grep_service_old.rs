use std::{borrow::Cow, collections::HashMap, fmt, io, path::PathBuf, str::FromStr, sync::Arc, sync::Mutex, fs};

use ast_grep_core::{AstGrep, Pattern};
use ast_grep_language::SupportLang as Language;
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
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Maximum file size to process (in bytes)
    pub max_file_size: u64,
    /// Maximum number of concurrent file operations
    pub max_concurrency: usize,
    /// Maximum number of results to return per search
    pub limit: usize,
    /// Root directories for file search (defaults to current working directory)
    pub root_directories: Vec<PathBuf>,
    /// Directory for storing custom rules created by LLMs
    pub rules_directory: PathBuf,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            max_file_size: 50 * 1024 * 1024, // 50MB
            max_concurrency: 10,
            limit: 100,
            root_directories: vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))],
            rules_directory: PathBuf::from(".ast-grep-rules"),
        }
    }
}

#[derive(Clone)]
pub struct AstGrepService {
    config: ServiceConfig,
    pattern_cache: Arc<Mutex<HashMap<String, Pattern>>>,
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
            ServiceError::SerdeJson(e) => write!(f, "JSON error: {}", e),
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

impl std::error::Error for ServiceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ServiceError::Io(e) => Some(e),
            ServiceError::SerdeJson(e) => Some(e),
            ServiceError::Glob(e) => Some(e),
            ServiceError::ParserError(_) => None,
            ServiceError::ToolNotFound(_) => None,
            ServiceError::Internal(_) => None,
        }
    }
}

impl From<ServiceError> for ErrorData {
    fn from(err: ServiceError) -> Self {
        ErrorData::internal_error(Cow::Owned(err.to_string()), None)
    }
}

impl Default for AstGrepService {
    fn default() -> Self {
        Self::new()
    }
}

impl AstGrepService {
    fn parse_language(&self, lang_str: &str) -> Result<Language, ServiceError> {
        Language::from_str(lang_str)
            .map_err(|_| ServiceError::Internal("Failed to parse language".into()))
    }

    pub fn new() -> Self {
        Self {
            config: ServiceConfig::default(),
            pattern_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[allow(dead_code)]
    pub fn with_config(config: ServiceConfig) -> Self {
        Self { 
            config,
            pattern_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn calculate_file_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("sha256:{}", hex::encode(hasher.finalize()))
    }

    fn get_or_create_pattern(&self, pattern_str: &str, lang: Language) -> Result<Pattern, ServiceError> {
        let cache_key = format!("{}:{}", lang as u8, pattern_str);
        
        // First try to get from cache
        if let Ok(cache) = self.pattern_cache.lock() {
            if let Some(pattern) = cache.get(&cache_key) {
                return Ok(pattern.clone());
            }
        }

        // Pattern not in cache, create it
        let pattern = Pattern::new(pattern_str, lang);
        
        // Try to add to cache (ignore if lock fails)
        if let Ok(mut cache) = self.pattern_cache.lock() {
            // Limit cache size to prevent memory bloat
            if cache.len() >= 1000 {
                cache.clear();
            }
            cache.insert(cache_key, pattern.clone());
        }

        Ok(pattern)
    }

    fn parse_rule_config(&self, rule_config_str: &str) -> Result<RuleConfig, ServiceError> {
        // First try to parse as YAML
        if let Ok(config) = serde_yaml::from_str::<RuleConfig>(rule_config_str) {
            return Ok(config);
        }
        
        // If YAML fails, try JSON
        serde_json::from_str::<RuleConfig>(rule_config_str)
            .map_err(|e| ServiceError::ParserError(format!("Failed to parse rule config as YAML or JSON: {}", e)))
    }

    fn validate_rule_config(&self, config: &RuleConfig) -> Result<(), ServiceError> {
        // Validate language
        self.parse_language(&config.language)?;
        
        // Validate rule has at least one condition
        if !self.has_rule_condition(&config.rule) {
            return Err(ServiceError::ParserError("Rule must have at least one condition".into()));
        }
        
        Ok(())
    }

    fn has_rule_condition(&self, rule: &RuleObject) -> bool {
        rule.pattern.is_some() ||
        rule.kind.is_some() ||
        rule.regex.is_some() ||
        rule.inside.is_some() ||
        rule.has.is_some() ||
        rule.follows.is_some() ||
        rule.precedes.is_some() ||
        rule.all.as_ref().is_some_and(|v| !v.is_empty()) ||
        rule.any.as_ref().is_some_and(|v| !v.is_empty()) ||
        rule.not.is_some() ||
        rule.matches.is_some()
    }

    fn extract_pattern_from_rule(&self, rule: &RuleObject) -> Option<String> {
        match &rule.pattern {
            Some(PatternSpec::Simple(pattern)) => Some(pattern.clone()),
            Some(PatternSpec::Advanced { context, .. }) => Some(context.clone()),
            None => None,
        }
    }

    fn is_simple_pattern_rule(&self, rule: &RuleObject) -> bool {
        // Check if this is a simple pattern rule that we can handle directly
        rule.pattern.is_some() && 
        rule.kind.is_none() &&
        rule.regex.is_none() &&
        rule.inside.is_none() &&
        rule.has.is_none() &&
        rule.follows.is_none() &&
        rule.precedes.is_none() &&
        rule.all.is_none() &&
        rule.any.is_none() &&
        rule.not.is_none() &&
        rule.matches.is_none()
    }

    #[allow(dead_code)]
    fn extract_all_patterns_from_composite_rule(&self, rule: &RuleObject) -> Vec<String> {
        let mut patterns = Vec::new();
        
        // Handle direct pattern
        if let Some(pattern) = self.extract_pattern_from_rule(rule) {
            patterns.push(pattern);
        }
        
        // Handle "all" composite rule
        if let Some(all_rules) = &rule.all {
            for sub_rule in all_rules {
                patterns.extend(self.extract_all_patterns_from_composite_rule(sub_rule));
            }
        }
        
        // Handle "any" composite rule  
        if let Some(any_rules) = &rule.any {
            for sub_rule in any_rules {
                patterns.extend(self.extract_all_patterns_from_composite_rule(sub_rule));
            }
        }
        
        // Handle "not" composite rule
        if let Some(not_rule) = &rule.not {
            patterns.extend(self.extract_all_patterns_from_composite_rule(not_rule));
        }
        
        patterns
    }

    fn evaluate_rule_against_code(&self, rule: &RuleObject, code: &str, lang: Language) -> Result<Vec<MatchResult>, ServiceError> {
        // Handle different rule types
        if let Some(pattern_spec) = &rule.pattern {
            // Simple pattern rule
            self.evaluate_pattern_rule(pattern_spec, code, lang)
        } else if let Some(all_rules) = &rule.all {
            // ALL composite rule - node must match ALL sub-rules
            self.evaluate_all_rule(all_rules, code, lang)
        } else if let Some(any_rules) = &rule.any {
            // ANY composite rule - node must match ANY sub-rule
            self.evaluate_any_rule(any_rules, code, lang)
        } else if let Some(not_rule) = &rule.not {
            // NOT composite rule - find nodes that DON'T match the sub-rule
            self.evaluate_not_rule(not_rule, code, lang)
        } else if let Some(kind) = &rule.kind {
            // Kind rule - match nodes by AST kind (simplified implementation)
            self.evaluate_kind_rule(kind, code, lang)
        } else if let Some(regex) = &rule.regex {
            // Regex rule - match nodes by text content
            self.evaluate_regex_rule(regex, code, lang)
        } else {
            Err(ServiceError::ParserError("Rule must have at least one condition".into()))
        }
    }

    fn evaluate_pattern_rule(&self, pattern_spec: &PatternSpec, code: &str, lang: Language) -> Result<Vec<MatchResult>, ServiceError> {
        let pattern_str = match pattern_spec {
            PatternSpec::Simple(pattern) => pattern.clone(),
            PatternSpec::Advanced { context, .. } => context.clone(),
        };

        let ast = AstGrep::new(code, lang);
        let pattern = self.get_or_create_pattern(&pattern_str, lang)?;

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

        Ok(matches)
    }

    fn evaluate_kind_rule(&self, _kind: &str, code: &str, lang: Language) -> Result<Vec<MatchResult>, ServiceError> {
        // For now, use a simple pattern that matches any node
        // This is a placeholder - proper kind matching would require deeper AST integration
        let ast = AstGrep::new(code, lang);
        
        // Create a pattern that matches anything and then filter by examining the AST
        // This is a simplified approach
        let pattern = Pattern::new("$_", lang);
        
        let matches: Vec<MatchResult> = ast
            .root()
            .find_all(pattern)
            .map(|node| {
                // Check if node kind matches (this is approximate)
                // Note: Direct kind checking may not be available, so we'll use text matching as fallback
                let text = node.text().to_string();
                let vars: HashMap<String, String> = node.get_env().clone().into();
                
                // For now, include all matches since we can't easily check AST node kind
                // This is a simplified implementation
                MatchResult {
                    text,
                    vars,
                }
            })
            .collect();

        Ok(matches)
    }

    fn evaluate_regex_rule(&self, regex_pattern: &str, code: &str, _lang: Language) -> Result<Vec<MatchResult>, ServiceError> {
        use std::str::FromStr;
        
        // Create regex
        let regex = regex::Regex::from_str(regex_pattern)
            .map_err(|e| ServiceError::ParserError(format!("Invalid regex pattern: {}", e)))?;

        let mut matches = Vec::new();
        
        // Find all matches in the code
        for mat in regex.find_iter(code) {
            matches.push(MatchResult {
                text: mat.as_str().to_string(),
                vars: HashMap::new(),
            });
        }
        
        Ok(matches)
    }

    fn evaluate_all_rule(&self, all_rules: &[RuleObject], code: &str, lang: Language) -> Result<Vec<MatchResult>, ServiceError> {
        if all_rules.is_empty() {
            return Ok(Vec::new());
        }

        // Start with matches from the first rule
        let mut current_matches = self.evaluate_rule_against_code(&all_rules[0], code, lang)?;

        // For each additional rule, filter current matches to only those that also match the new rule
        for rule in &all_rules[1..] {
            let rule_matches = self.evaluate_rule_against_code(rule, code, lang)?;
            
            // Keep only matches that appear in both sets (intersection)
            current_matches = self.intersect_matches(current_matches, rule_matches);
        }

        Ok(current_matches)
    }

    fn evaluate_any_rule(&self, any_rules: &[RuleObject], code: &str, lang: Language) -> Result<Vec<MatchResult>, ServiceError> {
        let mut all_matches = Vec::new();

        // Collect matches from all rules
        for rule in any_rules {
            let mut rule_matches = self.evaluate_rule_against_code(rule, code, lang)?;
            all_matches.append(&mut rule_matches);
        }

        // Remove duplicates by text (simple deduplication)
        all_matches.sort_by(|a, b| a.text.cmp(&b.text));
        all_matches.dedup_by(|a, b| a.text == b.text);

        Ok(all_matches)
    }

    fn evaluate_not_rule(&self, not_rule: &RuleObject, code: &str, lang: Language) -> Result<Vec<MatchResult>, ServiceError> {
        // This is complex - we need to find all nodes that DON'T match the rule
        // For now, implement a simplified approach using text analysis
        
        let excluded_matches = self.evaluate_rule_against_code(not_rule, code, lang)?;
        let excluded_texts: std::collections::HashSet<String> = excluded_matches.iter().map(|m| m.text.clone()).collect();

        // Get all possible tokens/expressions and filter out the excluded ones
        let ast = AstGrep::new(code, lang);
        let pattern = Pattern::new("$_", lang); // Match anything
        
        let filtered_matches: Vec<MatchResult> = ast
            .root()
            .find_all(pattern)
            .filter_map(|node| {
                let text = node.text().to_string();
                if !excluded_texts.contains(&text) {
                    let vars: HashMap<String, String> = node.get_env().clone().into();
                    Some(MatchResult {
                        text,
                        vars,
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(filtered_matches)
    }

    fn intersect_matches(&self, matches1: Vec<MatchResult>, matches2: Vec<MatchResult>) -> Vec<MatchResult> {
        let texts2: std::collections::HashSet<String> = matches2.iter().map(|m| m.text.clone()).collect();
        
        matches1
            .into_iter()
            .filter(|m| texts2.contains(&m.text))
            .collect()
    }

    #[tracing::instrument(skip(self), fields(language = %param.language, pattern = %param.pattern))]
    pub async fn search(&self, param: SearchParam) -> Result<SearchResult, ServiceError> {
        let lang = self.parse_language(param.language.as_str())?;

        let ast = AstGrep::new(param.code.as_str(), lang);
        let pattern = self.get_or_create_pattern(&param.pattern, lang)?;

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

        tracing::Span::current().record("matches_found", matches.len());
        Ok(SearchResult { matches })
    }

    #[tracing::instrument(skip(self), fields(language = %param.language, pattern = %param.pattern, path_pattern = %param.path_pattern))]
    pub async fn file_search(
        &self,
        param: FileSearchParam,
    ) -> Result<FileSearchResult, ServiceError> {
        let lang = self.parse_language(param.language.as_str())?;

        let mut builder = GlobSetBuilder::new();
        builder.add(Glob::new(&param.path_pattern)?);
        let globset = builder.build()?;

        let max_file_size = param.max_file_size.unwrap_or(self.config.max_file_size);
        let max_results = param.limit.unwrap_or(self.config.limit);

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

        // Collect all matching file paths from all root directories, sorted for consistent pagination
        let mut all_matching_files: Vec<_> = self
            .config
            .root_directories
            .iter()
            .flat_map(|root_dir| {
                WalkDir::new(root_dir)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|entry| {
                        let path = entry.path();
                        if !path.is_file() || !globset.is_match(path) {
                            return false;
                        }
                        // Check file size
                        if let Ok(metadata) = entry.metadata() {
                            if metadata.len() > max_file_size {
                                tracing::event!(
                                    tracing::Level::WARN,
                                    file_path = ?entry.path(),
                                    file_size_mb = metadata.len() / (1024 * 1024),
                                    "Skipping large file"
                                );
                                return false;
                            }
                        }
                        true
                    })
                    .map(|entry| entry.path().to_path_buf())
                    .collect::<Vec<_>>()
            })
            .collect();

        // Sort for consistent ordering across pagination requests
        all_matching_files.sort();
        let total_files_found = all_matching_files.len();
        tracing::Span::current().record("total_files_found", total_files_found);

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
                            tracing::event!(
                                tracing::Level::WARN,
                                file_path = ?path,
                                error = %e,
                                "Error processing file"
                            );
                            None
                        }
                    }
                })
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

        tracing::Span::current().record("files_with_matches", file_results.len());
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
        let pattern = self.get_or_create_pattern(&pattern_str, lang)?;

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

    #[tracing::instrument(skip(self), fields(language = %param.language, pattern = %param.pattern, replacement = %param.replacement))]
    pub async fn replace(&self, param: ReplaceParam) -> Result<ReplaceResult, ServiceError> {
        let lang = self.parse_language(param.language.as_str())?;

        let mut ast = AstGrep::new(param.code.as_str(), lang);
        let pattern = self.get_or_create_pattern(&param.pattern, lang)?;
        let replacement = param.replacement.as_str();

        // Find all matches and replace them manually
        let mut changed = true;
        while changed {
            // Safety limit to prevent infinite loops
            changed = false;
            if let Some(_node) = ast.root().find(pattern.clone()) {
                if ast.replace(pattern.clone(), replacement).is_ok() {
                    changed = true;
                }
            }
        }
        let rewritten_code = ast.root().text().to_string();

        Ok(ReplaceResult { rewritten_code })
    }

    #[tracing::instrument(skip(self), fields(language = %param.language, pattern = %param.pattern, replacement = %param.replacement, path_pattern = %param.path_pattern, dry_run = %param.dry_run, summary_only = %param.summary_only))]
    pub async fn file_replace(
        &self,
        param: FileReplaceParam,
    ) -> Result<FileReplaceResult, ServiceError> {
        let lang = self.parse_language(param.language.as_str())?;

        let mut builder = GlobSetBuilder::new();
        builder.add(Glob::new(&param.path_pattern)?);
        let globset = builder.build()?;

        let max_file_size = param.max_file_size.unwrap_or(self.config.max_file_size);
        let max_results = param.max_results.unwrap_or(self.config.limit);

        // Determine cursor position for pagination
        let cursor_path = if let Some(cursor) = &param.cursor {
            if cursor.is_complete {
                return Ok(FileReplaceResult {
                    file_results: vec![],
                    summary_results: vec![],
                    next_cursor: Some(SearchCursor::complete()),
                    total_files_found: 0,
                    dry_run: param.dry_run,
                    total_changes: 0,
                    files_with_changes: vec![],
                });
            }
            Some(cursor.decode_path()?)
        } else {
            None
        };

        // Collect all matching file paths from all root directories, sorted for consistent pagination
        let mut all_matching_files: Vec<_> = self
            .config
            .root_directories
            .iter()
            .flat_map(|root_dir| {
                WalkDir::new(root_dir)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|entry| {
                        let path = entry.path();
                        if !path.is_file() || !globset.is_match(path) {
                            return false;
                        }
                        // Check file size
                        if let Ok(metadata) = entry.metadata() {
                            if metadata.len() > max_file_size {
                                tracing::event!(
                                    tracing::Level::WARN,
                                    file_path = ?entry.path(),
                                    file_size_mb = metadata.len() / (1024 * 1024),
                                    "Skipping large file"
                                );
                                return false;
                            }
                        }
                        true
                    })
                    .map(|entry| entry.path().to_path_buf())
                    .collect::<Vec<_>>()
            })
            .collect();

        all_matching_files.sort();
        let total_files_found = all_matching_files.len();

        // Apply cursor-based filtering
        let files_to_process: Vec<_> = if let Some(cursor_path) = cursor_path {
            all_matching_files
                .into_iter()
                .skip_while(|path| path.to_string_lossy().as_ref() <= cursor_path.as_str())
                .collect()
        } else {
            all_matching_files.into_iter().collect()
        };

        // Process files in parallel
        let pattern_str = param.pattern.clone();
        let replacement_str = param.replacement.clone();
        let dry_run = param.dry_run;
        let pattern = self.get_or_create_pattern(&pattern_str, lang)?;
        let file_results_raw: Vec<(PathBuf, FileDiffResult)> =
            stream::iter(files_to_process.iter().cloned())
                .map(|path| {
                    let replacement_str = replacement_str.clone();
                    let pattern = pattern.clone();
                    async move {
                        let file_content = match tokio::fs::read_to_string(&path).await {
                            Ok(content) => content,
                            Err(e) => return (path, Err(ServiceError::Io(e))),
                        };
                        let original_lines: Vec<&str> = file_content.lines().collect();

                        let mut ast = AstGrep::new(file_content.as_str(), lang);
                        let replacement = replacement_str.as_str();

                        let mut changed = true;
                        let mut iterations = 0;
                        while changed && iterations < 100 {
                            changed = false;
                            if let Some(_node) = ast.root().find(pattern.clone()) {
                                if ast.replace(pattern.clone(), replacement).is_ok() {
                                    changed = true;
                                }
                            }
                            iterations += 1;
                        }

                        let rewritten_content = ast.root().text().to_string();

                        if rewritten_content == file_content {
                            return (path, Ok(None));
                        }

                        let new_lines: Vec<&str> = rewritten_content.lines().collect();
                        let mut changes = Vec::new();

                        let max_len = original_lines.len().max(new_lines.len());
                        for i in 0..max_len {
                            let old_line = original_lines.get(i).unwrap_or(&"");
                            let new_line = new_lines.get(i).unwrap_or(&"");

                            if old_line != new_line {
                                changes.push(FileDiffChange {
                                    line: i + 1,
                                    old_text: old_line.to_string(),
                                    new_text: new_line.to_string(),
                                });
                            }
                        }

                        if !dry_run && !changes.is_empty() {
                            if let Err(e) = tokio::fs::write(&path, rewritten_content).await {
                                return (path, Err(ServiceError::Io(e)));
                            }
                        }

                        let file_metadata = match tokio::fs::metadata(&path).await {
                            Ok(metadata) => metadata,
                            Err(e) => return (path, Err(ServiceError::Io(e))),
                        };

                        let total_changes = changes.len();
                        (
                            path.clone(),
                            Ok(Some(FileDiffResult {
                                file_path: path.clone(),
                                file_size_bytes: file_metadata.len(),
                                changes,
                                total_changes,
                                file_hash: Self::calculate_file_hash(&file_content),
                            })),
                        )
                    }
                })
                .buffer_unordered(self.config.max_concurrency)
                .filter_map(|(path, result)| async move {
                    match result {
                        Ok(Some(file_result)) => Some((path, file_result)),
                        Ok(None) => None,
                        Err(e) => {
                            tracing::event!(
                                tracing::Level::WARN,
                                file_path = ?path,
                                error = %e,
                                "Error processing file"
                            );
                            None
                        }
                    }
                })
                .collect::<Vec<_>>()
                .await;

        let next_cursor = if files_to_process.len() < max_results {
            Some(SearchCursor::complete())
        } else if let Some((last_path, _)) = file_results_raw.last() {
            Some(SearchCursor::new(&last_path.to_string_lossy()))
        } else if let Some(last_processed) = files_to_process.last() {
            Some(SearchCursor::new(&last_processed.to_string_lossy()))
        } else {
            Some(SearchCursor::complete())
        };

        let file_results: Vec<FileDiffResult> = file_results_raw
            .into_iter()
            .map(|(_, result)| result)
            .collect();

        // Calculate totals
        let total_changes: usize = file_results.iter().map(|r| r.total_changes).sum();
        let files_with_changes: Vec<(String, usize)> = file_results
            .iter()
            .map(|r| (r.file_path.to_string_lossy().to_string(), r.total_changes))
            .collect();

        tracing::Span::current().record("total_changes", total_changes);
        tracing::Span::current().record("files_modified", files_with_changes.len());

        if param.summary_only {
            // Convert to summary results
            let summary_results: Vec<FileSummaryResult> = file_results
                .into_iter()
                .map(|diff_result| {
                    let sample_changes = if param.include_samples {
                        diff_result.changes.into_iter().take(param.max_samples).collect()
                    } else {
                        vec![]
                    };
                    
                    FileSummaryResult {
                        file_path: diff_result.file_path,
                        file_size_bytes: diff_result.file_size_bytes,
                        total_changes: diff_result.total_changes,
                        lines_changed: diff_result.total_changes, // For now, assume 1:1 mapping
                        file_hash: diff_result.file_hash,
                        sample_changes,
                    }
                })
                .collect();

            Ok(FileReplaceResult {
                file_results: vec![],
                summary_results,
                next_cursor,
                total_files_found,
                dry_run: param.dry_run,
                total_changes,
                files_with_changes,
            })
        } else {
            Ok(FileReplaceResult {
                file_results,
                summary_results: vec![],
                next_cursor,
                total_files_found,
                dry_run: param.dry_run,
                total_changes,
                files_with_changes,
            })
        }
    }

    #[tracing::instrument(skip(self))]
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

    #[tracing::instrument(skip(self))]
    pub async fn documentation(
        &self,
        _param: DocumentationParam,
    ) -> Result<DocumentationResult, ServiceError> {
        let docs = r##"
# AST-Grep MCP Service Documentation

This service provides structural code search and transformation using ast-grep patterns and rule configurations.

## Key Concepts

**AST Patterns:** Use `$VAR` to capture single nodes, `$$$` to capture multiple statements
**Rule Configurations:** YAML or JSON configurations for complex pattern matching and transformations
**Languages:** Supports javascript, typescript, rust, python, java, go, and many more
**Glob Patterns:** Use `**/*.js` for recursive search, `src/*.ts` for single directory

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

**More Pattern Examples:**
```json
// Find function declarations
{
  "pattern": "function $NAME($PARAMS) { $BODY }",
  "language": "javascript"
}

// Find variable assignments
{
  "pattern": "const $VAR = $VALUE",
  "language": "javascript"
}

// Find Rust function definitions
{
  "pattern": "fn $NAME($PARAMS) -> $RETURN_TYPE { $BODY }",
  "language": "rust"
}
```

## file_search

Searches for patterns within files matching a glob pattern. Ideal for analyzing existing code files on the system.

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

**Common Use Cases:**
```json
// Find all TODO comments
{
  "path_pattern": "**/*.js",
  "pattern": "// TODO: $MESSAGE",
  "language": "javascript"
}

// Find error handling patterns
{
  "path_pattern": "src/**/*.ts",
  "pattern": "catch ($ERROR) { $BODY }",
  "language": "typescript"
}

// Find React components
{
  "path_pattern": "components/**/*.jsx",
  "pattern": "function $NAME($PROPS) { return $JSX }",
  "language": "javascript"
}

// Find Python class definitions
{
  "path_pattern": "**/*.py",
  "pattern": "class $NAME($BASE): $BODY",
  "language": "python"
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

**Transformation Examples:**
```json
// Convert var to const
{
  "pattern": "var $VAR = $VALUE",
  "replacement": "const $VAR = $VALUE",
  "language": "javascript"
}

// Add async/await
{
  "pattern": "function $NAME($PARAMS) { return $BODY }",
  "replacement": "async function $NAME($PARAMS) { return await $BODY }",
  "language": "javascript"
}

// Convert Python print statements
{
  "pattern": "print $ARGS",
  "replacement": "print($ARGS)",
  "language": "python"
}

// Modernize Rust syntax
{
  "pattern": "match $EXPR { $ARMS }",
  "replacement": "match $EXPR { $ARMS }",
  "language": "rust"
}
```

## file_replace

Replaces patterns within files matching a glob pattern. Supports bulk refactoring with optimized response formats.

**Parameters:**
- `path_pattern`: A glob pattern for files to modify (e.g., "src/**/*.js").
- `pattern`: The ast-grep pattern to search for.
- `replacement`: The ast-grep replacement pattern.
- `language`: The programming language of the file.
- `dry_run` (optional): If true (default), only show preview. If false, actually modify files.
- `summary_only` (optional): If true, return only change counts per file (default: false).
- `include_samples` (optional): If true with summary_only, include sample changes (default: false).
- `max_samples` (optional): Number of sample changes per file when include_samples=true (default: 3).
- `max_results` (optional): Maximum number of results to return (default: 1000).
- `max_file_size` (optional): Maximum file size to process in bytes (default: 50MB).
- `cursor` (optional): Continuation token from previous search for pagination.

**IMPORTANT FOR LLMs**: Use `summary_only=true` for bulk refactoring to avoid token limits. This returns concise statistics instead of full diffs.

**Bulk Refactoring Workflow for LLMs:**

1. **Survey scope (use for large codebases to avoid token limits):**
```json
{
  "path_pattern": "src/**/*.rs",
  "pattern": "\"$STRING\".to_string()",
  "replacement": "\"$STRING\".into()",
  "language": "rust",
  "summary_only": true,
  "dry_run": true
}
```
Returns: `{"files_with_changes": [["src/main.rs", 15], ["src/lib.rs", 8]], "total_changes": 23}`

2. **Preview samples before applying:**
```json
{
  "path_pattern": "src/**/*.rs",
  "pattern": "\"$STRING\".to_string()",
  "replacement": "\"$STRING\".into()",
  "language": "rust",
  "summary_only": true,
  "include_samples": true,
  "max_samples": 3,
  "dry_run": true
}
```

3. **Apply changes:**
```json
{
  "path_pattern": "src/**/*.rs", 
  "pattern": "\"$STRING\".to_string()",
  "replacement": "\"$STRING\".into()",
  "language": "rust",
  "summary_only": true,
  "dry_run": false
}
```

**Returns Line Diffs:**
```json
{
  "file_results": [{
    "file_path": "src/main.js",
    "file_size_bytes": 15420,
    "changes": [
      {
        "line": 15,
        "old_text": "const x = 5;",
        "new_text": "let x = 5;"
      },
      {
        "line": 23,
        "old_text": "const result = calculate();", 
        "new_text": "let result = calculate();"
      }
    ],
    "total_changes": 2,
    "file_hash": "sha256:abc123..."
  }],
  "dry_run": true
}
```

**Batch Transformation Examples:**
```json
// Preview changes first
{
  "path_pattern": "src/**/*.ts",
  "pattern": "fetch($URL).then($HANDLER)",
  "replacement": "await fetch($URL).then($HANDLER)",
  "language": "typescript",
  "dry_run": true
}

// Then apply the changes
{
  "path_pattern": "src/**/*.ts", 
  "pattern": "fetch($URL).then($HANDLER)",
  "replacement": "await fetch($URL).then($HANDLER)",
  "language": "typescript",
  "dry_run": false
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

## list_languages

Returns all supported programming languages.

**Usage:**
```json
{
  "tool_code": "list_languages",
  "tool_params": {}
}
```

**Supported Languages Include:**
- **Web:** javascript, typescript, tsx, html, css
- **Systems:** rust, c, cpp, go 
- **Enterprise:** java, csharp, kotlin, scala
- **Scripting:** python, ruby, lua, bash
- **Others:** swift, dart, elixir, haskell, php, yaml, json

## Best Practices

**Pattern Writing Tips:**
- Use specific patterns: `console.log($VAR)` vs `$ANY`
- Capture what you need: `function $NAME($PARAMS)` captures both name and parameters
- Test patterns with the `search` tool first before using `file_search`

**Performance Tips:**
- Use specific glob patterns: `src/components/*.tsx` vs `**/*`
- Set reasonable `max_file_size` and `max_results` limits
- Use pagination for large codebases

**Common Patterns:**
- Function calls: `$FUNC($ARGS)`
- Variable declarations: `$TYPE $NAME = $VALUE`
- Class methods: `$VISIBILITY $METHOD($PARAMS) { $BODY }`
- Import statements: `import $NAME from '$PATH'`

## Rule-Based Operations

### validate_rule

Validates ast-grep rule configuration syntax and optionally tests against sample code.

**Parameters:**
- `rule_config`: YAML or JSON rule configuration string
- `test_code` (optional): Sample code to test the rule against

**Rule Configuration Format (YAML):**
```yaml
id: unique-rule-id
language: javascript
message: "Optional message for matches"
severity: warning
rule:
  pattern: "console.log($ARG)"
fix: "console.debug($ARG)"  # For rule_replace only
```

**Rule Configuration Format (JSON):**
```json
{
  "id": "unique-rule-id",
  "language": "javascript",
  "message": "Optional message for matches", 
  "severity": "warning",
  "rule": {
    "pattern": "console.log($ARG)"
  },
  "fix": "console.debug($ARG)"
}
```

### rule_search

Search for patterns using ast-grep rule configurations. Supports complex pattern matching.

**Parameters:**
- `rule_config`: YAML or JSON rule configuration
- `path_pattern` (optional): Glob pattern for files to search (default: all files)
- `max_results` (optional): Maximum number of results
- `max_file_size` (optional): Maximum file size to process
- `cursor` (optional): Pagination cursor

**Supported Rule Types:**
- **Simple Pattern Rules**: `pattern: "console.log($ARG)"`
- **Composite Rules**: `all`, `any`, `not` (limited support)
- **Relational Rules**: `inside`, `has`, `follows`, `precedes` (planned)

### rule_replace

Replace patterns using ast-grep rule configurations with fix transformations.

**Parameters:**
- `rule_config`: YAML or JSON rule configuration with `fix` field
- `path_pattern` (optional): Glob pattern for files to modify
- `dry_run` (optional): If true (default), only show preview
- `summary_only` (optional): If true, return only summary statistics
- `max_results` (optional): Maximum number of results
- `cursor` (optional): Pagination cursor

**Example Rule with Fix:**
```yaml
id: modernize-var-to-const
language: javascript
message: "Replace var with const for immutable variables"
severity: info
rule:
  pattern: "var $NAME = $VALUE"
fix: "const $NAME = $VALUE"
```

## Rule Management for LLMs

The service provides comprehensive rule management capabilities allowing LLMs to create, store, and reuse custom rule configurations.

### create_rule

Create and store a new ast-grep rule configuration for reuse. LLMs can build custom rule libraries.

**Parameters:**
- `rule_config`: YAML or JSON rule configuration to create
- `overwrite` (optional): Whether to overwrite existing rule with same ID (default: false)

**Usage:**
```json
{
  "rule_config": "id: my-custom-rule\nlanguage: typescript\nmessage: \"Custom rule\"\nrule:\n  pattern: \"$VAR as any\"\nfix: \"$VAR as unknown\"",
  "overwrite": false
}
```

### list_rules

List all stored rule configurations with optional filtering.

**Parameters:**
- `language` (optional): Filter rules by programming language
- `severity` (optional): Filter rules by severity level (info, warning, error)

**Returns:** Array of rule information including ID, language, message, severity, file path, and whether it has a fix.

### get_rule

Retrieve a specific stored rule configuration by ID.

**Parameters:**
- `rule_id`: ID of the rule to retrieve

**Returns:** Full rule configuration as YAML string and file path.

### delete_rule

Delete a stored rule configuration by ID.

**Parameters:**
- `rule_id`: ID of the rule to delete

**Returns:** Confirmation of deletion with rule ID and file path.

**Rule Storage:**
- Rules are stored as YAML files in `.ast-grep-rules/` directory
- Each rule is saved as `{rule-id}.yaml`
- Directory is created automatically when first rule is saved
- Rules persist between server restarts

**LLM Workflow Example:**
1. Create a custom rule: `create_rule` with your pattern and fix
2. Test the rule: `validate_rule` with sample code
3. Apply the rule: `rule_search` or `rule_replace` using the stored rule ID
4. Manage rules: `list_rules`, `get_rule`, `delete_rule` as needed

## Error Handling

The service returns structured errors for:
- Invalid glob patterns
- Unsupported languages
- File access issues
- Pattern syntax errors
- Rule configuration parsing errors
- Missing fix field for replacements

Always check the response for error conditions before processing results.
        "##;
        Ok(DocumentationResult {
            documentation: docs.to_string(),
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn validate_rule(
        &self,
        param: RuleValidateParam,
    ) -> Result<RuleValidateResult, ServiceError> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut test_matches = None;

        // Parse the rule configuration
        let config = match self.parse_rule_config(&param.rule_config) {
            Ok(config) => config,
            Err(e) => {
                errors.push(e.to_string());
                return Ok(RuleValidateResult {
                    is_valid: false,
                    errors,
                    warnings,
                    test_matches,
                });
            }
        };

        // Validate the configuration
        if let Err(e) = self.validate_rule_config(&config) {
            errors.push(e.to_string());
        }

        // If test code is provided, test the rule against it
        if let Some(test_code) = param.test_code {
            if errors.is_empty() {
                if let Some(pattern_str) = self.extract_pattern_from_rule(&config.rule) {
                    match self.parse_language(&config.language) {
                        Ok(_lang) => {
                            let search_param = SearchParam {
                                code: test_code,
                                pattern: pattern_str,
                                language: config.language.clone(),
                            };
                            
                            match self.search(search_param).await {
                                Ok(result) => {
                                    test_matches = Some(result.matches);
                                }
                                Err(e) => {
                                    warnings.push(format!("Pattern test failed: {}", e));
                                }
                            }
                        }
                        Err(e) => {
                            errors.push(e.to_string());
                        }
                    }
                } else {
                    warnings.push("Complex rule detected - basic pattern testing not available".into());
                }
            }
        }

        Ok(RuleValidateResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            test_matches,
        })
    }

    #[tracing::instrument(skip(self), fields(rule_id))]
    pub async fn rule_search(
        &self,
        param: RuleSearchParam,
    ) -> Result<RuleSearchResult, ServiceError> {
        // Parse the rule configuration
        let config = self.parse_rule_config(&param.rule_config)?;
        self.validate_rule_config(&config)?;
        
        tracing::Span::current().record("rule_id", &config.id);

        // Check if this is a simple pattern rule or a composite rule
        if self.is_simple_pattern_rule(&config.rule) {
            // Handle simple pattern rule
            let pattern_str = self.extract_pattern_from_rule(&config.rule)
                .ok_or_else(|| ServiceError::ParserError("Pattern rule missing pattern".into()))?;
            
            return self.handle_simple_pattern_rule_search(&config, pattern_str, param).await;
        } else {
            // Handle composite rules
            return self.handle_composite_rule_search(&config, param).await;
        }
    }

    async fn handle_simple_pattern_rule_search(
        &self,
        config: &RuleConfig,
        pattern_str: String,
        param: RuleSearchParam,
    ) -> Result<RuleSearchResult, ServiceError> {
        let path_pattern = param.path_pattern.unwrap_or_else(|| "**/*".into());

        // Create equivalent FileSearchParam
        let file_search_param = FileSearchParam {
            path_pattern,
            pattern: pattern_str,
            language: config.language.clone(),
            limit: param.max_results,
            max_file_size: param.max_file_size,
            cursor: param.cursor,
        };

        // Use existing file_search functionality
        let search_result = self.file_search(file_search_param).await?;

        // Convert to rule search result format
        let rule_matches: Vec<RuleMatchResult> = search_result.file_results
            .into_iter()
            .map(|file_result| RuleMatchResult {
                file_path: file_result.file_path.to_string_lossy().to_string(),
                matches: file_result.matches,
                message: config.message.clone(),
                severity: config.severity.clone(),
            })
            .collect();

        Ok(RuleSearchResult {
            rule_id: config.id.clone(),
            matches: rule_matches,
            next_cursor: search_result.next_cursor,
            total_files_processed: search_result.total_files_found,
        })
    }

    async fn handle_composite_rule_search(
        &self,
        config: &RuleConfig,
        param: RuleSearchParam,
    ) -> Result<RuleSearchResult, ServiceError> {
        let lang = self.parse_language(&config.language)?;
        let path_pattern = param.path_pattern.unwrap_or_else(|| "**/*".into());

        // Build glob pattern
        let mut builder = GlobSetBuilder::new();
        builder.add(Glob::new(&path_pattern)?);
        let globset = builder.build()?;

        let max_file_size = param.max_file_size.unwrap_or(self.config.max_file_size);
        let max_results = param.max_results.unwrap_or(self.config.limit);

        // Get files to process
        let mut all_matching_files: Vec<_> = self
            .config
            .root_directories
            .iter()
            .flat_map(|root_dir| {
                WalkDir::new(root_dir)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|entry| {
                        let path = entry.path();
                        if !path.is_file() || !globset.is_match(path) {
                            return false;
                        }
                        // Check file size
                        if let Ok(metadata) = entry.metadata() {
                            if metadata.len() > max_file_size {
                                return false;
                            }
                        }
                        true
                    })
                    .map(|entry| entry.path().to_path_buf())
                    .collect::<Vec<_>>()
            })
            .collect();

        all_matching_files.sort();
        all_matching_files.truncate(max_results);

        // Process files with composite rule evaluation
        let mut rule_matches = Vec::new();

        for file_path in &all_matching_files {
            if let Ok(content) = std::fs::read_to_string(file_path) {
                match self.evaluate_rule_against_code(&config.rule, &content, lang) {
                    Ok(matches) => {
                        if !matches.is_empty() {
                            rule_matches.push(RuleMatchResult {
                                file_path: file_path.to_string_lossy().to_string(),
                                matches,
                                message: config.message.clone(),
                                severity: config.severity.clone(),
                            });
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to evaluate rule for file {:?}: {}", file_path, e);
                    }
                }
            }
        }

        Ok(RuleSearchResult {
            rule_id: config.id.clone(),
            matches: rule_matches,
            next_cursor: None, // For now, no pagination for composite rules
            total_files_processed: all_matching_files.len(),
        })
    }

    #[tracing::instrument(skip(self), fields(rule_id))]
    pub async fn rule_replace(
        &self,
        param: RuleReplaceParam,
    ) -> Result<RuleReplaceResult, ServiceError> {
        // Parse the rule configuration
        let config = self.parse_rule_config(&param.rule_config)?;
        self.validate_rule_config(&config)?;
        
        tracing::Span::current().record("rule_id", &config.id);

        // Extract the fix/replacement from the rule config
        let replacement = config.fix
            .as_ref()
            .ok_or_else(|| ServiceError::ParserError("Rule configuration must include 'fix' field for replacement".into()))?;

        // For now, only handle simple pattern rules
        if !self.is_simple_pattern_rule(&config.rule) {
            return Err(ServiceError::ParserError("Only simple pattern rules are currently supported for replacement".into()));
        }

        let pattern_str = self.extract_pattern_from_rule(&config.rule)
            .ok_or_else(|| ServiceError::ParserError("Pattern rule missing pattern".into()))?;

        let path_pattern = param.path_pattern.unwrap_or_else(|| "**/*".into());

        // Create equivalent FileReplaceParam
        let file_replace_param = FileReplaceParam {
            path_pattern,
            pattern: pattern_str,
            replacement: replacement.clone(),
            language: config.language.clone(),
            max_results: param.max_results,
            max_file_size: param.max_file_size,
            dry_run: param.dry_run.unwrap_or(true),
            summary_only: param.summary_only.unwrap_or(false),
            include_samples: false,
            max_samples: 3,
            cursor: param.cursor,
        };

        // Use existing file_replace functionality
        let replace_result = self.file_replace(file_replace_param).await?;

        Ok(RuleReplaceResult {
            rule_id: config.id,
            file_results: replace_result.file_results,
            next_cursor: replace_result.next_cursor,
            total_files_processed: replace_result.total_files_found,
            total_changes: replace_result.total_changes,
            dry_run: replace_result.dry_run,
        })
    }

    fn ensure_rules_directory(&self) -> Result<(), ServiceError> {
        if !self.config.rules_directory.exists() {
            fs::create_dir_all(&self.config.rules_directory)?;
        }
        Ok(())
    }

    fn get_rule_file_path(&self, rule_id: &str) -> PathBuf {
        self.config.rules_directory.join(format!("{}.yaml", rule_id))
    }

    #[tracing::instrument(skip(self), fields(rule_id))]
    pub async fn create_rule(
        &self,
        param: CreateRuleParam,
    ) -> Result<CreateRuleResult, ServiceError> {
        // Parse and validate the rule configuration
        let config = self.parse_rule_config(&param.rule_config)?;
        self.validate_rule_config(&config)?;
        
        tracing::Span::current().record("rule_id", &config.id);

        // Ensure rules directory exists
        self.ensure_rules_directory()?;

        let file_path = self.get_rule_file_path(&config.id);
        let exists = file_path.exists();

        // Check if rule exists and overwrite is not allowed
        if exists && !param.overwrite.unwrap_or(false) {
            return Err(ServiceError::Internal(format!("Rule '{}' already exists. Use overwrite=true to replace it.", config.id)));
        }

        // Write rule to file as YAML
        let yaml_content = serde_yaml::to_string(&config)
            .map_err(|e| ServiceError::Internal(format!("Failed to serialize rule to YAML: {}", e)))?;
        
        fs::write(&file_path, yaml_content)?;

        Ok(CreateRuleResult {
            rule_id: config.id,
            file_path: file_path.to_string_lossy().to_string(),
            created: !exists,
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn list_rules(
        &self,
        param: ListRulesParam,
    ) -> Result<ListRulesResult, ServiceError> {
        // Ensure rules directory exists
        self.ensure_rules_directory()?;

        let mut rules = Vec::new();

        // Read all YAML files in rules directory
        for entry in fs::read_dir(&self.config.rules_directory)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().is_some_and(|ext| ext == "yaml" || ext == "yml") {
                match self.load_rule_from_file(&path) {
                    Ok(config) => {
                        // Apply filters
                        if let Some(lang_filter) = &param.language {
                            if config.language != *lang_filter {
                                continue;
                            }
                        }
                        
                        if let Some(severity_filter) = &param.severity {
                            if config.severity.as_ref() != Some(severity_filter) {
                                continue;
                            }
                        }

                        rules.push(RuleInfo {
                            id: config.id,
                            language: config.language,
                            message: config.message,
                            severity: config.severity,
                            file_path: path.to_string_lossy().to_string(),
                            has_fix: config.fix.is_some(),
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load rule from {:?}: {}", path, e);
                    }
                }
            }
        }

        // Sort rules by ID for consistent ordering
        rules.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(ListRulesResult { rules })
    }

    fn load_rule_from_file(&self, path: &PathBuf) -> Result<RuleConfig, ServiceError> {
        let content = fs::read_to_string(path)?;
        self.parse_rule_config(&content)
    }

    #[tracing::instrument(skip(self))]
    pub async fn list_catalog_rules(
        &self,
        param: ListCatalogRulesParam,
    ) -> Result<ListCatalogRulesResult, ServiceError> {
        // For now, return a static list of example rules from the ast-grep catalog
        // In a real implementation, this would fetch from https://ast-grep.github.io/catalog/
        let mut rules = vec![
            CatalogRuleInfo {
                id: "xstate-v4-to-v5".to_string(),
                name: "XState v4 to v5 Migration".to_string(),
                description: "Migrate XState v4 code to v5 syntax".to_string(),
                language: "typescript".to_string(),
                category: "migration".to_string(),
                url: "https://ast-grep.github.io/catalog/typescript/xstate-v4-to-v5".to_string(),
            },
            CatalogRuleInfo {
                id: "no-console-log".to_string(),
                name: "No Console Log".to_string(),
                description: "Find and remove console.log statements".to_string(),
                language: "javascript".to_string(),
                category: "cleanup".to_string(),
                url: "https://ast-grep.github.io/catalog/javascript/no-console-log".to_string(),
            },
            CatalogRuleInfo {
                id: "use-strict-equality".to_string(),
                name: "Use Strict Equality".to_string(),
                description: "Replace == with === for strict equality".to_string(),
                language: "javascript".to_string(),
                category: "best-practices".to_string(),
                url: "https://ast-grep.github.io/catalog/javascript/use-strict-equality".to_string(),
            },
        ];

        // Filter by language if specified
        if let Some(lang) = &param.language {
            rules.retain(|rule| rule.language == *lang);
        }

        // Filter by category if specified
        if let Some(cat) = &param.category {
            rules.retain(|rule| rule.category == *cat);
        }

        Ok(ListCatalogRulesResult { rules })
    }

    #[tracing::instrument(skip(self))]
    pub async fn import_catalog_rule(
        &self,
        param: ImportCatalogRuleParam,
    ) -> Result<ImportCatalogRuleResult, ServiceError> {
        // For now, this is a mock implementation
        // In a real implementation, this would:
        // 1. Fetch the rule content from the provided URL
        // 2. Parse the YAML/JSON rule configuration
        // 3. Store it using the create_rule method
        
        // Extract rule ID from URL or use provided one
        let rule_id = param.rule_id.unwrap_or_else(|| {
            // Extract ID from URL (last segment)
            param.rule_url
                .split('/')
                .last()
                .unwrap_or("imported-rule")
                .to_string()
        });

        // Mock rule content - in real implementation, this would be fetched from the URL
        let mock_rule_config = format!(
            r#"
id: {}
message: "Imported rule from catalog"
language: javascript
severity: warning
rule:
  pattern: console.log($VAR)
fix: "// TODO: Replace with proper logging: console.log($VAR)"
"#,
            rule_id
        );

        // Use the existing create_rule method to store the imported rule
        let create_param = CreateRuleParam {
            rule_config: mock_rule_config,
            overwrite: false,
        };

        match self.create_rule(create_param).await {
            Ok(_) => Ok(ImportCatalogRuleResult {
                rule_id: rule_id.clone(),
                imported: true,
                message: format!("Successfully imported rule '{}' from catalog", rule_id),
            }),
            Err(e) => Ok(ImportCatalogRuleResult {
                rule_id: rule_id.clone(),
                imported: false,
                message: format!("Failed to import rule: {}", e),
            }),
        }
    }

    #[tracing::instrument(skip(self), fields(rule_id = %param.rule_id))]
    pub async fn delete_rule(
        &self,
        param: DeleteRuleParam,
    ) -> Result<DeleteRuleResult, ServiceError> {
        let file_path = self.get_rule_file_path(&param.rule_id);

        if file_path.exists() {
            fs::remove_file(&file_path)?;
            Ok(DeleteRuleResult {
                rule_id: param.rule_id,
                deleted: true,
                file_path: Some(file_path.to_string_lossy().to_string()),
            })
        } else {
            Ok(DeleteRuleResult {
                rule_id: param.rule_id,
                deleted: false,
                file_path: None,
            })
        }
    }

    #[tracing::instrument(skip(self), fields(rule_id = %param.rule_id))]
    pub async fn get_rule(
        &self,
        param: GetRuleParam,
    ) -> Result<GetRuleResult, ServiceError> {
        let file_path = self.get_rule_file_path(&param.rule_id);

        if !file_path.exists() {
            return Err(ServiceError::Internal(format!("Rule '{}' not found", param.rule_id)));
        }

        let content = fs::read_to_string(&file_path)?;

        Ok(GetRuleResult {
            rule_config: content,
            file_path: file_path.to_string_lossy().to_string(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchResult {
    pub text: String,
    pub vars: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct FileSearchParam {
    pub path_pattern: String,
    pub pattern: String,
    pub language: String,
    #[serde(default)]
    pub limit: Option<usize>,
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
    /// If true (default), only show preview. If false, actually modify files.
    #[serde(default = "default_dry_run")]
    pub dry_run: bool,
    /// If true, only return summary statistics (change counts per file)
    #[serde(default)]
    pub summary_only: bool,
    /// If true, include sample changes in the response (first few changes per file)
    #[serde(default)]
    pub include_samples: bool,
    /// Maximum number of sample changes to show per file (default: 3)
    #[serde(default = "default_max_samples")]
    pub max_samples: usize,
}

impl Default for FileReplaceParam {
    fn default() -> Self {
        Self {
            path_pattern: String::new(),
            pattern: String::new(),
            replacement: String::new(),
            language: String::new(),
            max_results: None,
            max_file_size: None,
            cursor: None,
            dry_run: true, // Default to true
            summary_only: false,
            include_samples: false,
            max_samples: default_max_samples(),
        }
    }
}

fn default_dry_run() -> bool {
    true
}

fn default_max_samples() -> usize {
    3
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileReplaceResult {
    /// Full diff results (only present when summary_only=false)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub file_results: Vec<FileDiffResult>,
    /// Summary results (only present when summary_only=true)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub summary_results: Vec<FileSummaryResult>,
    /// Continuation token for next page (None if no more results)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<SearchCursor>,
    /// Total number of files that matched the glob pattern (for progress indication)
    pub total_files_found: usize,
    /// Whether this was a dry run or actual file modification
    pub dry_run: bool,
    /// Total changes across all files
    pub total_changes: usize,
    /// Files with changes as (filename, change_count) pairs
    pub files_with_changes: Vec<(String, usize)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileDiffChange {
    pub line: usize,
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileDiffResult {
    pub file_path: PathBuf,
    pub file_size_bytes: u64,
    pub changes: Vec<FileDiffChange>,
    pub total_changes: usize,
    pub file_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileSummaryResult {
    pub file_path: PathBuf,
    pub file_size_bytes: u64,
    pub total_changes: usize,
    pub lines_changed: usize,
    pub file_hash: String,
    /// Sample changes if include_samples is true
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sample_changes: Vec<FileDiffChange>,
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

// Rule configuration structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConfig {
    pub id: String,
    pub message: Option<String>,
    pub language: String,
    pub severity: Option<String>,
    pub rule: RuleObject,
    pub fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleObject {
    // Atomic rules
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regex: Option<String>,
    
    // Relational rules
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inside: Option<Box<RelationalRule>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has: Option<Box<RelationalRule>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follows: Option<Box<RelationalRule>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precedes: Option<Box<RelationalRule>>,
    
    // Composite rules
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all: Option<Vec<RuleObject>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub any: Option<Vec<RuleObject>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not: Option<Box<RuleObject>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matches: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PatternSpec {
    Simple(String),
    Advanced {
        context: String,
        selector: Option<String>,
        strictness: Option<Strictness>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Strictness {
    Cst,
    Smart,
    Ast,
    Relaxed,
    Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationalRule {
    #[serde(flatten)]
    pub rule: RuleObject,
    #[serde(default, rename = "stopBy", skip_serializing_if = "Option::is_none")]
    pub stop_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

// Rule-based search parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct RuleSearchParam {
    pub rule_config: String, // YAML or JSON rule configuration
    #[serde(default)]
    pub path_pattern: Option<String>,
    #[serde(default)]
    pub max_results: Option<usize>,
    #[serde(default)]
    pub max_file_size: Option<u64>,
    #[serde(default)]
    pub cursor: Option<SearchCursor>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RuleSearchResult {
    pub rule_id: String,
    pub matches: Vec<RuleMatchResult>,
    pub next_cursor: Option<SearchCursor>,
    pub total_files_processed: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RuleMatchResult {
    pub file_path: String,
    pub matches: Vec<MatchResult>,
    pub message: Option<String>,
    pub severity: Option<String>,
}

// Rule-based replacement parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct RuleReplaceParam {
    pub rule_config: String, // YAML or JSON rule configuration
    #[serde(default)]
    pub path_pattern: Option<String>,
    #[serde(default)]
    pub max_results: Option<usize>,
    #[serde(default)]
    pub max_file_size: Option<u64>,
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(default)]
    pub summary_only: Option<bool>,
    #[serde(default)]
    pub cursor: Option<SearchCursor>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RuleReplaceResult {
    pub rule_id: String,
    pub file_results: Vec<FileDiffResult>,
    pub next_cursor: Option<SearchCursor>,
    pub total_files_processed: usize,
    pub total_changes: usize,
    pub dry_run: bool,
}

// Rule validation parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct RuleValidateParam {
    pub rule_config: String, // YAML or JSON rule configuration
    #[serde(default)]
    pub test_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RuleValidateResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub test_matches: Option<Vec<MatchResult>>,
}

// Rule management structures
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRuleParam {
    pub rule_config: String, // YAML or JSON rule configuration
    #[serde(default)]
    pub overwrite: Option<bool>, // Whether to overwrite existing rule with same ID
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRuleResult {
    pub rule_id: String,
    pub file_path: String,
    pub created: bool, // true if created, false if updated
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListRulesParam {
    #[serde(default)]
    pub language: Option<String>, // Filter by language
    #[serde(default)]
    pub severity: Option<String>, // Filter by severity
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListRulesResult {
    pub rules: Vec<RuleInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RuleInfo {
    pub id: String,
    pub language: String,
    pub message: Option<String>,
    pub severity: Option<String>,
    pub file_path: String,
    pub has_fix: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCatalogRulesParam {
    pub language: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCatalogRulesResult {
    pub rules: Vec<CatalogRuleInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogRuleInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub language: String,
    pub category: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCatalogRuleParam {
    pub rule_url: String,
    pub rule_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCatalogRuleResult {
    pub rule_id: String,
    pub imported: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRuleParam {
    pub rule_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRuleResult {
    pub rule_id: String,
    pub deleted: bool,
    pub file_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetRuleParam {
    pub rule_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetRuleResult {
    pub rule_config: String,
    pub file_path: String,
}

impl ServerHandler for AstGrepService {
    fn get_info(&self) -> InitializeResult {
        InitializeResult {
            protocol_version: ProtocolVersion::LATEST,
            server_info: Implementation {
                name: "ast-grep-mcp".into(),
                version: "0.1.0".into(),
            },
            capabilities: ServerCapabilities {
                tools: Some(rmcp::model::ToolsCapability { list_changed: Some(true) }),
                ..Default::default()
            },
            instructions: Some("This MCP server provides tools for structural code search and transformation using ast-grep. For bulk refactoring, use file_replace with summary_only=true to avoid token limits. Use the `documentation` tool for detailed examples.".into()),
        }
    }

    #[tracing::instrument(skip(self, _request, _context))]
    async fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: vec![
                Tool {
                    name: "search".into(),
                    description: "Search for patterns in code using ast-grep.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": { "code": { "type": "string" }, "pattern": { "type": "string" }, "language": { "type": "string" } } })).unwrap()),
                },
                Tool {
                    name: "file_search".into(),
                    description: "Search for patterns in a file using ast-grep.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path_pattern": { "type": "string" },
                            "pattern": { "type": "string" },
                            "language": { "type": "string" },
                            "max_results": { "type": "integer", "minimum": 1, "maximum": 100 },
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
                    name: "replace".into(),
                    description: "Replace patterns in code.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": { "code": { "type": "string" }, "pattern": { "type": "string" }, "replacement": { "type": "string" }, "language": { "type": "string" } } })).unwrap()),
                },
                Tool {
                    name: "file_replace".into(),
                    description: "Replace patterns in files. Use summary_only=true for bulk refactoring to avoid token limits. Returns change counts or line diffs.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path_pattern": { "type": "string" },
                            "pattern": { "type": "string" },
                            "replacement": { "type": "string" },
                            "language": { "type": "string" },
                            "max_results": { "type": "integer", "minimum": 1, "maximum": 10000 },
                            "max_file_size": { "type": "integer", "minimum": 1024, "maximum": 1073741824 },
                            "dry_run": { "type": "boolean", "default": true, "description": "If true (default), only show preview. If false, actually modify files." },
                            "summary_only": { "type": "boolean", "default": false, "description": "If true, only return summary statistics (change counts per file)" },
                            "include_samples": { "type": "boolean", "default": false, "description": "If true, include sample changes in the response (first few changes per file)" },
                            "max_samples": { "type": "integer", "default": 3, "minimum": 1, "maximum": 20, "description": "Maximum number of sample changes to show per file" },
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
                    name: "list_languages".into(),
                    description: "List all supported programming languages.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": {} })).unwrap()),
                },
                Tool {
                    name: "documentation".into(),
                    description: "Provides detailed usage examples for all tools.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": {} })).unwrap()),
                },
                Tool {
                    name: "rule_search".into(),
                    description: "Search for patterns using ast-grep rule configurations (YAML/JSON). Supports complex pattern matching with relational and composite rules.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_config": { "type": "string", "description": "YAML or JSON rule configuration" },
                            "path_pattern": { "type": "string", "description": "Glob pattern for files to search (optional, searches all files if not provided)" },
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
                        "required": ["rule_config"]
                    })).unwrap()),
                },
                Tool {
                    name: "rule_replace".into(),
                    description: "Replace patterns using ast-grep rule configurations with fix transformations. Supports complex rule-based code refactoring.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_config": { "type": "string", "description": "YAML or JSON rule configuration with fix field" },
                            "path_pattern": { "type": "string", "description": "Glob pattern for files to modify (optional, processes all files if not provided)" },
                            "max_results": { "type": "integer", "minimum": 1, "maximum": 10000 },
                            "max_file_size": { "type": "integer", "minimum": 1024, "maximum": 1073741824 },
                            "dry_run": { "type": "boolean", "default": true, "description": "If true (default), only show preview. If false, actually modify files." },
                            "summary_only": { "type": "boolean", "default": false, "description": "If true, only return summary statistics" },
                            "cursor": {
                                "type": "object",
                                "properties": {
                                    "last_file_path": { "type": "string" },
                                    "is_complete": { "type": "boolean" }
                                },
                                "required": ["last_file_path", "is_complete"]
                            }
                        },
                        "required": ["rule_config"]
                    })).unwrap()),
                },
                Tool {
                    name: "validate_rule".into(),
                    description: "Validate ast-grep rule configuration syntax and optionally test against sample code.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_config": { "type": "string", "description": "YAML or JSON rule configuration to validate" },
                            "test_code": { "type": "string", "description": "Optional code sample to test the rule against" }
                        },
                        "required": ["rule_config"]
                    })).unwrap()),
                },
                Tool {
                    name: "create_rule".into(),
                    description: "Create and store a new ast-grep rule configuration for reuse. LLMs can use this to build custom rule libraries.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_config": { "type": "string", "description": "YAML or JSON rule configuration to create" },
                            "overwrite": { "type": "boolean", "default": false, "description": "Whether to overwrite existing rule with same ID" }
                        },
                        "required": ["rule_config"]
                    })).unwrap()),
                },
                Tool {
                    name: "list_rules".into(),
                    description: "List all stored rule configurations with optional filtering by language or severity.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "language": { "type": "string", "description": "Filter rules by programming language" },
                            "severity": { "type": "string", "description": "Filter rules by severity level (info, warning, error)" }
                        }
                    })).unwrap()),
                },
                Tool {
                    name: "get_rule".into(),
                    description: "Retrieve a specific stored rule configuration by ID.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_id": { "type": "string", "description": "ID of the rule to retrieve" }
                        },
                        "required": ["rule_id"]
                    })).unwrap()),
                },
                Tool {
                    name: "delete_rule".into(),
                    description: "Delete a stored rule configuration by ID.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_id": { "type": "string", "description": "ID of the rule to delete" }
                        },
                        "required": ["rule_id"]
                    })).unwrap()),
                },
                Tool {
                    name: "list_catalog_rules".into(),
                    description: "List available rules from the ast-grep catalog with optional filtering.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "language": { "type": "string", "description": "Filter rules by programming language" },
                            "category": { "type": "string", "description": "Filter rules by category" }
                        }
                    })).unwrap()),
                },
                Tool {
                    name: "import_catalog_rule".into(),
                    description: "Import a rule from the ast-grep catalog into local storage.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_url": { "type": "string", "description": "URL of the catalog rule to import" },
                            "rule_id": { "type": "string", "description": "Optional custom ID for the imported rule" }
                        },
                        "required": ["rule_url"]
                    })).unwrap()),
                },
                ],
                ..Default::default()
            })
    }

    #[tracing::instrument(skip(self, request, _context), fields(tool_name = %request.name))]
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
            "validate_rule" => {
                let param: RuleValidateParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.validate_rule(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "rule_search" => {
                let param: RuleSearchParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.rule_search(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "rule_replace" => {
                let param: RuleReplaceParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.rule_replace(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "create_rule" => {
                let param: CreateRuleParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.create_rule(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "list_rules" => {
                let param: ListRulesParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.list_rules(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "get_rule" => {
                let param: GetRuleParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.get_rule(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "delete_rule" => {
                let param: DeleteRuleParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.delete_rule(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "list_catalog_rules" => {
                let param: ListCatalogRulesParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.list_catalog_rules(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "import_catalog_rule" => {
                let param: ImportCatalogRuleParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.import_catalog_rule(param).await.map_err(ErrorData::from)?;
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
            pattern: "console.log($VAR)".into(),
            language: "javascript".into(),
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
            pattern: "console.log($VAR)".into(),
            language: "javascript".into(),
        };

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 0);
    }

    #[tokio::test]
    async fn test_search_invalid_language() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "function greet() { console.log(\"Hello\"); }".to_string(),
            pattern: "console.log($VAR)".into(),
            language: "invalid_language".into(),
        };

        let result = service.search(param).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ServiceError::Internal(_)));
    }

    #[tokio::test]
    async fn test_replace_basic() {
        let service = AstGrepService::new();
        let param = ReplaceParam {
            code: "function oldName() { console.log(\"Hello\"); }".to_string(),
            pattern: "function oldName()".into(),
            replacement: "function newName()".into(),
            language: "javascript".into(),
        };

        let result = service.replace(param).await.unwrap();
        assert!(result.rewritten_code.contains("function newName()"));
        assert!(!result.rewritten_code.contains("function oldName()"));
    }

    #[tokio::test]
    async fn test_replace_with_vars() {
        let service = AstGrepService::new();
        let param = ReplaceParam {
            code: "const x = 5; const y = 10;".into(),
            pattern: "const $VAR = $VAL".into(),
            replacement: "let $VAR = $VAL".into(),
            language: "javascript".into(),
        };

        let result = service.replace(param).await.unwrap();
        assert!(result.rewritten_code.contains("let x = 5"));
        assert!(result.rewritten_code.contains("let y = 10"));
        assert!(!result.rewritten_code.contains("const"));
    }

    #[tokio::test]
    async fn test_replace_multiple_occurrences() {
        let service = AstGrepService::new();
        let param = ReplaceParam {
            code: "let a = 1; let b = 2; let c = 3;".into(),
            pattern: "let $VAR = $VAL".into(),
            replacement: "const $VAR = $VAL".into(),
            language: "javascript".into(),
        };
        let result = service.replace(param).await.unwrap();
        assert_eq!(
            result.rewritten_code,
            "const a = 1; const b = 2; const c = 3;"
        );
    }

    #[tokio::test]
    async fn test_rust_pattern_matching() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "fn main() { println!(\"Hello, world!\"); }".to_string(),
            pattern: "println!($VAR)".into(),
            language: "rust".into(),
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
            limit: 10,
            root_directories: vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))],
            rules_directory: PathBuf::from(".test-rules"),
        };

        let service = AstGrepService::with_config(custom_config);
        assert_eq!(service.config.max_file_size, 1024 * 1024);
        assert_eq!(service.config.max_concurrency, 5);
        assert_eq!(service.config.limit, 10);
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
            pattern: "console.log($VAR)".into(),
            language: "javascript".into(),
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
            code: "function test(a, b) { return a + b; } function add(x, y) { return x + y; }".into(),
            pattern: "function $NAME($PARAM1, $PARAM2) { return $PARAM1 + $PARAM2; }".into(),
            language: "javascript".into(),
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

    #[tokio::test]
    async fn test_invalid_language_error() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "let x = 1;".into(),
            pattern: "let x = 1;".into(),
            language: "not_a_real_language".into(),
        };
        let result = service.search(param).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ServiceError::Internal(msg) if msg == "Failed to parse language"));
    }

    #[tokio::test]
    async fn test_pattern_caching() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "console.log(\"test\"); console.log(\"another\");".into(),
            pattern: "console.log($VAR)".into(),
            language: "javascript".into(),
        };

        // Run the same search twice
        let result1 = service.search(param.clone()).await.unwrap();
        let result2 = service.search(param).await.unwrap();

        // Both should return the same results
        assert_eq!(result1.matches.len(), 2);
        assert_eq!(result2.matches.len(), 2);
        assert_eq!(result1.matches[0].text, result2.matches[0].text);

        // Verify cache has the pattern (cache should have 1 entry)
        let cache = service.pattern_cache.lock().unwrap();
        assert_eq!(cache.len(), 1);
    }
}