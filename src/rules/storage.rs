use crate::errors::ServiceError;
use crate::config::ServiceConfig;
use super::types::{
    RuleConfig, CreateRuleParam, CreateRuleResult, ListRulesParam, ListRulesResult,
    GetRuleParam, GetRuleResult, DeleteRuleParam, DeleteRuleResult, RuleInfo
};
use super::parser::parse_rule_config;
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct RuleStorage {
    config: ServiceConfig,
}

impl RuleStorage {
    pub fn new(config: ServiceConfig) -> Self {
        Self { config }
    }

    pub async fn create_rule(&self, param: CreateRuleParam) -> Result<CreateRuleResult, ServiceError> {
        // Parse and validate the rule config
        let rule = parse_rule_config(&param.rule_config)?;
        
        // Ensure rules directory exists
        fs::create_dir_all(&self.config.rules_directory)?;
        
        let file_path = self.get_rule_file_path(&rule.id);
        
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
        if !self.config.rules_directory.exists() {
            return Ok(ListRulesResult { rules: vec![] });
        }

        let mut rules = Vec::new();

        // Read all .yaml files in the rules directory
        for entry in fs::read_dir(&self.config.rules_directory)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") ||
               path.extension().and_then(|s| s.to_str()) == Some("yml") {
                
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
                        
                        if include {
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
                    Err(_) => {
                        // Skip invalid rule files
                        continue;
                    }
                }
            }
        }

        Ok(ListRulesResult { rules })
    }

    pub async fn get_rule(&self, param: GetRuleParam) -> Result<GetRuleResult, ServiceError> {
        let file_path = self.get_rule_file_path(&param.rule_id);
        
        if !file_path.exists() {
            return Err(ServiceError::FileNotFound(file_path));
        }
        
        let rule = self.load_rule_from_file(&file_path)?;
        
        Ok(GetRuleResult {
            rule_config: rule,
            file_path: file_path.to_string_lossy().to_string(),
        })
    }

    pub async fn delete_rule(&self, param: DeleteRuleParam) -> Result<DeleteRuleResult, ServiceError> {
        let file_path = self.get_rule_file_path(&param.rule_id);
        
        if file_path.exists() {
            fs::remove_file(&file_path)?;
            Ok(DeleteRuleResult {
                rule_id: param.rule_id,
                deleted: true,
                message: "Rule deleted successfully".to_string(),
            })
        } else {
            Ok(DeleteRuleResult {
                rule_id: param.rule_id,
                deleted: false,
                message: "Rule not found".to_string(),
            })
        }
    }

    fn get_rule_file_path(&self, rule_id: &str) -> PathBuf {
        self.config.rules_directory.join(format!("{}.yaml", rule_id))
    }

    fn load_rule_from_file(&self, path: &PathBuf) -> Result<RuleConfig, ServiceError> {
        let content = fs::read_to_string(path)?;
        parse_rule_config(&content)
    }
}