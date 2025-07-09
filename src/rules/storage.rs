use super::parser::parse_rule_config;
use super::types::{
    CreateRuleParam, CreateRuleResult, DeleteRuleParam, DeleteRuleResult, GetRuleParam,
    GetRuleResult, ListRulesParam, ListRulesResult, RuleConfig, RuleInfo,
};
use crate::errors::ServiceError;
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct RuleStorage {
    rule_directories: Vec<PathBuf>,
}

impl RuleStorage {
    pub fn new(rules_directory: PathBuf) -> Self {
        Self {
            rule_directories: vec![rules_directory],
        }
    }

    pub fn with_directories(directories: Vec<PathBuf>) -> Self {
        Self {
            rule_directories: directories,
        }
    }

    pub async fn create_rule(
        &self,
        param: CreateRuleParam,
    ) -> Result<CreateRuleResult, ServiceError> {
        // Parse and validate the rule config
        let rule = parse_rule_config(&param.rule_config)?;

        // Use the first directory for creating new rules
        let primary_dir = self
            .rule_directories
            .first()
            .ok_or_else(|| ServiceError::Internal("No rule directories configured".to_string()))?;

        // Ensure rules directory exists
        fs::create_dir_all(primary_dir)?;

        let file_path = primary_dir.join(format!("{}.yaml", rule.id));

        // Check if file already exists and overwrite is false
        if file_path.exists() && !param.overwrite {
            return Err(ServiceError::Internal(format!(
                "Rule '{}' already exists. Use overwrite=true to replace it.",
                rule.id
            )));
        }

        // Write the rule to file
        fs::write(&file_path, &param.rule_config)?;

        Ok(CreateRuleResult {
            rule_id: rule.id,
            created: true,
            file_path: file_path.to_string_lossy().to_string(),
        })
    }

    pub async fn list_rules(&self, param: ListRulesParam) -> Result<ListRulesResult, ServiceError> {
        let mut rules = Vec::new();
        let mut seen_rule_ids = std::collections::HashMap::new();

        // Search in all rule directories
        for directory in &self.rule_directories {
            if !directory.exists() {
                continue;
            }

            // Read all .yaml files in the rules directory
            for entry in fs::read_dir(directory)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                    || path.extension().and_then(|s| s.to_str()) == Some("yml")
                {
                    match self.load_rule_from_file(&path) {
                        Ok(rule) => {
                            // Apply filters
                            let mut include = true;

                            if let Some(ref lang) = param.language {
                                if rule.language != *lang {
                                    include = false;
                                }
                            }

                            if let Some(ref severity) = param.severity {
                                if rule.severity.as_ref() != Some(severity) {
                                    include = false;
                                }
                            }

                            // NOTE: We currently deduplicate rules by ID (first wins)
                            // This differs from ast-grep CLI which loads ALL rules including duplicates
                            if include {
                                if let Some(first_path) = seen_rule_ids.get(&rule.id) {
                                    // Emit warning for duplicate rule ID
                                    eprintln!(
                                        "Warning: Duplicate rule ID '{}' found in:\n  \
                                     - Current: {}\n  \
                                     - First loaded from: {}\n  \
                                     The rule from the current file will be ignored.",
                                        rule.id,
                                        path.display(),
                                        first_path
                                    );
                                } else {
                                    seen_rule_ids.insert(
                                        rule.id.clone(),
                                        path.to_string_lossy().to_string(),
                                    );
                                    rules.push(RuleInfo {
                                        id: rule.id,
                                        message: rule.message,
                                        language: rule.language,
                                        severity: rule.severity,
                                        file_path: path.to_string_lossy().to_string(),
                                        has_fix: rule.fix.is_some(),
                                    });
                                }
                            }
                        }
                        Err(_) => {
                            // Skip invalid rule files
                            continue;
                        }
                    }
                }
            }
        }

        Ok(ListRulesResult { rules })
    }

    pub async fn get_rule(&self, param: GetRuleParam) -> Result<GetRuleResult, ServiceError> {
        // Search for the rule in all directories
        for directory in &self.rule_directories {
            let file_path = directory.join(format!("{}.yaml", param.rule_id));

            if file_path.exists() {
                let rule = self.load_rule_from_file(&file_path)?;

                return Ok(GetRuleResult {
                    rule_config: rule,
                    file_path: file_path.to_string_lossy().to_string(),
                });
            }
        }

        // Rule not found in any directory
        Err(ServiceError::Internal(format!(
            "Rule '{}' not found",
            param.rule_id
        )))
    }

    pub async fn delete_rule(
        &self,
        param: DeleteRuleParam,
    ) -> Result<DeleteRuleResult, ServiceError> {
        // Search for the rule in all directories
        for directory in &self.rule_directories {
            let file_path = directory.join(format!("{}.yaml", param.rule_id));

            if file_path.exists() {
                fs::remove_file(&file_path)?;
                return Ok(DeleteRuleResult {
                    rule_id: param.rule_id,
                    deleted: true,
                    message: "Rule deleted successfully".to_string(),
                });
            }
        }

        Ok(DeleteRuleResult {
            rule_id: param.rule_id,
            deleted: false,
            message: "Rule not found".to_string(),
        })
    }

    fn load_rule_from_file(&self, path: &PathBuf) -> Result<RuleConfig, ServiceError> {
        let content = fs::read_to_string(path)?;
        parse_rule_config(&content)
    }
}
