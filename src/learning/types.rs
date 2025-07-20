//! Learning system types and structures

use serde::{Deserialize, Serialize};

/// Enhanced error response with learning guidance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedError {
    pub error: String,
    pub learning_hint: Option<LearningHint>,
    pub suggested_fixes: Vec<PatternFix>,
    pub related_examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningHint {
    pub category: String,
    pub explanation: String,
    pub guidance: String,
    pub difficulty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternFix {
    pub description: String,
    pub fixed_pattern: String,
    pub confidence: f32,
    pub rationale: String,
}

/// Parameters for pattern validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatePatternParam {
    pub pattern: String,
    pub language: String,
    pub test_code: Option<String>,
    pub context: Option<String>,
}

/// Result of pattern validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub match_result: Option<crate::types::MatchResult>,
    pub analysis: PatternAnalysis,
    pub learning_insights: Vec<LearningInsight>,
    pub suggested_experiments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternAnalysis {
    pub complexity_score: f32,
    pub language_compatibility: Vec<String>,
    pub metavar_usage: Vec<MetavarInfo>,
    pub potential_issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetavarInfo {
    pub name: String,
    pub capture_type: String,
    pub usage_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningInsight {
    pub category: String,
    pub insight: String,
    pub actionable_tip: String,
}

/// Parameters for pattern exploration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorePatternParam {
    pub language: Option<String>,
    pub category: Option<String>,
    pub complexity: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
}

/// Pattern catalog response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternCatalog {
    pub patterns: Vec<CatalogPattern>,
    pub total_available: u32,
    pub learning_path: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogPattern {
    pub id: String,
    pub pattern: String,
    pub description: String,
    pub language: String,
    pub examples: Vec<String>,
    pub difficulty: String,
    pub category: String,
    pub tags: Vec<String>,
    pub prerequisites: Vec<String>,
    pub related_patterns: Vec<String>,
    pub learning_notes: String,
}
