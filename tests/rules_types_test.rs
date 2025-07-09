use ast_grep_mcp::rules::types::*;
use ast_grep_mcp::types::CursorParam;

#[test]
fn test_rule_config_serialization() {
    let rule_config = RuleConfig {
        id: "test-rule".to_string(),
        message: Some("Test message".to_string()),
        language: "javascript".to_string(),
        severity: Some("warning".to_string()),
        rule: RuleObject {
            pattern: Some(PatternSpec::Simple("console.log($VAR)".to_string())),
            kind: None,
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: None,
            any: None,
            not: None,
            matches: None,
        },
        fix: Some("logger.info($VAR)".to_string()),
    };

    let json = serde_json::to_string(&rule_config).unwrap();
    let deserialized: RuleConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(rule_config.id, deserialized.id);
    assert_eq!(rule_config.message, deserialized.message);
    assert_eq!(rule_config.language, deserialized.language);
    assert_eq!(rule_config.severity, deserialized.severity);
    assert_eq!(rule_config.fix, deserialized.fix);
}

#[test]
fn test_rule_config_minimal() {
    let rule_config = RuleConfig {
        id: "minimal".to_string(),
        message: None,
        language: "rust".to_string(),
        severity: None,
        rule: RuleObject {
            pattern: Some(PatternSpec::Simple("fn $NAME()".to_string())),
            kind: None,
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: None,
            any: None,
            not: None,
            matches: None,
        },
        fix: None,
    };

    let json = serde_json::to_string(&rule_config).unwrap();
    let deserialized: RuleConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(rule_config.id, deserialized.id);
    assert_eq!(rule_config.message, None);
    assert_eq!(rule_config.severity, None);
    assert_eq!(rule_config.fix, None);
}

#[test]
fn test_pattern_spec_simple() {
    let pattern = PatternSpec::Simple("test pattern".to_string());
    let json = serde_json::to_string(&pattern).unwrap();
    let deserialized: PatternSpec = serde_json::from_str(&json).unwrap();

    match deserialized {
        PatternSpec::Simple(s) => assert_eq!(s, "test pattern"),
        _ => panic!("Expected Simple pattern"),
    }
}

#[test]
fn test_pattern_spec_advanced() {
    let pattern = PatternSpec::Advanced {
        context: "function $NAME($ARGS) { $BODY }".to_string(),
        selector: Some("$NAME".to_string()),
        strictness: Some("relaxed".to_string()),
    };

    let json = serde_json::to_string(&pattern).unwrap();
    let deserialized: PatternSpec = serde_json::from_str(&json).unwrap();

    match deserialized {
        PatternSpec::Advanced {
            context,
            selector,
            strictness,
        } => {
            assert_eq!(context, "function $NAME($ARGS) { $BODY }");
            assert_eq!(selector, Some("$NAME".to_string()));
            assert_eq!(strictness, Some("relaxed".to_string()));
        }
        _ => panic!("Expected Advanced pattern"),
    }
}

#[test]
fn test_pattern_spec_advanced_minimal() {
    let pattern = PatternSpec::Advanced {
        context: "if ($COND) { $BODY }".to_string(),
        selector: None,
        strictness: None,
    };

    let json = serde_json::to_string(&pattern).unwrap();
    let deserialized: PatternSpec = serde_json::from_str(&json).unwrap();

    match deserialized {
        PatternSpec::Advanced {
            context,
            selector,
            strictness,
        } => {
            assert_eq!(context, "if ($COND) { $BODY }");
            assert_eq!(selector, None);
            assert_eq!(strictness, None);
        }
        _ => panic!("Expected Advanced pattern"),
    }
}

