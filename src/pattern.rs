use crate::errors::ServiceError;
use crate::types::MatchResult;
use ast_grep_core::{AstGrep, Pattern};
use ast_grep_language::SupportLang as Language;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct PatternMatcher {
    pattern_cache: Arc<Mutex<LruCache<String, Pattern>>>,
}

impl Default for PatternMatcher {
    fn default() -> Self {
        let cache_size = NonZeroUsize::new(1000).unwrap(); // Default cache size
        Self {
            pattern_cache: Arc::new(Mutex::new(LruCache::new(cache_size))),
        }
    }
}

impl PatternMatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_cache(cache: Arc<Mutex<LruCache<String, Pattern>>>) -> Self {
        Self {
            pattern_cache: cache,
        }
    }

    pub fn get_cache(&self) -> Arc<Mutex<LruCache<String, Pattern>>> {
        self.pattern_cache.clone()
    }

    pub fn search(
        &self,
        code: &str,
        pattern: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        let ast = AstGrep::new(code, lang);
        let pattern = self.get_or_create_pattern(pattern, lang)?;

        let matches: Vec<MatchResult> = ast
            .root()
            .find_all(pattern)
            .map(|node| MatchResult::from_node_match(&node))
            .collect();

        Ok(matches)
    }

    pub fn replace(
        &self,
        code: &str,
        pattern: &str,
        replacement: &str,
        lang: Language,
    ) -> Result<String, ServiceError> {
        let ast = AstGrep::new(code, lang);
        let pattern = self.get_or_create_pattern(pattern, lang)?;

        // Apply replacements
        let edits = ast.root().replace_all(pattern, replacement);
        let mut result = code.to_string();

        // Apply edits in reverse order to maintain correct offsets
        for edit in edits.into_iter().rev() {
            let start = edit.position;
            let end = start + edit.deleted_length;
            result.replace_range(
                start..end,
                std::str::from_utf8(&edit.inserted_text).unwrap(),
            );
        }

        Ok(result)
    }

    fn get_or_create_pattern(
        &self,
        pattern_str: &str,
        lang: Language,
    ) -> Result<Pattern, ServiceError> {
        let cache_key = format!("{}:{}", lang, pattern_str);

        // Try to get from cache first
        {
            let mut cache = self.pattern_cache.lock().unwrap();
            if let Some(pattern) = cache.get(&cache_key) {
                return Ok(pattern.clone());
            }
        }

        // Create new pattern
        let pattern = Pattern::new(pattern_str, lang);

        // Store in cache
        {
            let mut cache = self.pattern_cache.lock().unwrap();
            cache.put(cache_key, pattern.clone());
        }

        Ok(pattern)
    }
}
