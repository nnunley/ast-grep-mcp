use crate::errors::ServiceError;
use crate::types::{ListCatalogRulesParam, ListCatalogRulesResult, CatalogRuleInfo, ImportCatalogRuleParam, ImportCatalogRuleResult};
use super::types::CreateRuleParam;
use super::storage::RuleStorage;

#[derive(Clone)]
pub struct CatalogManager {
    storage: RuleStorage,
}

impl CatalogManager {
    pub fn new(storage: RuleStorage) -> Self {
        Self { storage }
    }

    pub async fn list_catalog_rules(&self, param: ListCatalogRulesParam) -> Result<ListCatalogRulesResult, ServiceError> {
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
            CatalogRuleInfo {
                id: "no-var-declarations".to_string(),
                name: "No Var Declarations".to_string(),
                description: "Replace var with let or const".to_string(),
                language: "javascript".to_string(),
                category: "modernization".to_string(),
                url: "https://ast-grep.github.io/catalog/javascript/no-var-declarations".to_string(),
            },
            CatalogRuleInfo {
                id: "use-template-literals".to_string(),
                name: "Use Template Literals".to_string(),
                description: "Replace string concatenation with template literals".to_string(),
                language: "javascript".to_string(),
                category: "modernization".to_string(),
                url: "https://ast-grep.github.io/catalog/javascript/use-template-literals".to_string(),
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

    pub async fn import_catalog_rule(&self, param: ImportCatalogRuleParam) -> Result<ImportCatalogRuleResult, ServiceError> {
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

        // Mock rule content based on common patterns
        // In real implementation, this would be fetched from the URL
        let mock_rule_config = self.create_mock_rule_config(&rule_id, &param.rule_url);

        // Use the existing create_rule method to store the imported rule
        let create_param = CreateRuleParam {
            rule_config: mock_rule_config,
            overwrite: false,
        };

        match self.storage.create_rule(create_param).await {
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

    fn create_mock_rule_config(&self, rule_id: &str, _url: &str) -> String {
        // Create different mock rules based on the rule ID
        match rule_id {
            "no-console-log" => {
                format!(
                    r#"
id: {}
message: "Avoid using console.log in production code"
language: javascript
severity: warning
rule:
  pattern: console.log($ARGS)
fix: "// TODO: Replace with proper logging"
"#,
                    rule_id
                )
            }
            "use-strict-equality" => {
                format!(
                    r#"
id: {}
message: "Use strict equality (===) instead of loose equality (==)"
language: javascript
severity: warning
rule:
  pattern: $A == $B
fix: "$A === $B"
"#,
                    rule_id
                )
            }
            "no-var-declarations" => {
                format!(
                    r#"
id: {}
message: "Use let or const instead of var"
language: javascript
severity: warning
rule:
  pattern: var $VAR = $VALUE;
fix: "let $VAR = $VALUE;"
"#,
                    rule_id
                )
            }
            _ => {
                format!(
                    r#"
id: {}
message: "Imported rule from catalog"
language: javascript
severity: info
rule:
  pattern: console.log($VAR)
fix: "// TODO: Replace with proper logging: console.log($VAR)"
"#,
                    rule_id
                )
            }
        }
    }
}