#[test]
fn test_rule_object_with_kind() {
    let rule = RuleObject {
        pattern: None,
        kind: Some("function_declaration".to_string()),
        regex: None,
        inside: None,
        has: None,
        follows: None,
        precedes: None,
        all: None,
        any: None,
        not: None,
        matches: None,
    };

    let json = serde_json::to_string(&rule).unwrap();
    let deserialized: RuleObject = serde_json::from_str(&json).unwrap();

    assert_eq!(rule.kind, deserialized.kind);
    assert_eq!(rule.pattern.is_none(), deserialized.pattern.is_none());
}

#[test]
fn test_rule_object_with_regex() {
    let rule = RuleObject {
        pattern: None,
        kind: None,
        regex: Some(r"console\.(log|error)".to_string()),
        inside: None,
        has: None,
        follows: None,
        precedes: None,
        all: None,
        any: None,
        not: None,
        matches: None,
    };

    let json = serde_json::to_string(&rule).unwrap();
    let deserialized: RuleObject = serde_json::from_str(&json).unwrap();

    assert_eq!(rule.regex, deserialized.regex);
}

#[test]
fn test_rule_object_composite_all() {
    let rule = RuleObject {
        pattern: None,
        kind: None,
        regex: None,
        inside: None,
        has: None,
        follows: None,
        precedes: None,
        all: Some(vec![
            RuleObject {
                pattern: Some(PatternSpec::Simple("function $NAME()".to_string())),
                kind: None,
                regex: None,
                inside: None,
                has: None,
                follows: None,
                precedes: None,
                all: None,
                any: None,
                not: None,
                matches: None,
            },
            RuleObject {
                pattern: None,
                kind: Some("function_declaration".to_string()),
                regex: None,
                inside: None,
                has: None,
                follows: None,
                precedes: None,
                all: None,
                any: None,
                not: None,
                matches: None,
            },
        ]),
        any: None,
        not: None,
        matches: None,
    };

    let json = serde_json::to_string(&rule).unwrap();
    let deserialized: RuleObject = serde_json::from_str(&json).unwrap();

    assert!(deserialized.all.is_some());
    assert_eq!(deserialized.all.unwrap().len(), 2);
}

#[test]
fn test_rule_object_composite_any() {
    let rule = RuleObject {
        pattern: None,
        kind: None,
        regex: None,
        inside: None,
        has: None,
        follows: None,
        precedes: None,
        all: None,
        any: Some(vec![RuleObject {
            pattern: Some(PatternSpec::Simple("console.log($MSG)".to_string())),
            kind: None,
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: None,
            any: None,
            not: None,
            matches: None,
        }]),
        not: None,
        matches: None,
    };

    let json = serde_json::to_string(&rule).unwrap();
    let deserialized: RuleObject = serde_json::from_str(&json).unwrap();

    assert!(deserialized.any.is_some());
    assert_eq!(deserialized.any.unwrap().len(), 1);
}

#[test]
fn test_rule_object_with_not() {
    let rule = RuleObject {
        pattern: None,
        kind: None,
        regex: None,
        inside: None,
        has: None,
        follows: None,
        precedes: None,
        all: None,
        any: None,
        not: Some(Box::new(RuleObject {
            pattern: Some(PatternSpec::Simple("deprecated_function()".to_string())),
            kind: None,
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: None,
            any: None,
            not: None,
            matches: None,
        })),
        matches: None,
    };

    let json = serde_json::to_string(&rule).unwrap();
    let deserialized: RuleObject = serde_json::from_str(&json).unwrap();

    assert!(deserialized.not.is_some());
}

#[test]
fn test_rule_object_relational_inside() {
    let rule = RuleObject {
        pattern: Some(PatternSpec::Simple("$VAR".to_string())),
        kind: None,
        regex: None,
        inside: Some(Box::new(RuleObject {
            pattern: Some(PatternSpec::Simple(
                "function $NAME() { $BODY }".to_string(),
            )),
            kind: None,
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: None,
            any: None,
            not: None,
            matches: None,
        })),
        has: None,
        follows: None,
        precedes: None,
        all: None,
        any: None,
        not: None,
        matches: None,
    };

    let json = serde_json::to_string(&rule).unwrap();
    let deserialized: RuleObject = serde_json::from_str(&json).unwrap();

    assert!(deserialized.inside.is_some());
}

