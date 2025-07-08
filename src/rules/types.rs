use crate::types::CursorParam;
use serde::{Deserialize, Serialize};

// Rule configuration types
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
    pub pattern: Option<PatternSpec>,
    pub kind: Option<String>,
    pub regex: Option<String>,
    pub inside: Option<Box<RuleObject>>,
    pub has: Option<Box<RuleObject>>,
    pub follows: Option<Box<RuleObject>>,
    pub precedes: Option<Box<RuleObject>>,
    pub all: Option<Vec<RuleObject>>,
    pub any: Option<Vec<RuleObject>>,
    pub not: Option<Box<RuleObject>>,
    pub matches: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PatternSpec {
    Simple(String),
    Advanced {
        context: String,
        selector: Option<String>,
        strictness: Option<String>,
    },
}

// Rule operation parameters and results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSearchParam {
    pub rule_config: String,
    pub path_pattern: Option<String>,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    pub cursor: Option<CursorParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleReplaceParam {
    pub rule_config: String,
    pub path_pattern: Option<String>,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    #[serde(default = "default_true")]
    pub dry_run: bool,
    #[serde(default = "default_false")]
    pub summary_only: bool,
    pub cursor: Option<CursorParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleValidateParam {
    pub rule_config: String,
    pub test_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RuleValidateResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub test_results: Option<RuleTestResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RuleTestResult {
    pub matches_found: usize,
    pub sample_matches: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRuleParam {
    pub rule_config: String,
    #[serde(default = "default_false")]
    pub overwrite: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRuleResult {
    pub rule_id: String,
    pub created: bool,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListRulesParam {
    pub language: Option<String>,
    pub severity: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListRulesResult {
    pub rules: Vec<RuleInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RuleInfo {
    pub id: String,
    pub message: Option<String>,
    pub language: String,
    pub severity: Option<String>,
    pub file_path: String,
    pub has_fix: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRuleParam {
    pub rule_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetRuleResult {
    pub rule_config: RuleConfig,
    pub file_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRuleParam {
    pub rule_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRuleResult {
    pub rule_id: String,
    pub deleted: bool,
    pub message: String,
}

// Default functions for serde
fn default_max_results() -> usize {
    10000
}
fn default_max_file_size() -> u64 {
    50 * 1024 * 1024
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}

// Relational rule type for inside, has, follows, precedes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationalRule {
    pub pattern: Option<PatternSpec>,
    pub kind: Option<String>,
    pub regex: Option<String>,
    pub inside: Option<Box<RelationalRule>>,
    pub has: Option<Box<RelationalRule>>,
    pub follows: Option<Box<RelationalRule>>,
    pub precedes: Option<Box<RelationalRule>>,
    pub all: Option<Vec<RelationalRule>>,
    pub any: Option<Vec<RelationalRule>>,
    pub not: Option<Box<RelationalRule>>,
    pub matches: Option<String>,
}
