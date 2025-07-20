//! Pattern discovery and exploration
#![allow(clippy::unnecessary_map_or)] // is_none_or requires Rust 1.81+ using external data files

use super::types::*;
use crate::errors::ServiceError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguagePatternData {
    pub language: String,
    pub patterns: Vec<JsonPattern>,
    pub learning_progressions: Vec<LearningProgression>,
    pub categories: HashMap<String, PatternCategory>,
}

/// Pattern structure for JSON loading (without language field)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonPattern {
    pub id: String,
    pub pattern: String,
    pub description: String,
    pub examples: Vec<String>,
    pub difficulty: String,
    pub category: String,
    pub tags: Vec<String>,
    pub prerequisites: Vec<String>,
    pub related_patterns: Vec<String>,
    pub learning_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningProgression {
    pub name: String,
    pub title: String,
    pub description: String,
    pub levels: Vec<ProgressionLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressionLevel {
    pub level: u32,
    pub title: String,
    pub patterns: Vec<String>,
    pub skills: Vec<String>,
    pub learning_objectives: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternCategory {
    pub name: String,
    pub description: String,
    pub patterns: Vec<String>,
}

#[derive(Clone)]
pub struct DiscoveryService {
    patterns: Vec<CatalogPattern>,
    patterns_by_language: HashMap<String, Vec<CatalogPattern>>,
    progressions: Vec<LearningProgression>,
    categories: HashMap<String, PatternCategory>,
}

impl DiscoveryService {
    pub fn new() -> Result<Self, ServiceError> {
        let mut service = Self {
            patterns: Vec::new(),
            patterns_by_language: HashMap::new(),
            progressions: Vec::new(),
            categories: HashMap::new(),
        };

        service.load_pattern_data()?;
        Ok(service)
    }

    fn load_pattern_data(&mut self) -> Result<(), ServiceError> {
        // Load JavaScript patterns
        if let Ok(js_data) = self.load_language_patterns("javascript") {
            self.add_language_data(js_data);
        }

        // Load Rust patterns
        if let Ok(rust_data) = self.load_language_patterns("rust") {
            self.add_language_data(rust_data);
        }

        // Load Python patterns
        if let Ok(python_data) = self.load_language_patterns("python") {
            self.add_language_data(python_data);
        }

        Ok(())
    }

    fn load_language_patterns(&self, language: &str) -> Result<LanguagePatternData, ServiceError> {
        let data = match language {
            "javascript" => include_str!("../data/patterns/javascript.json"),
            "rust" => include_str!("../data/patterns/rust.json"),
            "python" => include_str!("../data/patterns/python.json"),
            _ => {
                return Err(ServiceError::Internal(format!(
                    "Unsupported language: {language}"
                )));
            }
        };

        serde_json::from_str(data).map_err(|e| {
            ServiceError::ParserError(format!("Failed to parse {language} patterns: {e}"))
        })
    }

    fn add_language_data(&mut self, data: LanguagePatternData) {
        let language = data.language.clone();

        // Convert JsonPattern to CatalogPattern by adding language field
        let patterns_with_language: Vec<CatalogPattern> = data
            .patterns
            .into_iter()
            .map(|json_pattern| CatalogPattern {
                id: json_pattern.id,
                pattern: json_pattern.pattern,
                description: json_pattern.description,
                language: language.clone(),
                examples: json_pattern.examples,
                difficulty: json_pattern.difficulty,
                category: json_pattern.category,
                tags: json_pattern.tags,
                prerequisites: json_pattern.prerequisites,
                related_patterns: json_pattern.related_patterns,
                learning_notes: json_pattern.learning_notes,
            })
            .collect();

        // Add patterns to global list and language-specific list
        self.patterns.extend(patterns_with_language.clone());
        self.patterns_by_language
            .insert(language.clone(), patterns_with_language);

        // Add progressions (with language prefix to avoid conflicts)
        for mut progression in data.learning_progressions {
            progression.name = format!("{}_{}", language, progression.name);
            self.progressions.push(progression);
        }

        // Add categories (with language prefix)
        for (key, mut category) in data.categories {
            let prefixed_key = format!("{language}_{key}");
            category.name = format!("{} ({})", category.name, language.to_uppercase());
            self.categories.insert(prefixed_key, category);
        }
    }

    pub async fn explore_patterns(
        &self,
        param: ExplorePatternParam,
    ) -> Result<PatternCatalog, ServiceError> {
        let mut filtered_patterns = self.patterns.clone();

        // Apply language filter
        if let Some(language) = &param.language {
            filtered_patterns.retain(|p| p.language == *language);
        }

        // Apply category filter
        if let Some(category) = &param.category {
            // Try exact match first, then language-prefixed match
            let category_patterns = if let Some(cat_data) = self.categories.get(category) {
                &cat_data.patterns
            } else if let Some(lang) = &param.language {
                let prefixed_key = format!("{lang}_{category}");
                if let Some(cat_data) = self.categories.get(&prefixed_key) {
                    &cat_data.patterns
                } else {
                    // Fallback: filter by category field in patterns
                    filtered_patterns.retain(|p| p.category == *category);
                    return self.build_catalog(filtered_patterns, &param);
                }
            } else {
                // No language specified, filter by category field
                filtered_patterns.retain(|p| p.category == *category);
                return self.build_catalog(filtered_patterns, &param);
            };

            // Filter patterns by category pattern IDs
            filtered_patterns.retain(|p| category_patterns.contains(&p.id));
        }

        // Apply complexity filter
        if let Some(complexity) = &param.complexity {
            filtered_patterns.retain(|p| p.difficulty == *complexity);
        }

        // Apply search filter
        if let Some(search) = &param.search {
            let search_lower = search.to_lowercase();
            filtered_patterns.retain(|p| {
                p.pattern.to_lowercase().contains(&search_lower)
                    || p.description.to_lowercase().contains(&search_lower)
                    || p.tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&search_lower))
            });
        }

        self.build_catalog(filtered_patterns, &param)
    }

    fn build_catalog(
        &self,
        mut patterns: Vec<CatalogPattern>,
        param: &ExplorePatternParam,
    ) -> Result<PatternCatalog, ServiceError> {
        // Sort patterns by difficulty and then by language
        patterns.sort_by(|a, b| {
            let difficulty_order = |d: &str| match d {
                "beginner" => 0,
                "intermediate" => 1,
                "advanced" => 2,
                _ => 3,
            };

            difficulty_order(&a.difficulty)
                .cmp(&difficulty_order(&b.difficulty))
                .then(a.language.cmp(&b.language))
                .then(a.id.cmp(&b.id))
        });

        // Apply limit
        let limit = param.limit.unwrap_or(20) as usize;
        let total_available = patterns.len();
        patterns.truncate(limit);

        Ok(PatternCatalog {
            patterns,
            total_available: total_available as u32,
            learning_path: self.generate_learning_path(param),
        })
    }

    fn generate_learning_path(&self, param: &ExplorePatternParam) -> Vec<String> {
        if let Some(language) = &param.language {
            // Find language-specific progression
            let progression_name = format!("{language}_basics");
            if let Some(progression) = self
                .progressions
                .iter()
                .find(|p| p.name.contains(&progression_name))
            {
                return progression
                    .levels
                    .iter()
                    .map(|level| format!("Level {}: {}", level.level, level.title))
                    .collect();
            }

            // Fallback to generic language path
            match language.as_str() {
                "javascript" => vec![
                    "Start with variables: const $VAR = $VALUE".to_string(),
                    "Learn functions: function $NAME() { $$$ }".to_string(),
                    "Practice with console.log: console.log($$$)".to_string(),
                    "Try conditionals: if ($COND) { $$$ }".to_string(),
                    "Explore arrow functions: ($$$PARAMS) => $EXPR".to_string(),
                ],
                "rust" => vec![
                    "Start with variables: let $VAR = $VALUE;".to_string(),
                    "Learn printing: println!($$$)".to_string(),
                    "Try functions: fn $NAME() -> $TYPE { $$$ }".to_string(),
                    "Explore structs: struct $NAME { $$$ }".to_string(),
                    "Practice pattern matching: match $EXPR { $$$ }".to_string(),
                ],
                "python" => vec![
                    "Start with variables: $VAR = $VALUE".to_string(),
                    "Learn functions: def $NAME($$$PARAMS): $$$".to_string(),
                    "Try print statements: print($$$)".to_string(),
                    "Explore classes: class $NAME: $$$".to_string(),
                    "Practice loops: for $VAR in $ITERABLE: $$$".to_string(),
                ],
                _ => vec![
                    "Choose a programming language to start".to_string(),
                    "Begin with simple variable patterns".to_string(),
                    "Practice with function patterns".to_string(),
                    "Explore language-specific constructs".to_string(),
                ],
            }
        } else {
            vec![
                "Choose a programming language (javascript, rust, python)".to_string(),
                "Start with beginner patterns for basic syntax".to_string(),
                "Practice with intermediate patterns".to_string(),
                "Challenge yourself with advanced patterns".to_string(),
                "Explore different categories (functions, variables, etc.)".to_string(),
            ]
        }
    }

    /// Get all available languages
    pub fn get_languages(&self) -> Vec<String> {
        self.patterns_by_language.keys().cloned().collect()
    }

    /// Get all available categories for a language
    pub fn get_categories(&self, language: Option<&str>) -> Vec<String> {
        if let Some(lang) = language {
            self.categories
                .keys()
                .filter(|k| k.starts_with(&format!("{lang}_")))
                .map(|k| k.strip_prefix(&format!("{lang}_")).unwrap_or(k).to_string())
                .collect()
        } else {
            // Return unique category names without language prefixes
            let mut categories: Vec<String> = self
                .categories
                .keys()
                .map(|k| {
                    if let Some(pos) = k.find('_') {
                        k[pos + 1..].to_string()
                    } else {
                        k.clone()
                    }
                })
                .collect();
            categories.sort();
            categories.dedup();
            categories
        }
    }

    /// Get patterns by specific criteria for advanced filtering
    pub fn get_patterns_by_criteria(
        &self,
        language: Option<&str>,
        category: Option<&str>,
        difficulty: Option<&str>,
        tags: Option<&[String]>,
    ) -> Vec<&CatalogPattern> {
        self.patterns
            .iter()
            .filter(|p| language.map_or(true, |l| p.language == l))
            .filter(|p| category.map_or(true, |c| p.category == c))
            .filter(|p| difficulty.map_or(true, |d| p.difficulty == d))
            .filter(|p| tags.map_or(true, |t| t.iter().any(|tag| p.tags.contains(tag))))
            .collect()
    }

    /// Get learning progression for a specific language
    pub fn get_learning_progression(&self, language: &str) -> Option<&LearningProgression> {
        let progression_name = format!("{language}_basics");
        self.progressions
            .iter()
            .find(|p| p.name.contains(&progression_name))
    }
}