#[test]
fn test_rule_object_relational_has() {
    let rule = RuleObject {
        pattern: Some(PatternSpec::Simple(
            "function $NAME() { $BODY }".to_string(),
        )),
        kind: None,
        regex: None,
        inside: None,
        has: Some(Box::new(RuleObject {
            pattern: Some(PatternSpec::Simple("return $VALUE".to_string())),
            kind: None,
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: None,
            any: None,
            not: None,
            matches: None,
        })),
        follows: None,
        precedes: None,
        all: None,
        any: None,
        not: None,
        matches: None,
    };

    let json = serde_json::to_string(&rule).unwrap();
    let deserialized: RuleObject = serde_json::from_str(&json).unwrap();

    assert!(deserialized.has.is_some());
}

#[test]
fn test_rule_object_relational_follows() {
    let rule = RuleObject {
        pattern: Some(PatternSpec::Simple("$STMT2".to_string())),
        kind: None,
        regex: None,
        inside: None,
        has: None,
        follows: Some(Box::new(RuleObject {
            pattern: Some(PatternSpec::Simple("$STMT1".to_string())),
            kind: None,
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: None,
            any: None,
            not: None,
            matches: None,
        })),
        precedes: None,
        all: None,
        any: None,
        not: None,
        matches: None,
    };

    let json = serde_json::to_string(&rule).unwrap();
    let deserialized: RuleObject = serde_json::from_str(&json).unwrap();

    assert!(deserialized.follows.is_some());
}

#[test]
fn test_rule_object_relational_precedes() {
    let rule = RuleObject {
        pattern: Some(PatternSpec::Simple("$STMT1".to_string())),
        kind: None,
        regex: None,
        inside: None,
        has: None,
        follows: None,
        precedes: Some(Box::new(RuleObject {
            pattern: Some(PatternSpec::Simple("$STMT2".to_string())),
            kind: None,
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: None,
            any: None,
            not: None,
            matches: None,
        })),
        all: None,
        any: None,
        not: None,
        matches: None,
    };

    let json = serde_json::to_string(&rule).unwrap();
    let deserialized: RuleObject = serde_json::from_str(&json).unwrap();

    assert!(deserialized.precedes.is_some());
}

#[test]
fn test_rule_object_with_matches() {
    let rule = RuleObject {
        pattern: None,
        kind: None,
        regex: None,
        inside: None,
        has: None,
        follows: None,
        precedes: None,
        all: None,
        any: None,
        not: None,
        matches: Some("variable-name".to_string()),
    };

    let json = serde_json::to_string(&rule).unwrap();
    let deserialized: RuleObject = serde_json::from_str(&json).unwrap();

    assert_eq!(rule.matches, deserialized.matches);
}

#[test]
fn test_rule_search_param_defaults() {
    let json = r#"{"rule_config": "test"}"#;
    let param: RuleSearchParam = serde_json::from_str(json).unwrap();

    assert_eq!(param.rule_config, "test");
    assert_eq!(param.path_pattern, None);
    assert_eq!(param.max_results, 10000); // default value
    assert_eq!(param.max_file_size, 50 * 1024 * 1024); // default value
    assert!(param.cursor.is_none());
}

