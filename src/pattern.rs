use crate::errors::ServiceError;
use crate::types::MatchResult;
use ast_grep_core::{AstGrep, Pattern};
use ast_grep_language::SupportLang as Language;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct PatternMatcher {
    pattern_cache: Arc<Mutex<HashMap<String, Pattern>>>,
}

impl PatternMatcher {
    pub fn new() -> Self {
        Self {
            pattern_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_cache(cache: Arc<Mutex<HashMap<String, Pattern>>>) -> Self {
        Self {
            pattern_cache: cache,
        }
    }

    pub fn get_cache(&self) -> Arc<Mutex<HashMap<String, Pattern>>> {
        self.pattern_cache.clone()
    }

    pub fn search(&self, code: &str, pattern: &str, lang: Language) -> Result<Vec<MatchResult>, ServiceError> {
        let ast = AstGrep::new(code, lang);
        let pattern = self.get_or_create_pattern(pattern, lang)?;

        let matches: Vec<MatchResult> = ast
            .root()
            .find_all(pattern)
            .map(|node| {
                let vars: HashMap<String, String> = node.get_env().clone().into();
                let range = node.range();
                let start_pos = range.start;
                let end_pos = range.end;

                MatchResult {
                    text: node.text().to_string(),
                    start_line: start_pos.row + 1,
                    end_line: end_pos.row + 1,
                    start_col: start_pos.column,
                    end_col: end_pos.column,
                    vars,
                }
            })
            .collect();

        Ok(matches)
    }

    pub fn replace(&self, code: &str, pattern: &str, replacement: &str, lang: Language) -> Result<String, ServiceError> {
        let ast = AstGrep::new(code, lang);
        let pattern = self.get_or_create_pattern(pattern, lang)?;

        // Apply replacements
        let result = ast.root().replace_all(pattern, replacement);
        Ok(result)
    }

    fn get_or_create_pattern(&self, pattern_str: &str, lang: Language) -> Result<Pattern, ServiceError> {
        let cache_key = format!("{}:{}", lang.to_string(), pattern_str);
        
        // Try to get from cache first
        {
            let cache = self.pattern_cache.lock().unwrap();
            if let Some(pattern) = cache.get(&cache_key) {
                return Ok(pattern.clone());
            }
        }

        // Create new pattern
        let pattern = Pattern::new(pattern_str, lang)
            .map_err(|_| ServiceError::ParserError("Failed to parse pattern".to_string()))?;

        // Store in cache
        {
            let mut cache = self.pattern_cache.lock().unwrap();
            cache.insert(cache_key, pattern.clone());
        }

        Ok(pattern)
    }
}