#[test]
fn test_rule_search_param_with_all_fields() {
    let cursor = CursorParam {
        last_file_path: "/path/to/file.rs".to_string(),
        is_complete: false,
    };

    let param = RuleSearchParam {
        rule_config: "test rule config".to_string(),
        path_pattern: Some("**/*.rs".to_string()),
        max_results: 500,
        max_file_size: 1024 * 1024,
        cursor: Some(cursor),
    };

    let json = serde_json::to_string(&param).unwrap();
    let deserialized: RuleSearchParam = serde_json::from_str(&json).unwrap();

    assert_eq!(param.rule_config, deserialized.rule_config);
    assert_eq!(param.path_pattern, deserialized.path_pattern);
    assert_eq!(param.max_results, deserialized.max_results);
    assert_eq!(param.max_file_size, deserialized.max_file_size);
    assert!(deserialized.cursor.is_some());
    let cursor = deserialized.cursor.unwrap();
    assert_eq!(cursor.last_file_path, "/path/to/file.rs");
    assert!(!cursor.is_complete);
}

#[test]
fn test_rule_replace_param_defaults() {
    let json = r#"{"rule_config": "test"}"#;
    let param: RuleReplaceParam = serde_json::from_str(json).unwrap();

    assert_eq!(param.rule_config, "test");
    assert_eq!(param.path_pattern, None);
    assert_eq!(param.max_results, 10000); // default value
    assert_eq!(param.max_file_size, 50 * 1024 * 1024); // default value
    assert!(param.dry_run); // default value
    assert!(!param.summary_only); // default value
    assert!(param.cursor.is_none());
}

#[test]
fn test_rule_replace_param_with_all_fields() {
    let param = RuleReplaceParam {
        rule_config: "test rule config".to_string(),
        path_pattern: Some("src/**/*.js".to_string()),
        max_results: 1000,
        max_file_size: 2 * 1024 * 1024,
        dry_run: false,
        summary_only: true,
        cursor: None,
    };

    let json = serde_json::to_string(&param).unwrap();
    let deserialized: RuleReplaceParam = serde_json::from_str(&json).unwrap();

    assert_eq!(param.rule_config, deserialized.rule_config);
    assert_eq!(param.path_pattern, deserialized.path_pattern);
    assert_eq!(param.max_results, deserialized.max_results);
    assert_eq!(param.max_file_size, deserialized.max_file_size);
    assert_eq!(param.dry_run, deserialized.dry_run);
    assert_eq!(param.summary_only, deserialized.summary_only);
}

#[test]
fn test_rule_validate_param() {
    let param = RuleValidateParam {
        rule_config: "id: test\nlanguage: javascript\nrule:\n  pattern: console.log($VAR)"
            .to_string(),
        test_code: Some("console.log('hello world');".to_string()),
    };

    let json = serde_json::to_string(&param).unwrap();
    let deserialized: RuleValidateParam = serde_json::from_str(&json).unwrap();

    assert_eq!(param.rule_config, deserialized.rule_config);
    assert_eq!(param.test_code, deserialized.test_code);
}

#[test]
fn test_rule_validate_param_no_test_code() {
    let param = RuleValidateParam {
        rule_config: "id: test\nlanguage: rust".to_string(),
        test_code: None,
    };

    let json = serde_json::to_string(&param).unwrap();
    let deserialized: RuleValidateParam = serde_json::from_str(&json).unwrap();

    assert_eq!(param.rule_config, deserialized.rule_config);
    assert_eq!(param.test_code, None);
}

#[test]
fn test_rule_validate_result() {
    let result = RuleValidateResult {
        valid: true,
        errors: vec![],
        test_results: Some(RuleTestResult {
            matches_found: 2,
            sample_matches: vec![
                "console.log('hello')".to_string(),
                "console.log('world')".to_string(),
            ],
        }),
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: RuleValidateResult = serde_json::from_str(&json).unwrap();

    assert_eq!(result.valid, deserialized.valid);
    assert_eq!(result.errors.len(), deserialized.errors.len());
    assert!(deserialized.test_results.is_some());
    let test_results = deserialized.test_results.unwrap();
    assert_eq!(test_results.matches_found, 2);
    assert_eq!(test_results.sample_matches.len(), 2);
}

#[test]
fn test_rule_validate_result_invalid() {
    let result = RuleValidateResult {
        valid: false,
        errors: vec![
            "Invalid YAML syntax".to_string(),
            "Missing required field 'rule'".to_string(),
        ],
        test_results: None,
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: RuleValidateResult = serde_json::from_str(&json).unwrap();

    assert_eq!(result.valid, deserialized.valid);
    assert_eq!(result.errors.len(), 2);
    assert!(result.test_results.is_none());
}

#[test]
fn test_create_rule_param_defaults() {
    let json = r#"{"rule_config": "test"}"#;
    let param: CreateRuleParam = serde_json::from_str(json).unwrap();

    assert_eq!(param.rule_config, "test");
    assert!(!param.overwrite); // default value
}

#[test]
fn test_create_rule_param_with_overwrite() {
    let param = CreateRuleParam {
        rule_config: "test rule config".to_string(),
        overwrite: true,
    };

    let json = serde_json::to_string(&param).unwrap();
    let deserialized: CreateRuleParam = serde_json::from_str(&json).unwrap();

    assert_eq!(param.rule_config, deserialized.rule_config);
    assert_eq!(param.overwrite, deserialized.overwrite);
}

#[test]
fn test_create_rule_result() {
    let result = CreateRuleResult {
        rule_id: "my-custom-rule".to_string(),
        created: true,
        file_path: "/home/user/.ast-grep-mcp/rules/my-custom-rule.yaml".to_string(),
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: CreateRuleResult = serde_json::from_str(&json).unwrap();

    assert_eq!(result.rule_id, deserialized.rule_id);
    assert_eq!(result.created, deserialized.created);
    assert_eq!(result.file_path, deserialized.file_path);
}

#[test]
fn test_list_rules_param() {
    let param = ListRulesParam {
        language: Some("javascript".to_string()),
        severity: Some("warning".to_string()),
    };

    let json = serde_json::to_string(&param).unwrap();
    let deserialized: ListRulesParam = serde_json::from_str(&json).unwrap();

    assert_eq!(param.language, deserialized.language);
    assert_eq!(param.severity, deserialized.severity);
}

#[test]
fn test_list_rules_param_empty() {
    let param = ListRulesParam {
        language: None,
        severity: None,
    };

    let json = serde_json::to_string(&param).unwrap();
    let _deserialized: ListRulesParam = serde_json::from_str(&json).unwrap();

    assert_eq!(param.language, None);
    assert_eq!(param.severity, None);
}

#[test]
fn test_list_rules_result() {
    let result = ListRulesResult {
        rules: vec![
            RuleInfo {
                id: "no-console-log".to_string(),
                message: Some("Avoid using console.log".to_string()),
                language: "javascript".to_string(),
                severity: Some("warning".to_string()),
                file_path: "/rules/no-console-log.yaml".to_string(),
                has_fix: true,
            },
            RuleInfo {
                id: "use-const".to_string(),
                message: None,
                language: "javascript".to_string(),
                severity: Some("error".to_string()),
                file_path: "/rules/use-const.yaml".to_string(),
                has_fix: false,
            },
        ],
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: ListRulesResult = serde_json::from_str(&json).unwrap();

    assert_eq!(result.rules.len(), 2);
    assert_eq!(deserialized.rules.len(), 2);
    assert_eq!(deserialized.rules[0].id, "no-console-log");
    assert!(deserialized.rules[0].has_fix);
    assert_eq!(deserialized.rules[1].id, "use-const");
    assert!(!deserialized.rules[1].has_fix);
}

#[test]
fn test_get_rule_param() {
    let param = GetRuleParam {
        rule_id: "my-rule-id".to_string(),
    };

    let json = serde_json::to_string(&param).unwrap();
    let deserialized: GetRuleParam = serde_json::from_str(&json).unwrap();

    assert_eq!(param.rule_id, deserialized.rule_id);
}

#[test]
fn test_get_rule_result() {
    let rule_config = RuleConfig {
        id: "test-rule".to_string(),
        message: Some("Test rule".to_string()),
        language: "javascript".to_string(),
        severity: Some("info".to_string()),
        rule: RuleObject {
            pattern: Some(PatternSpec::Simple("test".to_string())),
            kind: None,
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: None,
            any: None,
            not: None,
            matches: None,
        },
        fix: None,
    };

    let result = GetRuleResult {
        rule_config,
        file_path: "/rules/test-rule.yaml".to_string(),
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: GetRuleResult = serde_json::from_str(&json).unwrap();

    assert_eq!(result.rule_config.id, deserialized.rule_config.id);
    assert_eq!(result.file_path, deserialized.file_path);
}

#[test]
fn test_delete_rule_param() {
    let param = DeleteRuleParam {
        rule_id: "rule-to-delete".to_string(),
    };

    let json = serde_json::to_string(&param).unwrap();
    let deserialized: DeleteRuleParam = serde_json::from_str(&json).unwrap();

    assert_eq!(param.rule_id, deserialized.rule_id);
}

#[test]
fn test_delete_rule_result() {
    let result = DeleteRuleResult {
        rule_id: "deleted-rule".to_string(),
        deleted: true,
        message: "Rule successfully deleted".to_string(),
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: DeleteRuleResult = serde_json::from_str(&json).unwrap();

    assert_eq!(result.rule_id, deserialized.rule_id);
    assert_eq!(result.deleted, deserialized.deleted);
    assert_eq!(result.message, deserialized.message);
}

#[test]
fn test_delete_rule_result_failed() {
    let result = DeleteRuleResult {
        rule_id: "nonexistent-rule".to_string(),
        deleted: false,
        message: "Rule not found".to_string(),
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: DeleteRuleResult = serde_json::from_str(&json).unwrap();

    assert_eq!(result.rule_id, deserialized.rule_id);
    assert!(!result.deleted);
    assert_eq!(result.message, deserialized.message);
}

#[test]
fn test_relational_rule_nested() {
    let relational = RelationalRule {
        pattern: Some(PatternSpec::Simple("$VAR".to_string())),
        kind: None,
        regex: None,
        inside: Some(Box::new(RelationalRule {
            pattern: Some(PatternSpec::Simple(
                "function $NAME() { $BODY }".to_string(),
            )),
            kind: None,
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: None,
            any: None,
            not: None,
            matches: None,
        })),
        has: None,
        follows: None,
        precedes: None,
        all: None,
        any: None,
        not: None,
        matches: None,
    };

    let json = serde_json::to_string(&relational).unwrap();
    let deserialized: RelationalRule = serde_json::from_str(&json).unwrap();

    assert!(deserialized.inside.is_some());
    match &deserialized.pattern {
        Some(PatternSpec::Simple(s)) => assert_eq!(s, "$VAR"),
        _ => panic!("Expected simple pattern"),
    }
}

#[test]
fn test_relational_rule_composite() {
    let relational = RelationalRule {
        pattern: None,
        kind: None,
        regex: None,
        inside: None,
        has: None,
        follows: None,
        precedes: None,
        all: Some(vec![
            RelationalRule {
                pattern: Some(PatternSpec::Simple("test1".to_string())),
                kind: None,
                regex: None,
                inside: None,
                has: None,
                follows: None,
                precedes: None,
                all: None,
                any: None,
                not: None,
                matches: None,
            },
            RelationalRule {
                pattern: Some(PatternSpec::Simple("test2".to_string())),
                kind: None,
                regex: None,
                inside: None,
                has: None,
                follows: None,
                precedes: None,
                all: None,
                any: None,
                not: None,
                matches: None,
            },
        ]),
        any: None,
        not: None,
        matches: None,
    };

    let json = serde_json::to_string(&relational).unwrap();
    let deserialized: RelationalRule = serde_json::from_str(&json).unwrap();

    assert!(deserialized.all.is_some());
    assert_eq!(deserialized.all.unwrap().len(), 2);
}
